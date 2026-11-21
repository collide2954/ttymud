use std::fs::{self, OpenOptions};
use std::future::ready;
use std::io::{self, Write};
use std::net::{IpAddr, SocketAddr, TcpListener};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};
use std::thread;
use std::time::{Duration, Instant};

use russh::keys::ssh_key::LineEnding;
use russh::keys::{Algorithm, HashAlg, PrivateKey};
use russh::server::{
    Auth, ChannelOpenHandle, Config, Handle, Handler, Msg, Session, VerifiedPublicKey,
};
use russh::{Channel, ChannelId, Disconnect, MethodKind, MethodSet, Pty, SshId};
use ttymud::{Choices, Input, Remote, Wire, remote};
use ttymud_text::{Message, sanitize};
use ttymud_wire::{ClientMessage, ServerMessage};

use crate::game::Event;
use crate::key::AccountKey;
use crate::limits::{Admission, Limiter};
use crate::serve::Hub;

const HOST_KEY: &str = "ssh_host.key";
const LOGIN: Duration = Duration::from_secs(10);
const VARS: [&str; 5] = ["LANG", "LC_ALL", "LC_CTYPE", "COLORTERM", "NO_COLOR"];
const WORDS: &str = "Use line, edit, color, hud, ascii, utf8, verbose, or nomouse.";
const QUEUE: usize = 64;
const WRITE: Duration = Duration::from_secs(30);

pub struct SshPort {
    listener: TcpListener,
    key: PrivateKey,
}

impl SshPort {
    #[must_use]
    pub fn new(listener: TcpListener, key: PrivateKey) -> Self {
        Self { listener, key }
    }

    pub fn address(&self) -> io::Result<SocketAddr> {
        self.listener.local_addr()
    }

    #[must_use]
    pub fn fingerprint(&self) -> String {
        self.key.fingerprint(HashAlg::Sha256).to_string()
    }
}

pub fn ssh_host_key(dir: &Path) -> io::Result<PrivateKey> {
    let path = dir.join(HOST_KEY);
    match fs::read_to_string(&path) {
        Ok(text) => PrivateKey::from_openssh(text).map_err(io::Error::other),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            let key = PrivateKey::random(&mut rand::rng(), Algorithm::Ed25519)
                .map_err(io::Error::other)?;
            let text = key.to_openssh(LineEnding::LF).map_err(io::Error::other)?;
            save_new(&path, text.as_bytes())?;
            Ok(key)
        }
        Err(error) => Err(error),
    }
}

fn save_new(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path)?;
    file.write_all(bytes)?;
    file.sync_all()
}

pub(crate) fn start(port: SshPort, hub: Hub) -> io::Result<()> {
    let SshPort { listener, key } = port;
    listener.set_nonblocking(true)?;
    let config = Arc::new(config(key));
    thread::Builder::new()
        .name("ssh".to_owned())
        .spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build();
            match runtime {
                Ok(runtime) => runtime.block_on(listen(listener, config, hub)),
                Err(error) => eprintln!("ttymud-server: ssh: {error}"),
            }
        })?;
    Ok(())
}

fn config(key: PrivateKey) -> Config {
    Config {
        server_id: SshId::Standard(concat!("SSH-2.0-ttymud_", env!("CARGO_PKG_VERSION")).into()),
        methods: MethodSet::from(&[MethodKind::PublicKey][..]),
        auth_rejection_time: Duration::from_secs(1),
        auth_rejection_time_initial: Some(Duration::ZERO),
        keys: vec![key],
        max_auth_attempts: 6,
        inactivity_timeout: None,
        keepalive_interval: Some(Duration::from_secs(60)),
        nodelay: true,
        ..Config::default()
    }
}

