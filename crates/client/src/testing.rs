use std::fs;
use std::io;
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::mpsc;
use std::thread;

use ttymud_net::{Keys, PublicKey, Receiver, Sender};
use ttymud_text::Message;
use ttymud_wire::{ClientMessage, ServerMessage};

use crate::{Link, Wire};

type Handler = fn(Receiver<TcpStream>, Sender<TcpStream>);

pub(crate) fn scratch(name: &str) -> PathBuf {
    let id = std::process::id();
    let path = std::env::temp_dir().join(format!("ttymud-client-{id}-{name}"));
    let _ = fs::remove_dir_all(&path);
    path
}

pub(crate) fn server(handle: Handler) -> (SocketAddr, PublicKey) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let keys = Keys::generate().unwrap();
    let public = keys.public();
    thread::spawn(move || {
        for mut stream in listener.incoming().flatten() {
            let keys = keys.clone();
            thread::spawn(move || {
                let Ok(session) = ttymud_net::accept(&mut stream, &keys) else {
                    return;
                };
                let (receiver, sender) = session.split(stream.try_clone().unwrap(), stream);
                handle(receiver, sender);
            });
        }
    });
    (address, public)
}

pub(crate) fn link(handle: Handler) -> Link {
    let (address, _) = server(handle);
    let known = std::env::temp_dir().join(format!(
        "ttymud-known-{}-{}",
        std::process::id(),
        address.port()
    ));
    let keys = Keys::generate().unwrap();
    let link = crate::connect(&address.to_string(), &keys, &known, None, |_, _| Ok(true)).unwrap();
    let _ = std::fs::remove_file(&known);
    link
}

pub(crate) fn send(sender: &mut Sender<TcpStream>, message: &ServerMessage) {
    let _ = sender.send(&message.encode());
}

pub(crate) fn ticker(mut receiver: Receiver<TcpStream>, mut sender: Sender<TcpStream>) {
    let welcome = ServerMessage::Welcome {
        city: "Test City".to_owned(),
        name: "anon-test".to_owned(),
    };
    send(&mut sender, &welcome);
    send(&mut sender, &ServerMessage::Prompt("$ ".to_owned()));
    while let Ok(bytes) = receiver.receive() {
        let Ok(ClientMessage::Command(line)) = ClientMessage::decode(&bytes) else {
            return;
        };
        send(&mut sender, &ServerMessage::Text(Message::game("tick")));
        send(&mut sender, &ServerMessage::Refresh("% ".to_owned()));
        send(
            &mut sender,
            &ServerMessage::Text(Message::game(format!("echo {line}"))),
        );
        send(&mut sender, &ServerMessage::Prompt("$ ".to_owned()));
    }
}

pub(crate) fn echo_wire() -> Wire {
    let (to_client, from_server) = mpsc::channel();
    let (to_server, from_client) = mpsc::channel();
    let welcome = ServerMessage::Welcome {
        city: "Test City".to_owned(),
        name: "anon-test".to_owned(),
    };
    for message in [welcome, ServerMessage::Prompt("$ ".to_owned())] {
        let _ = to_client.send(message);
    }
    thread::spawn(move || {
        for message in from_client {
            if let ClientMessage::Command(line) = message {
                let echo = Message::game(format!("echo {line}"));
                let _ = to_client.send(ServerMessage::Text(echo));
                let _ = to_client.send(ServerMessage::Prompt("$ ".to_owned()));
            }
        }
    });
    Wire::new(
        move || from_server.recv().ok(),
        move |message| to_server.send(message).map_err(io::Error::other),
        || {},
    )
}

pub(crate) fn echo(mut receiver: Receiver<TcpStream>, mut sender: Sender<TcpStream>) {
    let welcome = ServerMessage::Welcome {
        city: "Test City".to_owned(),
        name: "anon-test".to_owned(),
    };
    send(&mut sender, &welcome);
    send(&mut sender, &ServerMessage::Text(Message::game("Hello.")));
    send(&mut sender, &ServerMessage::Prompt("$ ".to_owned()));
    while let Ok(bytes) = receiver.receive() {
        let Ok(ClientMessage::Command(line)) = ClientMessage::decode(&bytes) else {
            return;
        };
        if line == "close" {
            send(&mut sender, &ServerMessage::Close("Bye.".to_owned()));
            while receiver.receive().is_ok() {}
            return;
        }
        send(
            &mut sender,
            &ServerMessage::Text(Message::game(format!("echo {line}"))),
        );
        send(&mut sender, &ServerMessage::Prompt("$ ".to_owned()));
    }
}
