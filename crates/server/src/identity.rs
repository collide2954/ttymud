use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::Path;

use ttymud_game::ContentError;
use ttymud_game::ini::{self, Section};

pub(crate) const KEYS: &str = "keys.ini";
const LINK_TIME: u64 = 10 * 60;

#[derive(Debug, Default)]
pub struct Identity {
    accounts: BTreeMap<String, Keyring>,
    offers: Vec<Offer>,
    dirty: bool,
}

#[derive(Debug)]
struct Keyring {
    recovery: Option<String>,
    keys: Vec<String>,
}

#[derive(Debug)]
struct Offer {
    code: String,
    account: String,
    until: u64,
}

impl Identity {
    pub(crate) fn account_of(&self, key: &str) -> Option<String> {
        let owner = self
            .accounts
            .iter()
            .find(|(_, ring)| ring.keys.iter().any(|known| known == key));
        match owner {
            Some((account, _)) => Some(account.clone()),
            None if self.accounts.contains_key(key) => None,
            None => Some(key.to_owned()),
        }
    }

    pub(crate) fn keys(&self, account: &str) -> Vec<String> {
        self.accounts
            .get(account)
            .map_or_else(|| vec![account.to_owned()], |ring| ring.keys.clone())
    }

    pub(crate) fn has_recovery(&self, account: &str) -> bool {
        self.accounts
            .get(account)
            .is_some_and(|ring| ring.recovery.is_some())
    }

    pub(crate) fn set_recovery(&mut self, account: &str, digest: String) {
        self.ring(account).recovery = Some(digest);
        self.dirty = true;
    }

    pub(crate) fn recover(&mut self, digest: &str, key: &str) -> Option<String> {
        let account = self
            .accounts
            .iter()
            .find(|(_, ring)| ring.recovery.as_deref() == Some(digest))
            .map(|(account, _)| account.clone())?;
        self.add_key(&account, key);
        Some(account)
    }

    pub(crate) fn offer(&mut self, account: &str, digest: String, now: u64) {
        self.offers
            .retain(|offer| offer.until > now && offer.account != account);
        self.offers.push(Offer {
            code: digest,
            account: account.to_owned(),
            until: now + LINK_TIME,
        });
    }

    pub(crate) fn accept(&mut self, digest: &str, key: &str, now: u64) -> Option<String> {
        let index = self
            .offers
            .iter()
            .position(|offer| offer.code == digest && offer.until > now)?;
        let offer = self.offers.remove(index);
        self.add_key(&offer.account, key);
        Some(offer.account)
    }

    pub(crate) fn revoke(&mut self, account: &str, key: &str) {
        self.ring(account).keys.retain(|known| known != key);
        self.dirty = true;
    }

    fn add_key(&mut self, account: &str, key: &str) {
        for ring in self.accounts.values_mut() {
            ring.keys.retain(|known| known != key);
        }
        let ring = self.ring(account);
        ring.keys.push(key.to_owned());
        self.dirty = true;
    }

    fn ring(&mut self, account: &str) -> &mut Keyring {
        self.accounts
            .entry(account.to_owned())
            .or_insert_with(|| Keyring {
                recovery: None,
                keys: vec![account.to_owned()],
            })
    }

    pub(crate) fn save(&mut self) -> Option<(String, String)> {
        if !std::mem::take(&mut self.dirty) {
            return None;
        }
        let mut lines = vec!["# The keys of each account. The server writes this file.".to_owned()];
        for (account, ring) in &self.accounts {
            lines.push(String::new());
            lines.push(format!("[account {account}]"));
            lines.extend(
                ring.recovery
                    .iter()
                    .map(|digest| format!("recovery = {digest}")),
            );
            lines.extend(ring.keys.iter().map(|key| format!("key = {key}")));
        }
        lines.push(String::new());
        Some((KEYS.to_owned(), lines.join("\n")))
    }

    pub fn read(text: &str) -> Result<Self, Vec<ContentError>> {
        let (sections, mut errors) = ini::read(text);
        let mut identity = Self::default();
        for section in &sections {
            if section.kind == "account" {
                let ring = keyring(section, &mut errors);
                identity.accounts.insert(section.id.to_owned(), ring);
            } else {
                let (line, kind) = (section.line, section.kind);
                let message = format!("unknown section type: {kind}");
                errors.push(ContentError { line, message });
            }
        }
        errors.sort_by_key(|error| error.line);
        if errors.is_empty() {
            Ok(identity)
        } else {
            Err(errors)
        }
    }
}

