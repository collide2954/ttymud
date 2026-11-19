#![allow(clippy::unwrap_used)]

use std::net::{SocketAddr, TcpListener, TcpStream};
use std::thread;
use std::time::Duration;

use ttymud_net::{Keys, PublicKey, Receiver, Sender};
use ttymud_wire::{ClientMessage, ServerMessage};

fn start() -> (SocketAddr, PublicKey) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let keys = Keys::generate().unwrap();
    let public = keys.public();
    thread::spawn(move || {
        ttymud_server::serve(&listener, keys, ttymud_server::port_ostra().unwrap())
    });
    (address, public)
}

struct Client {
    receiver: Receiver<TcpStream>,
    sender: Sender<TcpStream>,
    server: PublicKey,
}

impl Client {
    fn connect(address: SocketAddr) -> Self {
        let mut stream = TcpStream::connect(address).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let session = ttymud_net::connect(&mut stream, &Keys::generate().unwrap()).unwrap();
        let server = session.peer();
        let (receiver, sender) = session.split(stream.try_clone().unwrap(), stream);
        Self {
            receiver,
            sender,
            server,
        }
    }

    fn joined(address: SocketAddr) -> (Self, String) {
        let mut client = Self::connect(address);
        let name = client.welcome();
        client.until_prompt();
        (client, name)
    }

    fn next(&mut self) -> ServerMessage {
        ServerMessage::decode(&self.receiver.receive().unwrap()).unwrap()
    }

    fn text(&mut self) -> String {
        match self.next() {
            ServerMessage::Text(text) => text.plain(),
            other => panic!("expected text, got {other:?}"),
        }
    }

    fn welcome(&mut self) -> String {
        match self.next() {
            ServerMessage::Welcome { name, .. } => name,
            other => panic!("expected a welcome, got {other:?}"),
        }
    }

    fn send(&mut self, line: &str) {
        let message = ClientMessage::Command(line.to_owned());
        self.sender.send(&message.encode()).unwrap();
    }

    fn until_prompt(&mut self) -> Vec<String> {
        let mut texts = Vec::new();
        loop {
            match self.next() {
                ServerMessage::Text(text) => texts.push(text.plain()),
                ServerMessage::Prompt(_) => return texts,
                other => panic!("unexpected {other:?}"),
            }
        }
    }
}

#[test]
fn a_client_gets_a_welcome_and_the_scene() {
    let (address, server) = start();
    let mut client = Client::connect(address);
    assert_eq!(client.server, server);
    assert!(client.welcome().starts_with("anon-"));
    assert_eq!(client.until_prompt()[0], "The Relay");
}

#[test]
fn commands_get_answers() {
    let (address, _) = start();
    let (mut client, _) = Client::joined(address);
    client.send("ls");
    assert!(client.until_prompt().contains(&"night-market/".to_owned()));
}

#[test]
fn players_hear_each_other() {
    let (address, _) = start();
    let (mut ghost, _) = Client::joined(address);
    let (mut nyx, name) = Client::joined(address);
    assert_eq!(ghost.text(), format!("{name} connects."));
    nyx.send("say hello");
    assert_eq!(nyx.until_prompt(), [format!("<{name}> hello")]);
    assert_eq!(ghost.text(), format!("<{name}> hello"));
}

#[test]
fn others_see_a_disconnect() {
    let (address, _) = start();
    let (mut ghost, _) = Client::joined(address);
    let (nyx, name) = Client::joined(address);
    assert_eq!(ghost.text(), format!("{name} connects."));
    drop(nyx);
    assert_eq!(ghost.text(), format!("{name} disconnects."));
}

#[test]
fn a_bad_message_ends_the_connection() {
    let (address, _) = start();
    let (mut client, _) = Client::joined(address);
    client.sender.send(&[200, 1, 2]).unwrap();
    assert!(client.receiver.receive().is_err());
}
