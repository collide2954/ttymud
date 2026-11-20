use std::io::{self, BufRead, BufReader, Write};
use std::net::TcpStream;
use std::process::{Command, Output, Stdio};
use std::time::Duration;

use ttymud_net::Keys;
use ttymud_wire::ServerMessage;

fn server(args: &[&str]) -> io::Result<Output> {
    Command::new(env!("CARGO_BIN_EXE_ttymud-server"))
        .args(args)
        .output()
}

#[test]
fn version_prints_name_and_version() -> io::Result<()> {
    let output = server(&["--version"])?;
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        concat!("ttymud-server ", env!("CARGO_PKG_VERSION"), "\n")
    );
    Ok(())
}

#[test]
fn help_prints_usage() -> io::Result<()> {
    let output = server(&["--help"])?;
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).starts_with("Usage: ttymud-server "));
    Ok(())
}

#[test]
fn play_walks_the_default_city() -> io::Result<()> {
    let mut child = Command::new(env!("CARGO_BIN_EXE_ttymud-server"))
        .arg("play")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(b"nyx\nghost\ngo night-market\nquit\n")?;
    }
    let output = child.wait_with_output()?;
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("nyx@night-market $ "));
    Ok(())
}

#[test]
fn init_shows_the_fingerprint_and_keeps_the_key() -> io::Result<()> {
    let dir = std::env::temp_dir().join(format!("ttymud-cli-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let place = dir.to_string_lossy().into_owned();
    let first = server(&["init", &place])?;
    let second = server(&["init", &place])?;
    let _ = std::fs::remove_dir_all(&dir);
    assert!(first.status.success());
    let first = String::from_utf8_lossy(&first.stdout);
    let second = String::from_utf8_lossy(&second.stdout);
    assert!(first.contains("Created a server key"));
    assert!(second.contains("already has a server key"));
    let line = |text: &str, start: &str| {
        text.lines()
            .find_map(|line| line.strip_prefix(start))
            .map(str::to_owned)
    };
    assert!(line(&first, "Fingerprint: ").is_some());
    assert_eq!(
        line(&first, "Fingerprint: "),
        line(&second, "Fingerprint: ")
    );
    let key = line(&first, "Key: ").unwrap_or_default();
    let public = ttymud_net::PublicKey::from_hex(&key);
    assert_eq!(
        public.map(|key| key.fingerprint()),
        line(&first, "Fingerprint: ")
    );
    assert_eq!(line(&second, "Key: "), Some(key));
    Ok(())
}

fn scratch(name: &str) -> String {
    let dir = std::env::temp_dir().join(format!("ttymud-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir.to_string_lossy().into_owned()
}

#[test]
fn run_without_init_explains_what_to_do() -> io::Result<()> {
    let dir = scratch("no-init");
    let output = server(&["run", &dir])?;
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("has no server key. Run: ttymud-server init"));
    Ok(())
}

#[test]
fn run_opens_the_city_to_clients() -> io::Result<()> {
    let dir = scratch("run");
    server(&["init", &dir])?;
    let mut child = Command::new(env!("CARGO_BIN_EXE_ttymud-server"))
        .args(["run", &dir, "--listen", "127.0.0.1:0"])
        .stdout(Stdio::piped())
        .spawn()?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| io::Error::other("no stdout"))?;
    let mut lines = BufReader::new(stdout).lines();
    let first = lines.next().unwrap_or_else(|| Ok(String::new()))?;
    let address = first
        .trim_end_matches('.')
        .rsplit(' ')
        .next()
        .unwrap_or_default()
        .to_owned();
    let welcome = welcome_from(&address);
    child.kill()?;
    child.wait()?;
    let _ = std::fs::remove_dir_all(&dir);
    assert!(
        first.starts_with("Port Ostra is open on 127.0.0.1:"),
        "{first}"
    );
    assert!(
        matches!(welcome, Some(ServerMessage::Welcome { .. })),
        "{welcome:?}"
    );
    Ok(())
}

fn welcome_from(address: &str) -> Option<ServerMessage> {
    let mut stream = TcpStream::connect(address).ok()?;
    stream.set_read_timeout(Some(Duration::from_secs(5))).ok()?;
    let session = ttymud_net::connect(&mut stream, &Keys::generate().ok()?).ok()?;
    let (mut receiver, _sender) = session.split(stream.try_clone().ok()?, stream);
    ServerMessage::decode(&receiver.receive().ok()?).ok()
}

#[test]
fn unknown_argument_fails_with_a_hint() -> io::Result<()> {
    let output = server(&["--bogus"])?;
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("unknown argument '--bogus'"));
    assert!(stderr.contains("ttymud-server --help"));
    Ok(())
}
