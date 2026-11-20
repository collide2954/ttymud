use std::fmt;
use std::io;
use std::net::{TcpStream, ToSocketAddrs};
use std::path::Path;
use std::time::Duration;

use ttymud_net::{Keys, PublicKey, Receiver, Sender, normalize};

use crate::known::{KnownServers, Trust};

const TIMEOUT: Duration = Duration::from_secs(10);

pub struct Link {
    pub receiver: Receiver<TcpStream>,
    pub sender: Sender<TcpStream>,
    pub control: TcpStream,
    pub server: PublicKey,
}

#[derive(Debug)]
pub enum ConnectError {
    Io(io::Error),
    NotTrusted,
    KeyChanged {
        address: String,
        expected: PublicKey,
        found: PublicKey,
    },
    WrongKey {
        address: String,
        expected: PublicKey,
        found: PublicKey,
    },
}

impl fmt::Display for ConnectError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "{error}"),
            Self::NotTrusted => f.write_str("you did not trust the server"),
            Self::KeyChanged { address, .. } => write!(f, "the key of {address} changed"),
            Self::WrongKey { address, .. } => {
                write!(f, "the key of {address} is not the expected key")
            }
        }
    }
}

impl std::error::Error for ConnectError {}

impl From<io::Error> for ConnectError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<ttymud_net::Error> for ConnectError {
    fn from(error: ttymud_net::Error) -> Self {
        Self::Io(error.into())
    }
}

pub fn connect(
    address: &str,
    keys: &Keys,
    known_servers: &Path,
    given: Option<PublicKey>,
    ask: impl FnOnce(&str, PublicKey) -> io::Result<bool>,
) -> Result<Link, ConnectError> {
    let address = normalize(address);
    let mut stream = open(&address)?;
    stream.set_read_timeout(Some(TIMEOUT))?;
    let session = ttymud_net::connect(&mut stream, keys)?;
    stream.set_read_timeout(None)?;
    let server = session.peer();
    let mut known = KnownServers::load(known_servers)?;
    match (given, known.check(&address, server)) {
        (Some(expected), _) if expected != server => {
            return Err(ConnectError::WrongKey {
                address,
                expected,
                found: server,
            });
        }
        (_, Trust::Known) => {}
        (Some(_), _) => {
            known.set(&address, server);
            known.save(known_servers)?;
        }
        (None, Trust::New) => {
            if !ask(&address, server)? {
                return Err(ConnectError::NotTrusted);
            }
            known.add(&address, server);
            known.save(known_servers)?;
        }
        (None, Trust::Changed(expected)) => {
            return Err(ConnectError::KeyChanged {
                address,
                expected,
                found: server,
            });
        }
    }
    let control = stream.try_clone()?;
    let (receiver, sender) = session.split(stream.try_clone()?, stream);
    Ok(Link {
        receiver,
        sender,
        control,
        server,
    })
}