async fn listen(listener: TcpListener, config: Arc<Config>, hub: Hub) {
    let listener = match tokio::net::TcpListener::from_std(listener) {
        Ok(listener) => listener,
        Err(error) => {
            eprintln!("ttymud-server: ssh: {error}");
            return;
        }
    };
    loop {
        let Ok((stream, peer)) = listener.accept().await else {
            tokio::time::sleep(Duration::from_millis(100)).await;
            continue;
        };
        let Some(admission) = Admission::new(&hub.limits, peer.ip()) else {
            continue;
        };
        let _ = stream.set_nodelay(true);
        let client = Client::new(hub.clone(), admission, peer.ip());
        tokio::spawn(serve(Arc::clone(&config), stream, client));
    }
}

async fn serve(config: Arc<Config>, stream: tokio::net::TcpStream, client: Client) {
    let ready = Arc::clone(&client.ready);
    let started = tokio::time::timeout(LOGIN, russh::server::run_stream(config, stream, client));
    let Ok(Ok(session)) = started.await else {
        return;
    };
    let handle = session.handle();
    tokio::spawn(async move {
        tokio::time::sleep(LOGIN).await;
        if !ready.load(Ordering::Relaxed) {
            let reason = "The login took too long.".to_owned();
            let _ = handle
                .disconnect(Disconnect::ByApplication, reason, String::new())
                .await;
        }
    });
    let _ = session.await;
}

struct Client {
    hub: Hub,
    address: IpAddr,
    _admission: Admission,
    ready: Arc<AtomicBool>,
    key: Option<[u8; 32]>,
    channel: Option<ChannelId>,
    started: bool,
    terminal: Option<(String, u16, u16)>,
    vars: Vec<(String, String)>,
    input: Option<tokio::sync::mpsc::Sender<Input>>,
}

impl Client {
    fn new(hub: Hub, admission: Admission, address: IpAddr) -> Self {
        Self {
            hub,
            address,
            _admission: admission,
            ready: Arc::new(AtomicBool::new(false)),
            key: None,
            channel: None,
            started: false,
            terminal: None,
            vars: Vec::new(),
            input: None,
        }
    }

    fn begin(
        &mut self,
        channel: ChannelId,
        words: &str,
        session: &mut Session,
    ) -> Result<(), russh::Error> {
        let open = !self.started && self.channel == Some(channel);
        let Some(digest) = self.key.filter(|_| open) else {
            return session.channel_failure(channel);
        };
        self.started = true;
        let choices = match choices(words) {
            Ok(choices) => choices,
            Err(problem) => {
                session.data(channel, format!("{problem}\r\n"))?;
                session.exit_status_request(channel, 2)?;
                return session.close(channel);
            }
        };
        session.channel_success(channel)?;
        let terminal = self.remote(choices);
        let (input, mut inbox) = tokio::sync::mpsc::channel(QUEUE);
        self.input = Some(input);
        let connection = self.hub.connection();
        let wire = self.join(connection, digest);
        let output = Outgoing::new(channel, session.handle());
        thread::Builder::new()
            .name(format!("ssh-{connection}"))
            .spawn(move || {
                let _ = remote(wire, move || inbox.blocking_recv(), output, terminal);
            })
            .map_err(russh::Error::IO)?;
        Ok(())
    }

    fn remote(&self, choices: Choices) -> Remote {
        let term = self.terminal.as_ref().map(|(term, ..)| term.clone());
        let vars = self.vars.clone();
        let local = vars
            .iter()
            .any(|(name, _)| name == "LANG" || name.starts_with("LC_"));
        let lookup = move |name: &str| match name {
            "TERM" => term.clone(),
            "LANG" if !local => Some("C.UTF-8".to_owned()),
            _ => vars
                .iter()
                .find(|(known, _)| known == name)
                .map(|(_, value)| value.clone()),
        };
        let size = self
            .terminal
            .as_ref()
            .map(|&(_, width, height)| (width, height));
        Remote::new(lookup, size, choices)
    }

