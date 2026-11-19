use std::net::{SocketAddr, TcpListener, TcpStream};
use std::thread;

use ttymud_net::{Keys, PublicKey, Receiver, Sender};
use ttymud_text::Message;
use ttymud_wire::{ClientMessage, ServerMessage};

type Handler = fn(Receiver<TcpStream>, Sender<TcpStream>);

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

pub(crate) fn send(sender: &mut Sender<TcpStream>, message: &ServerMessage) {
    let _ = sender.send(&message.encode());
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