fn open(address: &str) -> io::Result<TcpStream> {
    let mut last = io::Error::new(io::ErrorKind::NotFound, format!("no address for {address}"));
    for target in address.to_socket_addrs()? {
        match TcpStream::connect_timeout(&target, TIMEOUT) {
            Ok(stream) => return Ok(stream),
            Err(error) => last = error,
        }
    }
    Err(last)
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::fs;
    use std::net::{SocketAddr, TcpStream};
    use std::path::PathBuf;
    use std::thread;
    use std::time::Duration;

    use ttymud_net::{Keys, PublicKey, Receiver, Sender};
    use ttymud_wire::ServerMessage;

    use super::{ConnectError, connect};
    use crate::KnownServers;
    use crate::testing::send;

    fn prompt(_: Receiver<TcpStream>, mut sender: Sender<TcpStream>) {
        send(&mut sender, &ServerMessage::Prompt("$ ".to_owned()));
        thread::sleep(Duration::from_secs(2));
    }

    fn server() -> (SocketAddr, PublicKey) {
        crate::testing::server(prompt)
    }

    fn scratch(name: &str) -> PathBuf {
        let id = std::process::id();
        let path = std::env::temp_dir().join(format!("ttymud-link-{id}-{name}"));
        let _ = fs::remove_file(&path);
        path
    }

    fn keys() -> Keys {
        Keys::generate().unwrap()
    }

    #[test]
    fn a_new_server_is_saved_after_a_yes() {
        let (address, server_key) = server();
        let path = scratch("yes");
        let asked = Cell::new(0);
        let link = connect(&address.to_string(), &keys(), &path, None, |_, key| {
            asked.set(asked.get() + 1);
            assert_eq!(key, server_key);
            Ok(true)
        });
        let known = KnownServers::load(&path).unwrap();
        let _ = fs::remove_file(&path);
        assert!(link.is_ok());
        assert_eq!(asked.get(), 1);
        assert_eq!(
            known.check(&address.to_string(), server_key),
            crate::Trust::Known
        );
    }

    #[test]
    fn each_address_of_a_name_gets_a_try() {
        let (address, _) = server();
        let path = scratch("localhost");
        let name = format!("localhost:{}", address.port());
        let link = connect(&name, &keys(), &path, None, |_, _| Ok(true));
        let _ = fs::remove_file(&path);
        link.unwrap();
    }

    #[test]
    fn a_known_server_is_not_asked_about_again() {
        let (address, _) = server();
        let path = scratch("known");
        let keys = keys();
        connect(&address.to_string(), &keys, &path, None, |_, _| Ok(true)).unwrap();
        let second = connect(&address.to_string(), &keys, &path, None, |_, _| {
            panic!("asked twice")
        });
        let _ = fs::remove_file(&path);
        assert!(second.is_ok());
    }

    #[test]
    fn a_no_refuses_the_server_and_saves_nothing() {
        let (address, _) = server();
        let path = scratch("no");
        let result = connect(&address.to_string(), &keys(), &path, None, |_, _| Ok(false));
        let saved = path.exists();
        assert!(matches!(result, Err(ConnectError::NotTrusted)));
        assert!(!saved);
    }

    #[test]
    fn a_changed_key_stops_the_connection() {
        let (address, server_key) = server();
        let path = scratch("changed");
        let old = Keys::generate().unwrap().public();
        let mut known = KnownServers::default();
        known.add(&address.to_string(), old);
        known.save(&path).unwrap();
        let result = connect(&address.to_string(), &keys(), &path, None, |_, _| {
            panic!("asked about a changed key")
        });
        let kept = KnownServers::load(&path).unwrap();
        let _ = fs::remove_file(&path);
        match result {
            Err(ConnectError::KeyChanged {
                expected, found, ..
            }) => {
                assert_eq!((expected, found), (old, server_key));
            }
            other => panic!("expected a changed key, got {:?}", other.err()),
        }
        assert_eq!(kept, known);
    }

    #[test]
    fn a_given_key_is_checked_and_trusted_without_a_question() {
        let (address, server_key) = server();
        let path = scratch("given");
        let link = connect(
            &address.to_string(),
            &keys(),
            &path,
            Some(server_key),
            |_, _| panic!("asked about a given key"),
        );
        let known = KnownServers::load(&path).unwrap();
        let _ = fs::remove_file(&path);
        assert!(link.is_ok());
        assert_eq!(
            known.check(&address.to_string(), server_key),
            crate::Trust::Known
        );
    }

    #[test]
    fn a_wrong_given_key_stops_the_connection_and_saves_nothing() {
        let (address, server_key) = server();
        let path = scratch("wrong");
        let other = Keys::generate().unwrap().public();
        let result = connect(&address.to_string(), &keys(), &path, Some(other), |_, _| {
            panic!("asked about a given key")
        });
        let saved = path.exists();
        match result {
            Err(ConnectError::WrongKey {
                expected, found, ..
            }) => assert_eq!((expected, found), (other, server_key)),
            other => panic!("expected a wrong key, got {:?}", other.err()),
        }
        assert!(!saved);
    }

    #[test]
    fn a_given_key_replaces_an_old_key_of_the_address() {
        let (address, server_key) = server();
        let path = scratch("rotated");
        let mut known = KnownServers::default();
        known.add(&address.to_string(), Keys::generate().unwrap().public());
        known.save(&path).unwrap();
        let link = connect(
            &address.to_string(),
            &keys(),
            &path,
            Some(server_key),
            |_, _| panic!("asked about a given key"),
        );
        let kept = KnownServers::load(&path).unwrap();
        let _ = fs::remove_file(&path);
        assert!(link.is_ok());
        assert_eq!(
            kept.check(&address.to_string(), server_key),
            crate::Trust::Known
        );
    }

    #[test]
    fn the_link_carries_server_messages() {
        let (address, _) = server();
        let path = scratch("messages");
        let mut link =
            connect(&address.to_string(), &keys(), &path, None, |_, _| Ok(true)).unwrap();
        let _ = fs::remove_file(&path);
        let message = ServerMessage::decode(&link.receiver.receive().unwrap()).unwrap();
        assert_eq!(message, ServerMessage::Prompt("$ ".to_owned()));
    }
}
