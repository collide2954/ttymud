use std::net::{Shutdown, TcpStream};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use ttymud_net::{Error, Keys, Sender};
use ttymud_text::Message;
use ttymud_wire::{ClientMessage, ServerMessage};

use crate::game::Event;
use crate::limits::Limiter;

const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);
const WRITE_TIMEOUT: Duration = Duration::from_secs(10);

pub(crate) fn run(stream: TcpStream, connection: u64, keys: &Keys, events: &mpsc::Sender<Event>) {
    if let Err(error) = serve(stream, connection, keys, events) {
        eprintln!("ttymud-server: connection {connection}: {error}");
    }
}

fn serve(
    mut stream: TcpStream,
    connection: u64,
    keys: &Keys,
    events: &mpsc::Sender<Event>,
) -> Result<(), Error> {
    stream.set_read_timeout(Some(HANDSHAKE_TIMEOUT))?;
    stream.set_write_timeout(Some(WRITE_TIMEOUT))?;
    let session = ttymud_net::accept(&mut stream, keys)?;
    stream.set_read_timeout(None)?;
    let key = session.peer();
    let control = stream.try_clone()?;
    let (mut receiver, sender) = session.split(stream.try_clone()?, stream);
    let (outbox, mailbox) = mpsc::channel();
    let warnings = outbox.clone();
    let writer = thread::Builder::new()
        .name(format!("writer-{connection}"))
        .spawn(move || write_loop(sender, &mailbox, &control))?;
    let joined = Event::Join {
        connection,
        key,
        outbox,
    };
    if events.send(joined).is_ok() {
        let mut limiter = Limiter::new(Instant::now());
        while let Ok(bytes) = receiver.receive() {
            let Ok(ClientMessage::Command(line)) = ClientMessage::decode(&bytes) else {
                break;
            };
            if let Some(warning) = limiter.check(&line, Instant::now()) {
                let _ = warnings.send(ServerMessage::Text(Message::error(warning)));
                continue;
            }
            if events.send(Event::Command { connection, line }).is_err() {
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
        if sender.send(&message.encode()).is_err() || close {
            break;
        }
    }
    let _ = control.shutdown(Shutdown::Both);
}
