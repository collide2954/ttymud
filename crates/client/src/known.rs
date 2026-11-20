use std::fmt::Write as _;
use std::fs;
use std::io;
use std::path::Path;

use ttymud_net::PublicKey;

pub const DEFAULT_PORT: u16 = 7331;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Trust {
    Known,
    New,
    Changed(PublicKey),
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct KnownServers {
    entries: Vec<(String, PublicKey)>,
}

#[must_use]
pub fn normalize(address: &str) -> String {
    let address = address.trim().to_ascii_lowercase();
    if address.starts_with('[') {
        if address.contains("]:") {
            address
        } else {
            format!("{address}:{DEFAULT_PORT}")
        }
    } else if address.matches(':').count() > 1 {
        format!("[{address}]:{DEFAULT_PORT}")
    } else if address.contains(':') {
        address
    } else {
        format!("{address}:{DEFAULT_PORT}")
    }
}

impl KnownServers {
    #[must_use]
    pub fn parse(text: &str) -> Self {
        let entries = text
            .lines()
            .filter_map(|line| {
                let (address, key) = line.trim().split_once(' ')?;
                Some((address.to_owned(), PublicKey::from_hex(key.trim())?))
            })
            .collect();
        Self { entries }
    }

    #[must_use]
    pub fn check(&self, address: &str, key: PublicKey) -> Trust {
        match self.entries.iter().find(|(known, _)| known == address) {
            None => Trust::New,
            Some((_, known)) if *known == key => Trust::Known,
            Some((_, known)) => Trust::Changed(*known),
        }
    }

    pub fn add(&mut self, address: &str, key: PublicKey) {
        self.entries.push((address.to_owned(), key));
    }

    pub fn set(&mut self, address: &str, key: PublicKey) {
        self.entries.retain(|(known, _)| known != address);
        self.add(address, key);
    }

    #[must_use]
    pub fn to_text(&self) -> String {
        let mut text = String::new();
        for (address, key) in &self.entries {
            let _ = writeln!(text, "{address} {}", key.to_hex());
        }
        text
    }

    pub fn load(path: &Path) -> io::Result<Self> {
        match fs::read_to_string(path) {
            Ok(text) => Ok(Self::parse(&text)),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Self::default()),
            Err(error) => Err(error),
        }
    }

    pub fn save(&self, path: &Path) -> io::Result<()> {
        let temporary = path.with_extension("tmp");
        fs::write(&temporary, self.to_text())?;
        fs::rename(&temporary, path)
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use ttymud_net::{Keys, PublicKey};

    use super::{KnownServers, Trust, normalize};

    fn key() -> PublicKey {
        Keys::generate().unwrap().public()
    }

    #[test]
    fn addresses_get_the_default_port() {
        assert_eq!(normalize("Ostra.Example.net"), "ostra.example.net:7331");
        assert_eq!(normalize(" 127.0.0.1 "), "127.0.0.1:7331");
        assert_eq!(
            normalize("ostra.example.net:4000"),
            "ostra.example.net:4000"
        );
    }

    #[test]
    fn ipv6_addresses_get_brackets_and_a_port() {
        assert_eq!(normalize("::1"), "[::1]:7331");
        assert_eq!(normalize("[::1]"), "[::1]:7331");
        assert_eq!(normalize("[::1]:4000"), "[::1]:4000");
    }

    #[test]
    fn a_server_is_new_then_known() {
        let (address, key) = ("ostra.example.net:7331", key());
        let mut known = KnownServers::default();
        assert_eq!(known.check(address, key), Trust::New);
        known.add(address, key);
        assert_eq!(known.check(address, key), Trust::Known);
    }

    #[test]
    fn a_changed_key_is_reported_with_the_old_key() {
        let (address, old) = ("ostra.example.net:7331", key());
        let mut known = KnownServers::default();
        known.add(address, old);
        assert_eq!(known.check(address, key()), Trust::Changed(old));
    }

    #[test]
    fn known_servers_survive_a_round_trip_through_text() {
        let mut known = KnownServers::default();
        known.add("a.example:7331", key());
        known.add("[::1]:7331", key());
        assert_eq!(KnownServers::parse(&known.to_text()), known);
    }

    #[test]
    fn bad_lines_are_skipped() {
        let key = key();
        let text = format!(
            "# notes\nbroken\nb.example:7331 zz\na.example:7331 {}\n",
            key.to_hex()
        );
        let known = KnownServers::parse(&text);
        assert_eq!(known.check("a.example:7331", key), Trust::Known);
        assert_eq!(known.entries.len(), 1);
    }

    #[test]
    fn a_missing_file_means_no_known_servers() {
        let path =
            std::env::temp_dir().join(format!("ttymud-known-missing-{}", std::process::id()));
        assert_eq!(KnownServers::load(&path).unwrap(), KnownServers::default());
    }

    #[test]
    fn known_servers_survive_a_round_trip_through_a_file() {
        let path = std::env::temp_dir().join(format!("ttymud-known-{}", std::process::id()));
        let mut known = KnownServers::default();
        known.add("a.example:7331", key());
        known.save(&path).unwrap();
        let loaded = KnownServers::load(&path).unwrap();
        let _ = fs::remove_file(&path);
        assert_eq!(loaded, known);
    }
}
