use std::fs;
use std::io;
use std::path::Path;

use ttymud_net::{PublicKey, is_address, is_server_name};
use ttymud_text::ini::{self, Error, Section};

pub(crate) const LINKS: &str = "links.ini";
const MAX_PEERS: usize = 16;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Peer {
    pub name: String,
    pub address: String,
    pub key: PublicKey,
}

pub fn load_peers(dir: &Path, own: PublicKey) -> io::Result<Vec<Peer>> {
    let path = dir.join(LINKS);
    if !path.is_file() {
        return Ok(Vec::new());
    }
    read_peers(&fs::read_to_string(path)?, own).map_err(|errors| {
        let lines: Vec<String> = errors
            .iter()
            .map(|error| format!("{LINKS}: {error}"))
            .collect();
        io::Error::other(lines.join("\n"))
    })
}

fn read_peers(text: &str, own: PublicKey) -> Result<Vec<Peer>, Vec<Error>> {
    let (sections, mut errors) = ini::read(text);
    let mut peers: Vec<Peer> = Vec::new();
    for section in &sections {
        let peer = match read_peer(section) {
            Ok(peer) => peer,
            Err(error) => {
                errors.push(error);
                continue;
            }
        };
        let name = &peer.name;
        let twin = peers.iter().find(|known| known.key == peer.key);
        let problem = if peer.key == own {
            Some(format!("link {name} has the key of this server"))
        } else if peers.iter().any(|known| &known.name == name) {
            Some(format!("link {name} is in the file more than once"))
        } else {
            twin.map(|twin| format!("link {name} has the same key as link {}", twin.name))
        };
        match problem {
            Some(message) => errors.push(Error {
                line: section.line,
                message,
            }),
            None => peers.push(peer),
        }
    }
    if peers.len() > MAX_PEERS {
        let count = peers.len();
        let message = format!("too many links: {count} (the maximum is {MAX_PEERS})");
        errors.push(Error { line: 0, message });
    }
    if errors.is_empty() {
        Ok(peers)
    } else {
        Err(errors)
    }
}

fn read_peer(section: &Section<'_>) -> Result<Peer, Error> {
    let (name, line) = (section.id, section.line);
    let error = |line, message| Error { line, message };
    if section.kind != "link" {
        let kind = section.kind;
        return Err(error(
            line,
            format!("unknown section type: {kind} (use link)"),
        ));
    }
    if !is_server_name(name) {
        return Err(error(
            line,
            format!("bad link name: {name} (use 3 to 16 of a-z, 0-9, and -, but not localhost)"),
        ));
    }
    if let Some(unknown) = section.unknown(&["address", "key"]) {
        return Err(unknown);
    }
    let (address, key) = (section.single("address")?, section.single("key")?);
    if !is_address(address.value) {
        let message = format!("bad address for link {name}: {}", address.value);
        return Err(error(address.line, message));
    }
    let Some(public) = PublicKey::from_hex(key.value) else {
        let message = format!("bad key for link {name} (use the Key line of the other server)");
        return Err(error(key.line, message));
    };
    Ok(Peer {
        name: name.to_owned(),
        address: address.value.to_owned(),
        key: public,
    })
}

#[cfg(test)]
mod tests {
    use std::fs;

    use ttymud_net::PublicKey;

    use super::{Peer, load_peers, read_peers};
    use crate::testing::scratch;

    fn key(byte: u8) -> PublicKey {
        PublicKey::from_hex(&format!("{byte:02x}").repeat(32)).unwrap()
    }

    fn entry(name: &str, key: PublicKey) -> String {
        let key = key.to_hex();
        format!("[link {name}]\naddress = {name}.example.net:7400\nkey = {key}\n")
    }

    fn peers(text: &str) -> Result<Vec<Peer>, Vec<String>> {
        let found = read_peers(text, key(9));
        found.map_err(|errors| errors.iter().map(ToString::to_string).collect())
    }

    #[test]
    fn a_link_has_a_name_an_address_and_a_key() {
        let owl = Peer {
            name: "night-owl".to_owned(),
            address: "night-owl.example.net:7400".to_owned(),
            key: key(1),
        };
        assert_eq!(peers(&entry("night-owl", key(1))), Ok(vec![owl]));
        assert_eq!(peers("# No links.\n"), Ok(Vec::new()));
    }

    #[test]
    fn bad_links_fail_with_a_line_and_a_reason() {
        let one = entry("dock", key(1));
        let cases = [
            (
                "[server dock]\n".to_owned(),
                "line 1: unknown section type: server (use link)",
            ),
            (
                entry("Dock", key(1)),
                "line 1: bad link name: Dock (use 3 to 16 of a-z, 0-9, and -, but not localhost)",
            ),
            (
                one.clone() + "city = x\n",
                "line 4: unknown key in link dock: city",
            ),
            (
                one.replace("address = dock.example.net:7400\n", ""),
                "line 1: link dock has no address",
            ),
            (
                one.replace("dock.example.net:7400", "a b"),
                "line 2: bad address for link dock: a b",
            ),
            (
                one.replace(&key(1).to_hex(), "12"),
                "line 3: bad key for link dock (use the Key line of the other server)",
            ),
            (
                entry("dock", key(9)),
                "line 1: link dock has the key of this server",
            ),
            (
                one.clone() + &entry("dock", key(2)),
                "line 4: link dock is in the file more than once",
            ),
            (
                one.clone() + &entry("pier", key(1)),
                "line 4: link pier has the same key as link dock",
            ),
        ];
        for (text, error) in cases {
            assert_eq!(peers(&text), Err(vec![error.to_owned()]), "{text}");
        }
    }

    #[test]
    fn a_server_has_16_links_or_fewer() {
        let text: String = (0..17)
            .map(|number| entry(&format!("city-{number}"), key(number + 10)))
            .collect();
        assert_eq!(
            peers(&text),
            Err(vec!["too many links: 17 (the maximum is 16)".to_owned()])
        );
    }

    #[test]
    fn load_peers_reads_links_ini_and_names_the_file_in_errors() {
        let dir = scratch("links");
        fs::create_dir_all(&dir).unwrap();
        assert_eq!(load_peers(&dir, key(9)).unwrap(), Vec::new());
        fs::write(dir.join("links.ini"), entry("dock", key(1))).unwrap();
        let found = load_peers(&dir, key(9)).unwrap();
        fs::write(dir.join("links.ini"), "[link Dock]\n").unwrap();
        let error = load_peers(&dir, key(9)).unwrap_err().to_string();
        let _ = fs::remove_dir_all(&dir);
        assert_eq!(found.len(), 1);
        assert_eq!(
            error,
            "links.ini: line 1: bad link name: Dock (use 3 to 16 of a-z, 0-9, and -, but not localhost)"
        );
    }
}
