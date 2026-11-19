#![allow(clippy::unwrap_used)]

use std::io::{ErrorKind, Read};
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
        Self::connect_as(address, &Keys::generate().unwrap())
    }

    fn connect_as(address: SocketAddr, keys: &Keys) -> Self {
        let mut stream = TcpStream::connect(address).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let session = ttymud_net::connect(&mut stream, keys).unwrap();
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
fn a_new_connection_with_the_same_key_takes_over() {
    let (address, _) = start();
    let keys = Keys::generate().unwrap();
    let mut first = Client::connect_as(address, &keys);
    let name = first.welcome();
    first.until_prompt();
    let mut second = Client::connect_as(address, &keys);
    assert_eq!(second.welcome(), name);
    first.send("look");
    let closed = ServerMessage::Close("You connected from another place.".to_owned());
    assert_eq!(first.next(), closed);
    assert!(first.receiver.receive().is_err());
}

fn count(client: &mut Client, sent: usize, slowed: &str) -> (usize, usize) {
    let (mut answered, mut refused) = (0, 0);
    while answered + refused < sent {
        match client.next() {
            ServerMessage::Prompt(_) => answered += 1,
            ServerMessage::Text(text) if text.plain() == slowed => refused += 1,
            ServerMessage::Text(_) => {}
            other => panic!("unexpected {other:?}"),
        }
    }
    (answered, refused)
}

#[test]
fn fast_commands_are_slowed_down() {
    let (address, _) = start();
    let (mut client, _) = Client::joined(address);
    for _ in 0..30 {
        client.send("?");
    }
    let (answered, refused) = count(
        &mut client,
        30,
        "You are sending commands too fast. Wait a moment.",
    );
    assert!(answered >= 20, "{answered}");
    assert!(refused >= 1, "{refused}");
}

#[test]
fn fast_chat_is_slowed_down_sooner() {
    let (address, _) = start();
    let (mut client, _) = Client::joined(address);
    for _ in 0..8 {
        client.send("say hi");
    }
    let (answered, refused) = count(&mut client, 8, "You are talking too fast. Wait a moment.");
    assert!(answered >= 5, "{answered}");
    assert!(refused >= 1, "{refused}");
}

#[test]
fn too_many_connections_from_one_address_are_refused() {
    let (address, _) = start();
    let held: Vec<TcpStream> = (0..8)
        .map(|_| TcpStream::connect(address).unwrap())
        .collect();
    let mut ninth = TcpStream::connect(address).unwrap();
    ninth
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let result = ninth.read(&mut [0; 1]);
    drop(held);
    let closed = match &result {
        Ok(read) => *read == 0,
        Err(error) => matches!(
            error.kind(),
            ErrorKind::ConnectionReset | ErrorKind::ConnectionAborted
        ),
    };
    assert!(closed, "{result:?}");
}

#[test]
fn a_bad_message_ends_the_connection() {
    let (address, _) = start();
    let (mut client, _) = Client::joined(address);
    client.sender.send(&[200, 1, 2]).unwrap();
    assert!(client.receiver.receive().is_err());
}
