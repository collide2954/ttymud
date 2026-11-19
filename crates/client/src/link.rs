use std::fmt;
use std::io;
use std::net::{TcpStream, ToSocketAddrs};
use std::path::Path;
use std::time::Duration;

use ttymud_net::{Keys, PublicKey, Receiver, Sender};

use crate::known::{KnownServers, Trust, normalize};

const TIMEOUT: Duration = Duration::from_secs(10);

pub struct Link {
    pub receiver: Receiver<TcpStream>,
    pub sender: Sender<TcpStream>,
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
}

impl fmt::Display for ConnectError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "{error}"),
            Self::NotTrusted => f.write_str("you did not trust the server"),
            Self::KeyChanged { address, .. } => write!(f, "the key of {address} changed"),
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
    ask: impl FnOnce(&str, PublicKey) -> io::Result<bool>,
) -> Result<Link, ConnectError> {
    let address = normalize(address);
    let target = address.to_socket_addrs()?.next().ok_or_else(|| {
        io::Error::new(io::ErrorKind::NotFound, format!("no address for {address}"))
    })?;
    let mut stream = TcpStream::connect_timeout(&target, TIMEOUT)?;
    stream.set_read_timeout(Some(TIMEOUT))?;
    let session = ttymud_net::connect(&mut stream, keys)?;
    stream.set_read_timeout(None)?;
    let server = session.peer();
    let mut known = KnownServers::load(known_servers)?;
    match known.check(&address, server) {
        Trust::Known => {}
        Trust::New => {
            if !ask(&address, server)? {
                return Err(ConnectError::NotTrusted);
            }
            known.add(&address, server);
            known.save(known_servers)?;
        }
        Trust::Changed(expected) => {
            return Err(ConnectError::KeyChanged {
                address,
                expected,
                found: server,
            });
        }
    }
    let (receiver, sender) = session.split(stream.try_clone()?, stream);
    Ok(Link {
        receiver,
        sender,
        server,
    })
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::fs;
    use std::net::{SocketAddr, TcpListener};
    use std::path::PathBuf;
    use std::thread;
    use std::time::Duration;

    use ttymud_net::{Keys, PublicKey};
    use ttymud_wire::ServerMessage;

    use super::{ConnectError, connect};
    use crate::KnownServers;

    fn server() -> (SocketAddr, PublicKey) {
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
                    let (_, mut sender) = session.split(stream.try_clone().unwrap(), stream);
                    let _ = sender.send(&ServerMessage::Prompt("$ ".to_owned()).encode());
                    thread::sleep(Duration::from_secs(2));
                });
            }
        });
        (address, public)
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
        let link = connect(&address.to_string(), &keys(), &path, |_, key| {
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
    fn a_known_server_is_not_asked_about_again() {
        let (address, _) = server();
        let path = scratch("known");
        let keys = keys();
        connect(&address.to_string(), &keys, &path, |_, _| Ok(true)).unwrap();
        let second = connect(&address.to_string(), &keys, &path, |_, _| {
            panic!("asked twice")
        });
        let _ = fs::remove_file(&path);
        assert!(second.is_ok());
    }

    #[test]
    fn a_no_refuses_the_server_and_saves_nothing() {
        let (address, _) = server();
        let path = scratch("no");
        let result = connect(&address.to_string(), &keys(), &path, |_, _| Ok(false));
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
        let result = connect(&address.to_string(), &keys(), &path, |_, _| {
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
    fn the_link_carries_server_messages() {
        let (address, _) = server();
        let path = scratch("messages");
        let mut link = connect(&address.to_string(), &keys(), &path, |_, _| Ok(true)).unwrap();
        let _ = fs::remove_file(&path);
        let message = ServerMessage::decode(&link.receiver.receive().unwrap()).unwrap();
        assert_eq!(message, ServerMessage::Prompt("$ ".to_owned()));
    }
}