    fn join(&self, connection: u64, digest: [u8; 32]) -> Wire {
        let (outbox, mailbox) = mpsc::channel();
        let warnings = outbox.clone();
        let _ = self.hub.events.send(Event::Join {
            connection,
            key: AccountKey::Ssh(digest),
            address: self.address,
            outbox,
        });
        let (events, leave) = (self.hub.events.clone(), self.hub.events.clone());
        let mut limiter = Limiter::new(Instant::now());
        Wire::new(
            move || mailbox.recv().ok(),
            move |message| {
                let event = match message {
                    ClientMessage::Command(line) => match limiter.check(&line, Instant::now()) {
                        Some(warning) => {
                            let _ = warnings.send(ServerMessage::Text(Message::error(warning)));
                            return Ok(());
                        }
                        None => Event::Command { connection, line },
                    },
                    ClientMessage::Complete(text) if limiter.completion(Instant::now()) => {
                        Event::Complete { connection, text }
                    }
                    ClientMessage::Complete(_) => return Ok(()),
                };
                events
                    .send(event)
                    .map_err(|_| io::Error::from(io::ErrorKind::BrokenPipe))
            },
            move || {
                let _ = leave.send(Event::Leave { connection });
            },
        )
    }
}

impl Handler for Client {
    type Error = russh::Error;

    fn auth_publickey(
        &mut self,
        _user: &str,
        key: VerifiedPublicKey<'_>,
    ) -> impl Future<Output = Result<Auth, Self::Error>> + Send {
        let auth = match key.public_key.fingerprint(HashAlg::Sha256).sha256() {
            Some(digest) => {
                self.key = Some(digest);
                self.ready.store(true, Ordering::Relaxed);
                Auth::Accept
            }
            None => Auth::reject(),
        };
        ready(Ok(auth))
    }

    async fn channel_open_session(
        &mut self,
        channel: Channel<Msg>,
        reply: ChannelOpenHandle,
        _session: &mut Session,
    ) -> Result<(), Self::Error> {
        if self.channel.is_none() {
            self.channel = Some(channel.id());
            reply.accept().await;
        }
        Ok(())
    }