pub fn load_identity(dir: &Path) -> io::Result<Identity> {
    let path = dir.join(KEYS);
    if !path.is_file() {
        return Ok(Identity::default());
    }
    Identity::read(&fs::read_to_string(path)?).map_err(|errors| {
        let lines: Vec<String> = errors
            .iter()
            .map(|error| format!("{KEYS}: {error}"))
            .collect();
        io::Error::other(lines.join("\n"))
    })
}

fn keyring(section: &Section<'_>, errors: &mut Vec<ContentError>) -> Keyring {
    let mut ring = Keyring {
        recovery: None,
        keys: Vec::new(),
    };
    for pair in &section.pairs {
        match pair.key {
            "recovery" => ring.recovery = Some(pair.value.to_owned()),
            "key" => ring.keys.push(pair.value.to_owned()),
            other => {
                let (line, id) = (pair.line, section.id);
                let message = format!("unknown key in account {id}: {other}");
                errors.push(ContentError { line, message });
            }
        }
    }
    ring
}

#[cfg(test)]
mod tests {
    use super::Identity;

    fn linked(identity: &mut Identity, account: &str, key: &str) {
        identity.offer(account, "code".to_owned(), 0);
        identity.accept("code", key, 1);
    }

    #[test]
    fn a_new_key_is_its_own_account() {
        let identity = Identity::default();
        assert_eq!(identity.account_of("ka"), Some("ka".to_owned()));
        assert_eq!(identity.keys("ka"), ["ka"]);
        assert!(!identity.has_recovery("ka"));
    }

    #[test]
    fn a_recovery_code_adds_a_key_to_its_account() {
        let mut identity = Identity::default();
        identity.set_recovery("ka", "d1".to_owned());
        assert!(identity.has_recovery("ka"));
        assert_eq!(identity.recover("d0", "kb"), None);
        assert_eq!(identity.recover("d1", "kb"), Some("ka".to_owned()));
        assert_eq!(identity.account_of("kb"), Some("ka".to_owned()));
        assert_eq!(identity.keys("ka"), ["ka", "kb"]);
    }

    #[test]
    fn a_link_code_works_one_time_for_10_minutes() {
        let mut identity = Identity::default();
        identity.offer("ka", "l1".to_owned(), 100);
        assert_eq!(identity.accept("l1", "kb", 700), None);
        identity.offer("ka", "l2".to_owned(), 1_000);
        identity.offer("ka", "l3".to_owned(), 1_000);
        assert_eq!(identity.accept("l2", "kb", 1_001), None);
        assert_eq!(identity.accept("l3", "kb", 1_599), Some("ka".to_owned()));
        assert_eq!(identity.accept("l3", "kc", 1_599), None);
    }

    #[test]
    fn a_revoked_key_has_no_account() {
        let mut identity = Identity::default();
        linked(&mut identity, "ka", "kb");
        identity.revoke("ka", "ka");
        assert_eq!(identity.account_of("ka"), None);
        assert_eq!(identity.account_of("kb"), Some("ka".to_owned()));
        assert_eq!(identity.keys("ka"), ["kb"]);
    }

    #[test]
    fn a_key_belongs_to_one_account() {
        let mut identity = Identity::default();
        linked(&mut identity, "ka", "kb");
        linked(&mut identity, "kc", "kb");
        assert_eq!(identity.account_of("kb"), Some("kc".to_owned()));
        assert_eq!(identity.keys("ka"), ["ka"]);
    }

    #[test]
    fn keys_come_back_from_their_file() {
        let mut identity = Identity::default();
        assert_eq!(identity.save(), None);
        identity.set_recovery("ka", "d1".to_owned());
        linked(&mut identity, "ka", "kb");
        let (path, text) = identity.save().unwrap_or_default();
        assert_eq!(path, "keys.ini");
        assert_eq!(
            text,
            "# The keys of each account. The server writes this file.\n\n\
             [account ka]\nrecovery = d1\nkey = ka\nkey = kb\n"
        );
        assert_eq!(identity.save(), None);
        let read = Identity::read(&text).unwrap();
        assert_eq!(read.account_of("kb"), Some("ka".to_owned()));
        assert!(read.has_recovery("ka"));
    }

    #[test]
    fn a_bad_keys_file_fails() {
        let errors: Vec<String> = Identity::read("[account ka]\ncolor = red\n[other x]\n")
            .unwrap_err()
            .iter()
            .map(ToString::to_string)
            .collect();
        assert_eq!(
            errors,
            [
                "line 2: unknown key in account ka: color",
                "line 3: unknown section type: other"
            ]
        );
    }
}
