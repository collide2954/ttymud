#![cfg(feature = "ssh")]
#![allow(clippy::unwrap_used)]

use std::net::{SocketAddr, TcpListener};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use russh::client::{self, Handle, Msg};
use russh::keys::{Algorithm, PrivateKey, PrivateKeyWithHashAlg, PublicKeyOrCertificate};
use russh::{Channel, ChannelMsg};
use ttymud_net::Keys;
use ttymud_server::{Identity, SshPort, port_ostra, serve_with_ssh, ssh_host_key};

const WAIT: Duration = Duration::from_secs(10);

struct Trusting;

impl client::Handler for Trusting {
    type Error = russh::Error;

    fn check_server_key(
        &mut self,
        _key: &PublicKeyOrCertificate,
    ) -> impl Future<Output = Result<bool, Self::Error>> + Send {
        std::future::ready(Ok(true))
    }
}

fn start(name: &str) -> SocketAddr {
    let dir = std::env::temp_dir().join(format!("ttymud-ssh-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let key = ssh_host_key(&dir).unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = SshPort::new(TcpListener::bind("127.0.0.1:0").unwrap(), key);
    let address = port.address().unwrap();
    thread::spawn(move || {
        let keys = Keys::generate().unwrap();
        let world = port_ostra(0).unwrap();
        let identity = Identity::default();
        serve_with_ssh(&listener, port, keys, world, identity, None, Vec::new())
    });
    address
}

async fn login(address: SocketAddr) -> Handle<Trusting> {
    let config = Arc::new(client::Config::default());
    let mut session = client::connect(config, address, Trusting).await.unwrap();
    let key = PrivateKey::random(&mut rand::rng(), Algorithm::Ed25519).unwrap();
    let auth = session
        .authenticate_publickey("player", PrivateKeyWithHashAlg::new(Arc::new(key), None))
        .await
        .unwrap();
    assert!(auth.success());
    session
}

async fn read_until(channel: &mut Channel<Msg>, marker: &str) -> String {
    let mut text = String::new();
    while !text.contains(marker) {
        match tokio::time::timeout(WAIT, channel.wait()).await.unwrap() {
            Some(ChannelMsg::Data { data }) => text.push_str(&String::from_utf8_lossy(&data)),
            Some(_) => {}
            None => break,
        }
    }
    text
}

async fn exit_status(channel: &mut Channel<Msg>) -> Option<u32> {
    let mut status = None;
    while let Some(message) = tokio::time::timeout(WAIT, channel.wait()).await.unwrap() {
        if let ChannelMsg::ExitStatus { exit_status } = message {
            status = Some(exit_status);
        }
    }
    status
}

fn block_on<F: Future>(future: F) -> F::Output {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(future)
}

#[test]
fn a_player_with_an_ssh_key_makes_a_handle_and_plays() {
    let address = start("play");
    block_on(async {
        let session = login(address).await;
        let mut channel = session.channel_open_session().await.unwrap();
        channel
            .request_pty(false, "xterm", 80, 24, 0, 0, &[])
            .await
            .unwrap();
        channel.exec(false, "edit").await.unwrap();
        let text = read_until(&mut channel, "handle: ").await;
        assert!(text.contains("You are new in Port Ostra."), "{text:?}");
        channel.data(&b"nyx\r"[..]).await.unwrap();
        read_until(&mut channel, "kit: ").await;
        channel.data(&b"ghost\r"[..]).await.unwrap();
        let text = read_until(&mut channel, "nyx@relay $ ").await;
        assert!(text.contains("The Relay"), "{text:?}");
        channel.data(&b"\x04"[..]).await.unwrap();
        assert_eq!(exit_status(&mut channel).await, Some(0));
    });
}

#[test]
fn an_unknown_word_after_the_address_is_refused() {
    let address = start("word");
    block_on(async {
        let session = login(address).await;
        let mut channel = session.channel_open_session().await.unwrap();
        channel.exec(false, "fancy").await.unwrap();
        let text = read_until(&mut channel, "\r\n").await;
        assert_eq!(
            text,
            "Unknown word: fancy. Use line, edit, color, hud, ascii, utf8, or verbose.\r\n"
        );
        assert_eq!(exit_status(&mut channel).await, Some(2));
    });
}
