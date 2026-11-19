use std::io::{Read, Write};
use std::sync::Arc;

use snow::{HandshakeState, StatelessTransportState};

use crate::keys::params;
use crate::{Error, Keys, PublicKey, frame};

const PROLOGUE: &[u8] = b"ttymud/1";
const MAX_MESSAGE: usize = 65535;
const TAG: usize = 16;

pub const MAX_PAYLOAD: usize = MAX_MESSAGE - TAG;

pub struct Session {
    transport: Arc<StatelessTransportState>,
    peer: PublicKey,
}

pub struct Sender<W> {
    transport: Arc<StatelessTransportState>,
    writer: W,
    nonce: u64,
}

pub struct Receiver<R> {
    transport: Arc<StatelessTransportState>,
    reader: R,
    nonce: u64,
}

pub fn connect(stream: &mut (impl Read + Write), keys: &Keys) -> Result<Session, Error> {
    let mut noise = builder(keys)?.build_initiator()?;
    let mut buffer = vec![0; MAX_MESSAGE];
    let length = noise.write_message(&[], &mut buffer)?;
    frame::write(stream, &buffer[..length])?;
    noise.read_message(&frame::read(stream)?, &mut buffer)?;
    let length = noise.write_message(&[], &mut buffer)?;
    frame::write(stream, &buffer[..length])?;
    Session::new(noise)
}

pub fn accept(stream: &mut (impl Read + Write), keys: &Keys) -> Result<Session, Error> {
    let mut noise = builder(keys)?.build_responder()?;
    let mut buffer = vec![0; MAX_MESSAGE];
    noise.read_message(&frame::read(stream)?, &mut buffer)?;
    let length = noise.write_message(&[], &mut buffer)?;
    frame::write(stream, &buffer[..length])?;
    noise.read_message(&frame::read(stream)?, &mut buffer)?;
    Session::new(noise)
}

fn builder(keys: &Keys) -> Result<snow::Builder<'_>, Error> {
    Ok(snow::Builder::new(params()?)
        .local_private_key(keys.private())?
        .prologue(PROLOGUE)?)
}

impl Session {
    fn new(noise: HandshakeState) -> Result<Self, Error> {
        let peer = noise
            .get_remote_static()
            .and_then(PublicKey::from_bytes)
            .ok_or(Error::NoPeerKey)?;
        let transport = noise.into_stateless_transport_mode()?;
        Ok(Self {
            transport: Arc::new(transport),
            peer,
        })
    }

    #[must_use]
    pub fn peer(&self) -> PublicKey {
        self.peer
    }

    #[must_use]
    pub fn split<R: Read, W: Write>(self, reader: R, writer: W) -> (Receiver<R>, Sender<W>) {
        let receiver = Receiver {
            transport: Arc::clone(&self.transport),
            reader,
            nonce: 0,
        };
        let sender = Sender {
            transport: self.transport,
            writer,
            nonce: 0,
        };
        (receiver, sender)
    }
}

impl<W: Write> Sender<W> {
    pub fn send(&mut self, payload: &[u8]) -> Result<(), Error> {
        if payload.len() > MAX_PAYLOAD {
            return Err(Error::TooLarge);
        }
        let mut message = vec![0; payload.len() + TAG];
        let length = self
            .transport
            .write_message(self.nonce, payload, &mut message)?;
        self.nonce = self.nonce.checked_add(1).ok_or(Error::Closed)?;
        frame::write(&mut self.writer, &message[..length])
    }
}

impl<R: Read> Receiver<R> {
    pub fn receive(&mut self) -> Result<Vec<u8>, Error> {
        let message = frame::read(&mut self.reader)?;
        let mut payload = vec![0; message.len()];
        let length = self
            .transport
            .read_message(self.nonce, &message, &mut payload)?;
        self.nonce = self.nonce.checked_add(1).ok_or(Error::Closed)?;
        payload.truncate(length);
        Ok(payload)
    }
}

#[cfg(test)]
mod tests {
    use std::io::{self, Cursor};
    use std::net::{TcpListener, TcpStream};
    use std::thread;

    use super::{MAX_PAYLOAD, Session, accept, connect};
    use crate::{Error, Keys};

    struct Pair {
        client: Session,
        server: Session,
        client_keys: Keys,
        server_keys: Keys,
        client_stream: TcpStream,
        server_stream: TcpStream,
    }

    fn pair() -> Pair {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server_keys = Keys::generate().unwrap();
        let keys = server_keys.clone();
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let session = accept(&mut stream, &keys).unwrap();
            (session, stream)
        });
        let client_keys = Keys::generate().unwrap();
        let mut client_stream = TcpStream::connect(address).unwrap();
        let client = connect(&mut client_stream, &client_keys).unwrap();
        let (server, server_stream) = server.join().unwrap();
        Pair {
            client,
            server,
            client_keys,
            server_keys,
            client_stream,
            server_stream,
        }
    }

    #[test]
    fn each_side_learns_the_key_of_the_other() {
        let pair = pair();
        assert_eq!(pair.client.peer(), pair.server_keys.public());
        assert_eq!(pair.server.peer(), pair.client_keys.public());
    }

    #[test]
    fn messages_travel_both_ways() {
        let pair = pair();
        let (mut client_in, mut client_out) = pair
            .client
            .split(pair.client_stream.try_clone().unwrap(), pair.client_stream);
        let (mut server_in, mut server_out) = pair
            .server
            .split(pair.server_stream.try_clone().unwrap(), pair.server_stream);
        client_out.send(b"hello").unwrap();
        client_out.send(b"again").unwrap();
        assert_eq!(server_in.receive().unwrap(), b"hello");
        assert_eq!(server_in.receive().unwrap(), b"again");
        server_out.send(b"welcome").unwrap();
        assert_eq!(client_in.receive().unwrap(), b"welcome");
    }

    fn sent(session: Session, messages: &[&[u8]]) -> Vec<u8> {
        let mut wire = Vec::new();
        let (_, mut sender) = session.split(io::empty(), &mut wire);
        for message in messages {
            sender.send(message).unwrap();
        }
        drop(sender);
        wire
    }

    #[test]
    fn a_changed_frame_fails() {
        let pair = pair();
        let mut wire = sent(pair.client, &[b"secret"]);
        let last = wire.len() - 1;
        wire[last] ^= 1;
        let (mut receiver, _) = pair.server.split(Cursor::new(wire), io::sink());
        assert!(matches!(receiver.receive(), Err(Error::Noise(_))));
    }

    #[test]
    fn a_replayed_frame_fails() {
        let pair = pair();
        let wire = sent(pair.client, &[b"first"]);
        let replay = [wire.clone(), wire].concat();
        let (mut receiver, _) = pair.server.split(Cursor::new(replay), io::sink());
        assert_eq!(receiver.receive().unwrap(), b"first");
        assert!(matches!(receiver.receive(), Err(Error::Noise(_))));
    }

    #[test]
    fn payloads_have_a_maximum_size() {
        let pair = pair();
        let (_, mut sender) = pair.client.split(io::empty(), io::sink());
        assert!(sender.send(&vec![0; MAX_PAYLOAD]).is_ok());
        assert!(matches!(
            sender.send(&vec![0; MAX_PAYLOAD + 1]),
            Err(Error::TooLarge)
        ));
    }

    #[test]
    fn a_closed_stream_ends_the_session() {
        let pair = pair();
        let (mut receiver, _) = pair.server.split(io::empty(), io::sink());
        assert!(matches!(receiver.receive(), Err(Error::Closed)));
    }
}
