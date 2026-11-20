#![allow(clippy::unwrap_used)]

use std::io::{ErrorKind, Read};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::thread;
use std::time::{Duration, Instant};

use ttymud_net::{Keys, PublicKey, Receiver, Sender};
use ttymud_server::{Identity, Peer, port_ostra, serve};
use ttymud_wire::{ClientMessage, ServerMessage};

fn start() -> (SocketAddr, PublicKey) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let keys = Keys::generate().unwrap();
    let public = keys.public();
    thread::spawn(move || {
        let world = ttymud_server::port_ostra(0).unwrap();
        let identity = ttymud_server::Identity::default();
        ttymud_server::serve(&listener, keys, world, identity, None, Vec::new())
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

    fn joined(address: SocketAddr, name: &str) -> Self {
        let mut client = Self::connect(address);
        client.welcome();
        client.until_prompt();
        client.send(name);
        client.until_prompt();
        client.send("breaker");
        client.until_prompt();
        client
    }

    fn next(&mut self) -> ServerMessage {
        ServerMessage::decode(&self.receiver.receive().unwrap()).unwrap()
    }

    fn text(&mut self) -> String {
        loop {
            match self.next() {
                ServerMessage::Text(text) => return text.plain(),
                ServerMessage::Refresh(_) | ServerMessage::Status(_) => {}
                other => panic!("expected text, got {other:?}"),
            }
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
                ServerMessage::Refresh(_) | ServerMessage::Status(_) => {}
                other => panic!("unexpected {other:?}"),
            }
        }
    }
}

#[test]
fn a_new_client_gets_a_welcome_and_makes_a_handle() {
    let (address, server) = start();
    let mut client = Client::connect(address);
    assert_eq!(client.server, server);
    assert!(client.welcome().starts_with("anon-"));
    assert!(client.until_prompt()[0].starts_with("You are new in Port Ostra."));
    client.send("ghost");
    assert_eq!(client.until_prompt()[0], "Choose a starter kit:");
    client.send("breaker");
    assert_eq!(client.until_prompt()[1], "The Relay");
}

#[test]
fn commands_get_answers() {
    let (address, _) = start();
    let mut client = Client::joined(address, "ghost");
    client.send("ls");
    assert!(client.until_prompt().contains(&"night-market/".to_owned()));
}

#[test]
fn players_hear_each_other() {
    let (address, _) = start();
    let mut ghost = Client::joined(address, "ghost");
    let mut nyx = Client::joined(address, "nyx");
    assert_eq!(ghost.text(), "nyx connects.");
    nyx.send("say hello");
    assert_eq!(nyx.until_prompt(), ["<nyx> hello"]);
    assert_eq!(ghost.text(), "<nyx> hello");
}

#[test]
fn others_see_a_disconnect() {
    let (address, _) = start();
    let mut ghost = Client::joined(address, "ghost");
    let nyx = Client::joined(address, "nyx");
    assert_eq!(ghost.text(), "nyx connects.");
    drop(nyx);
    assert_eq!(ghost.text(), "nyx disconnects.");
}

#[test]
fn completion_requests_get_candidates() {
    let (address, _) = start();
    let mut client = Client::joined(address, "ghost");
    let request = ClientMessage::Complete("go n".to_owned());
    client.sender.send(&request.encode()).unwrap();
    assert_eq!(
        client.next(),
        ServerMessage::Candidates {
            word: "n".to_owned(),
            names: vec!["night-market".to_owned(), "newsstand".to_owned()],
        }
    );
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
    let mut client = Client::joined(address, "ghost");
    for _ in 0..30 {
        client.send("?");
    }
    let (answered, refused) = count(
        &mut client,
        30,
        "You are sending commands too fast. Wait a moment.",
    );
    assert!(answered >= 18, "{answered}");
    assert!(refused >= 1, "{refused}");
}

#[test]
fn fast_chat_is_slowed_down_sooner() {
    let (address, _) = start();
    let mut client = Client::joined(address, "ghost");
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

fn linked(both: bool) -> (SocketAddr, SocketAddr) {
    let listeners = [(); 2].map(|()| TcpListener::bind("127.0.0.1:0").unwrap());
    let addresses = listeners
        .each_ref()
        .map(|listener| listener.local_addr().unwrap());
    let keys = [(); 2].map(|()| Keys::generate().unwrap());
    let peer = |name: &str, index: usize| Peer {
        name: name.to_owned(),
        address: addresses[index].to_string(),
        key: keys[index].public(),
    };
    let lists = [
        vec![peer("bravo", 1)],
        if both {
            vec![peer("alpha", 0)]
        } else {
            Vec::new()
        },
    ];
    for ((listener, keys), peers) in listeners.into_iter().zip(keys).zip(lists) {
        thread::spawn(move || {
            let world = port_ostra(0).unwrap();
            serve(&listener, keys, world, Identity::default(), None, peers)
        });
    }
    (addresses[0], addresses[1])
}

fn cities_until(client: &mut Client, line: &str) -> Vec<String> {
    let start = Instant::now();
    loop {
        client.send("cities");
        let shown = client.until_prompt();
        if shown.iter().any(|known| known == line) || start.elapsed() > Duration::from_secs(10) {
            return shown;
        }
        thread::sleep(Duration::from_millis(50));
    }
}

fn heard(client: &mut Client, expected: &str) -> String {
    for _ in 0..10 {
        let text = client.text();
        if text == expected {
            return text;
        }
    }
    String::new()
}

#[test]
fn linked_servers_carry_the_longline_both_ways() {
    let (alpha, bravo) = linked(true);
    let mut ghost = Client::joined(alpha, "ghost");
    let mut nyx = Client::joined(bravo, "nyx");
    for client in [&mut ghost, &mut nyx] {
        client.send("join longline");
        client.until_prompt();
    }
    let up = "bravo: Port Ostra. The link is up.";
    assert!(cities_until(&mut ghost, up).contains(&up.to_owned()));
    let up = "alpha: Port Ostra. The link is up.";
    assert!(cities_until(&mut nyx, up).contains(&up.to_owned()));
    ghost.send("#longline hello bravo");
    assert_eq!(ghost.until_prompt(), ["[longline] <ghost> hello bravo"]);
    let expected = "[longline] <ghost@alpha> hello bravo";
    assert_eq!(heard(&mut nyx, expected), expected);
    nyx.send("#longline hello alpha");
    let expected = "[longline] <nyx@bravo> hello alpha";
    assert_eq!(heard(&mut ghost, expected), expected);
}

#[test]
fn a_link_needs_both_servers_to_list_each_other() {
    let (alpha, _) = linked(false);
    let mut ghost = Client::joined(alpha, "ghost");
    thread::sleep(Duration::from_millis(500));
    ghost.send("cities");
    assert_eq!(ghost.until_prompt()[0], "bravo: The link is down.");
}

#[test]
fn a_bad_message_ends_the_connection() {
    let (address, _) = start();
    let mut client = Client::joined(address, "ghost");
    client.sender.send(&[200, 1, 2]).unwrap();
    assert!(client.receiver.receive().is_err());
}
