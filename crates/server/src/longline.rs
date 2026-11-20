use std::io;
use std::net::{Shutdown, TcpStream};
use std::sync::{Arc, mpsc};
use std::thread;
use std::time::{Duration, Instant};

use ttymud_net::{Keys, Receiver, Sender, Session, normalize, open};
use ttymud_text::sanitize;
use ttymud_wire::{DecodeError, LinkMessage};

use crate::game::Event;
use crate::limits::Bucket;
use crate::peers::Peer;
use crate::serve::Hub;

const TIMEOUT: Duration = Duration::from_secs(10);
const WRITE_TIMEOUT: Duration = Duration::from_secs(30);
const PING: Duration = Duration::from_secs(60);
const SILENCE: Duration = Duration::from_secs(150);
const QUEUE: usize = 64;
const FIRST_WAIT: Duration = Duration::from_secs(5);
const LONGEST_WAIT: Duration = Duration::from_secs(300);
const BURST: f64 = 40.0;
const PER_SECOND: f64 = 4.0;

type Halves = (Receiver<TcpStream>, Sender<TcpStream>, TcpStream);

pub(crate) fn start(hub: &Hub, keys: &Arc<Keys>) -> io::Result<()> {
    let own = keys.public();
    for peer in hub.peers.iter().filter(|peer| own < peer.key) {
        let (peer, keys, hub) = (peer.clone(), Arc::clone(keys), hub.clone());
        thread::Builder::new()
            .name(format!("link-{}", peer.name))
            .spawn(move || dial(&peer, &keys, &hub))?;
    }
    Ok(())
}

fn dial(peer: &Peer, keys: &Keys, hub: &Hub) {
    let mut wait = FIRST_WAIT;
    let mut reported = String::new();
    loop {
        match connect(peer, keys, hub) {
            Ok(()) => {
                wait = FIRST_WAIT;
                reported.clear();
                thread::sleep(wait);
            }
            Err(error) => {
                let text = error.to_string();
                if text != reported {
                    eprintln!("ttymud-server: link {}: {text}", peer.name);
                    reported = text;
                }
                thread::sleep(wait);
                wait = (wait * 2).min(LONGEST_WAIT);
            }
        }
    }
}

fn connect(peer: &Peer, keys: &Keys, hub: &Hub) -> io::Result<()> {
    let address = normalize(&peer.address);
    let mut stream = open(&address, TIMEOUT)?;
    stream.set_read_timeout(Some(TIMEOUT))?;
    stream.set_write_timeout(Some(WRITE_TIMEOUT))?;
    let session = ttymud_net::connect(&mut stream, keys)?;
    if session.peer() != peer.key {
        let message = format!("the server at {address} has a different key");
        return Err(io::Error::other(message));
    }
    stream.set_read_timeout(Some(SILENCE))?;
    let (mut receiver, mut sender, control) = split(session, stream)?;
    let Ok(LinkMessage::Hello { city }) = LinkMessage::decode(&receiver.receive()?) else {
        let message = format!("the server at {address} does not list this server in its links.ini");
        return Err(io::Error::other(message));
    };
    sender.send(&hello(hub))?;
    link(peer, &city, (receiver, sender, control), hub);
    Ok(())
}

pub(crate) fn accept(
    peer: &Peer,
    session: Session,
    stream: TcpStream,
    hub: &Hub,
) -> io::Result<()> {
    stream.set_read_timeout(Some(SILENCE))?;
    stream.set_write_timeout(Some(WRITE_TIMEOUT))?;
    let (mut receiver, mut sender, control) = split(session, stream)?;
    sender.send(&hello(hub))?;
    let Ok(LinkMessage::Hello { city }) = LinkMessage::decode(&receiver.receive()?) else {
        return Err(io::Error::other("the linked server sent no hello"));
    };
    link(peer, &city, (receiver, sender, control), hub);
    Ok(())
}

fn split(session: Session, stream: TcpStream) -> io::Result<Halves> {
    let control = stream.try_clone()?;
    let (receiver, sender) = session.split(stream.try_clone()?, stream);
    Ok((receiver, sender, control))
}

fn hello(hub: &Hub) -> Vec<u8> {
    let city = hub.city.to_string();
    LinkMessage::Hello { city }.encode()
}

fn link(peer: &Peer, city: &str, (mut receiver, sender, control): Halves, hub: &Hub) {
    let name = &peer.name;
    let connection = hub.connection();
    let (outbox, mailbox) = mpsc::sync_channel(QUEUE);
    let Ok(closer) = control.try_clone() else {
        return;
    };
    let spawned = thread::Builder::new()
        .name(format!("link-writer-{name}"))
        .spawn(move || write_loop(sender, &mailbox, &closer));
    let Ok(writer) = spawned else {
        return;
    };
    println!("Link {name} is up: {}.", sanitize(city));
    let up = Event::PeerUp {
        name: name.clone(),
        city: city.to_owned(),
        connection,
        outbox,
    };
    let reason = if hub.events.send(up).is_ok() {
        let reason = read_loop(name, connection, &mut receiver, hub);
        let down = Event::PeerDown {
            name: name.clone(),
            connection,
        };
        let _ = hub.events.send(down);
        reason
    } else {
        "the game stopped".to_owned()
    };
    println!("Link {name} is down: {reason}.");
    let _ = control.shutdown(Shutdown::Both);
    let _ = writer.join();
}

fn read_loop(name: &str, connection: u64, receiver: &mut Receiver<TcpStream>, hub: &Hub) -> String {
    let mut bucket = Bucket::new(BURST, PER_SECOND, Instant::now());
    loop {
        let bytes = match receiver.receive() {
            Ok(bytes) => bytes,
            Err(error) => return error.to_string(),
        };
        let message = match LinkMessage::decode(&bytes) {
            Ok(LinkMessage::Ping) | Err(DecodeError::UnknownType(_)) => continue,
            Ok(LinkMessage::Hello { .. }) => return "the server sent a second hello".to_owned(),
            Err(error) => return format!("the server sent a bad message: {error}"),
            Ok(message) => message,
        };
        if !bucket.take(Instant::now()) {
            continue;
        }
        let event = Event::PeerMessage {
            name: name.to_owned(),
            connection,
            message,
        };
        if hub.events.send(event).is_err() {
            return "the game stopped".to_owned();
        }
    }
}

fn write_loop(
    mut sender: Sender<TcpStream>,
    mailbox: &mpsc::Receiver<LinkMessage>,
    control: &TcpStream,
) {
    loop {
        let message = match mailbox.recv_timeout(PING) {
            Ok(message) => message,
            Err(mpsc::RecvTimeoutError::Timeout) => LinkMessage::Ping,
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        };
        if sender.send(&message.encode()).is_err() {
            break;
        }
    }
    let _ = control.shutdown(Shutdown::Both);
}
