use std::net::{IpAddr, Shutdown, TcpStream};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use ttymud_net::{Error, Keys, Sender};
use ttymud_text::Message;
use ttymud_wire::{ClientMessage, ServerMessage};

use crate::game::Event;
use crate::limits::Limiter;
use crate::longline;
use crate::serve::Hub;

const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);
const WRITE_TIMEOUT: Duration = Duration::from_secs(10);
const LINGER: Duration = Duration::from_secs(5);

pub(crate) fn run(stream: TcpStream, connection: u64, address: IpAddr, keys: &Keys, hub: &Hub) {
    if let Err(error) = serve(stream, connection, address, keys, hub) {
        eprintln!("ttymud-server: connection {connection}: {error}");
    }
}

fn serve(
    mut stream: TcpStream,
    connection: u64,
    address: IpAddr,
    keys: &Keys,
    hub: &Hub,
) -> Result<(), Error> {
    stream.set_read_timeout(Some(HANDSHAKE_TIMEOUT))?;
    stream.set_write_timeout(Some(WRITE_TIMEOUT))?;
    let session = ttymud_net::accept(&mut stream, keys)?;
    let key = session.peer();
    if let Some(peer) = hub.peers.iter().find(|peer| peer.key == key) {
        return Ok(longline::accept(peer, session, stream, hub)?);
    }
    stream.set_read_timeout(None)?;
    let events = &hub.events;
    let control = stream.try_clone()?;
    let (mut receiver, sender) = session.split(stream.try_clone()?, stream);
    let (outbox, mailbox) = mpsc::channel();
    let warnings = outbox.clone();
    let writer = thread::Builder::new()
        .name(format!("writer-{connection}"))
        .spawn(move || write_loop(sender, &mailbox, &control))?;
    let joined = Event::Join {
        connection,
        key: key.into(),
        address,
        outbox,
    };
    if events.send(joined).is_ok() {
        let mut limiter = Limiter::new(Instant::now());
        while let Ok(bytes) = receiver.receive() {
            let event = match ClientMessage::decode(&bytes) {
                Ok(ClientMessage::Command(line)) => {
                    if let Some(warning) = limiter.check(&line, Instant::now()) {
                        let _ = warnings.send(ServerMessage::Text(Message::error(warning)));
                        continue;
                    }
                    Event::Command { connection, line }
                }
                Ok(ClientMessage::Complete(text)) => {
                    if !limiter.completion(Instant::now()) {
                        continue;
                    }
                    Event::Complete { connection, text }
                }
                Err(_) => break,
            };
            if events.send(event).is_err() {
                break;
            }
        }
        let _ = events.send(Event::Leave { connection });
    }
    drop(warnings);
    let _ = writer.join();
    Ok(())
}

fn write_loop(
    mut sender: Sender<TcpStream>,
    mailbox: &mpsc::Receiver<ServerMessage>,
    control: &TcpStream,
) {
    for message in mailbox {
        let close = matches!(message, ServerMessage::Close(_));
        if sender.send(&message.encode()).is_err() {
            break;
        }
        if close {
            let _ = control.shutdown(Shutdown::Write);
            linger(mailbox);
            break;
        }
    }
    let _ = control.shutdown(Shutdown::Both);
}

fn linger(mailbox: &mpsc::Receiver<ServerMessage>) {
    let deadline = Instant::now() + LINGER;
    while let Some(left) = deadline.checked_duration_since(Instant::now()) {
        if matches!(
            mailbox.recv_timeout(left),
            Err(mpsc::RecvTimeoutError::Disconnected)
        ) {
            return;
        }
    }
}