    fn pty_request(
        &mut self,
        channel: ChannelId,
        term: &str,
        col_width: u32,
        row_height: u32,
        _pix_width: u32,
        _pix_height: u32,
        _modes: &[(Pty, u32)],
        session: &mut Session,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send {
        let term: String = term.chars().take(64).collect();
        self.terminal = Some((term, cells(col_width, 80), cells(row_height, 24)));
        ready(session.channel_success(channel))
    }

    fn env_request(
        &mut self,
        channel: ChannelId,
        variable_name: &str,
        variable_value: &str,
        session: &mut Session,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send {
        let known = self.vars.iter().any(|(name, _)| name == variable_name);
        if VARS.contains(&variable_name) && !known {
            let value: String = variable_value.chars().take(64).collect();
            self.vars.push((variable_name.to_owned(), value));
        }
        ready(session.channel_success(channel))
    }

    fn shell_request(
        &mut self,
        channel: ChannelId,
        session: &mut Session,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send {
        ready(self.begin(channel, "", session))
    }

    fn exec_request(
        &mut self,
        channel: ChannelId,
        data: &[u8],
        session: &mut Session,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send {
        ready(self.begin(channel, &String::from_utf8_lossy(data), session))
    }

    async fn window_change_request(
        &mut self,
        _channel: ChannelId,
        col_width: u32,
        row_height: u32,
        _pix_width: u32,
        _pix_height: u32,
        _session: &mut Session,
    ) -> Result<(), Self::Error> {
        if let Some(input) = &self.input {
            let size = Input::Resize(cells(col_width, 80), cells(row_height, 24));
            let _ = input.send(size).await;
        }
        Ok(())
    }

    async fn data(
        &mut self,
        _channel: ChannelId,
        data: &[u8],
        _session: &mut Session,
    ) -> Result<(), Self::Error> {
        if let Some(input) = &self.input
            && input.send(Input::Bytes(data.to_vec())).await.is_err()
        {
            return Err(russh::Error::Disconnect);
        }
        Ok(())
    }

    fn channel_eof(
        &mut self,
        _channel: ChannelId,
        _session: &mut Session,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send {
        self.input = None;
        ready(Ok(()))
    }

    fn channel_close(
        &mut self,
        _channel: ChannelId,
        _session: &mut Session,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send {
        self.input = None;
        ready(Ok(()))
    }
}

struct Outgoing {
    buffer: Vec<u8>,
    sender: tokio::sync::mpsc::Sender<Vec<u8>>,
}

impl Outgoing {
    fn new(channel: ChannelId, handle: Handle) -> Self {
        let (sender, mut receiver) = tokio::sync::mpsc::channel::<Vec<u8>>(QUEUE);
        tokio::spawn(async move {
            while let Some(bytes) = receiver.recv().await {
                let sent = tokio::time::timeout(WRITE, handle.data(channel, bytes)).await;
                if !matches!(sent, Ok(Ok(()))) {
                    let reason = "The client stopped reading.".to_owned();
                    let goodbye =
                        handle.disconnect(Disconnect::ByApplication, reason, String::new());
                    let _ = tokio::time::timeout(Duration::from_secs(1), goodbye).await;
                    return;
                }
            }
            let _ = handle.exit_status_request(channel, 0).await;
            let _ = handle.eof(channel).await;
            let _ = handle.close(channel).await;
        });
        Self {
            buffer: Vec::new(),
            sender,
        }
    }
}

impl Write for Outgoing {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.buffer.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        if self.buffer.is_empty() {
            return Ok(());
        }
        let bytes = std::mem::take(&mut self.buffer);
        self.sender
            .blocking_send(bytes)
            .map_err(|_| io::Error::from(io::ErrorKind::BrokenPipe))
    }
}

impl Drop for Outgoing {
    fn drop(&mut self) {
        let _ = self.flush();
    }
}

fn cells(value: u32, fallback: u16) -> u16 {
    u16::try_from(value)
        .ok()
        .filter(|&value| value > 0)
        .unwrap_or(fallback)
}

fn choices(words: &str) -> Result<Choices, String> {
    let mut choices = Choices::default();
    for word in words.split_whitespace() {
        match word.to_ascii_lowercase().as_str() {
            "ascii" => choices.ascii = Some(true),
            "utf8" | "utf-8" => choices.ascii = Some(false),
            "verbose" => choices.verbose = Some(true),
            "nomouse" => choices.mouse = Some(false),
            other => {
                let shown: String = sanitize(word).chars().take(20).collect();
                let layer = other
                    .parse()
                    .map_err(|_| format!("Unknown word: {shown}. {WORDS}"))?;
                choices.layer = Some(layer);
            }
        }
    }
    Ok(choices)
}

#[cfg(test)]
mod tests {
    use ttymud::{Choices, Layer};

    use super::{cells, choices, ssh_host_key};

    #[test]
    fn the_words_after_the_address_choose_the_display() {
        let hud = Choices {
            layer: Some(Layer::Hud),
            ascii: Some(true),
            verbose: Some(true),
            mouse: Some(false),
            ..Choices::default()
        };
        assert_eq!(choices("HUD ascii verbose nomouse"), Ok(hud));
        assert_eq!(choices(""), Ok(Choices::default()));
        assert_eq!(choices("utf8").map(|found| found.ascii), Ok(Some(false)));
        assert_eq!(
            choices("line fancy"),
            Err(
                "Unknown word: fancy. Use line, edit, color, hud, ascii, utf8, verbose, or nomouse."
                    .to_owned()
            )
        );
    }

    #[test]
    fn a_size_of_zero_or_too_big_takes_the_default() {
        assert_eq!(cells(120, 80), 120);
        assert_eq!(cells(0, 80), 80);
        assert_eq!(cells(70_000, 24), 24);
    }

    #[test]
    fn the_host_key_is_made_once_and_then_kept() {
        let dir = std::env::temp_dir().join(format!("ttymud-ssh-key-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let first = ssh_host_key(&dir).unwrap();
        let second = ssh_host_key(&dir).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(first.public_key(), second.public_key());
    }
}
