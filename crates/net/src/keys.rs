use std::fmt::{self, Write as _};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;

use snow::params::NoiseParams;

use crate::Error;

const KEY_LENGTH: usize = 32;
const HEADER: &str = "ttymud key 1";
const PATTERN: &str = "Noise_XX_25519_ChaChaPoly_BLAKE2s";

pub(crate) fn params() -> Result<NoiseParams, Error> {
    PATTERN.parse().map_err(Error::Noise)
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PublicKey([u8; KEY_LENGTH]);

impl PublicKey {
    #[must_use]
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        <[u8; KEY_LENGTH]>::try_from(bytes).ok().map(Self)
    }

    #[must_use]
    pub fn from_hex(text: &str) -> Option<Self> {
        Self::from_bytes(&decode_hex(text)?)
    }

    #[must_use]
    pub fn to_hex(&self) -> String {
        encode_hex(&self.0)
    }

    #[must_use]
    pub fn fingerprint(&self) -> String {
        let groups: Vec<String> = self.0[..8].chunks(2).map(encode_hex).collect();
        groups.join(":")
    }
}

impl fmt::Debug for PublicKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "PublicKey({})", self.to_hex())
    }
}

#[derive(Clone)]
pub struct Keys {
    private: [u8; KEY_LENGTH],
    public: PublicKey,
}

impl Keys {
    pub fn generate() -> Result<Self, Error> {
        let pair = snow::Builder::new(params()?).generate_keypair()?;
        Self::from_parts(&pair.private, &pair.public)
    }

    fn from_parts(private: &[u8], public: &[u8]) -> Result<Self, Error> {
        let private = <[u8; KEY_LENGTH]>::try_from(private).map_err(|_| Error::BadKey)?;
        let public = PublicKey::from_bytes(public).ok_or(Error::BadKey)?;
        Ok(Self { private, public })
    }

    #[must_use]
    pub fn public(&self) -> PublicKey {
        self.public
    }

    pub(crate) fn private(&self) -> &[u8] {
        &self.private
    }

    #[must_use]
    pub fn to_text(&self) -> String {
        let private = encode_hex(&self.private);
        let public = self.public.to_hex();
        format!("{HEADER}\nprivate = {private}\npublic = {public}\n")
    }

    pub fn load(path: &Path) -> Result<Self, Error> {
        Self::from_text(&fs::read_to_string(path)?)
    }

    pub fn save_new(&self, path: &Path) -> Result<(), Error> {
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(path)?;
        file.write_all(self.to_text().as_bytes())?;
        file.sync_all()?;
        Ok(())
    }

    pub fn from_text(text: &str) -> Result<Self, Error> {
        let text = text.strip_prefix('\u{feff}').unwrap_or(text);
        let mut lines = text.lines().map(str::trim);
        if lines.next() != Some(HEADER) {
            return Err(Error::BadKey);
        }
        let private = field(lines.next(), "private")?;
        let public = field(lines.next(), "public")?;
        Self::from_parts(&private, &public)
    }
}

fn field(line: Option<&str>, name: &str) -> Result<Vec<u8>, Error> {
    let value = line
        .and_then(|line| line.strip_prefix(name))
        .and_then(|rest| rest.trim_start().strip_prefix('='))
        .ok_or(Error::BadKey)?;
    decode_hex(value.trim()).ok_or(Error::BadKey)
}

pub(crate) fn encode_hex(bytes: &[u8]) -> String {
    let mut text = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        let _ = write!(text, "{byte:02x}");
    }
    text
}

fn decode_hex(text: &str) -> Option<Vec<u8>> {
    let digits: Vec<u8> = text
        .chars()
        .map(|c| c.to_digit(16).and_then(|digit| u8::try_from(digit).ok()))
        .collect::<Option<_>>()?;
    if !digits.len().is_multiple_of(2) {
        return None;
    }
    Some(
        digits
            .chunks(2)
            .map(|pair| (pair[0] << 4) | pair[1])
            .collect(),
    )
}

impl fmt::Debug for Keys {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Keys")
            .field("public", &self.public)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::{Keys, PublicKey, encode_hex};

    #[test]
    fn generated_keys_differ() {
        let (first, second) = (Keys::generate().unwrap(), Keys::generate().unwrap());
        assert_ne!(first.public(), second.public());
        assert_ne!(first.private(), second.private());
    }

    #[test]
    fn keys_survive_a_round_trip_through_text() {
        let keys = Keys::generate().unwrap();
        let copy = Keys::from_text(&keys.to_text()).unwrap();
        assert_eq!(copy.public(), keys.public());
        assert_eq!(copy.private(), keys.private());
    }

    #[test]
    fn key_text_has_a_header_and_two_hex_lines() {
        let text = Keys::generate().unwrap().to_text();
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0], "ttymud key 1");
        assert!(lines[1].starts_with("private = "));
        assert!(lines[2].starts_with("public = "));
    }

    #[test]
    fn bad_key_text_fails() {
        let good = Keys::generate().unwrap().to_text();
        let short = good.replace("public = ", "public = 00");
        for text in [
            "",
            "ttymud key 1\n",
            &good.replace("ttymud key 1", "other"),
            &short,
        ] {
            assert!(Keys::from_text(text).is_err(), "{text:?}");
        }
    }

    fn scratch(name: &str) -> std::path::PathBuf {
        let id = std::process::id();
        let path = std::env::temp_dir().join(format!("ttymud-net-{id}-{name}.key"));
        let _ = std::fs::remove_file(&path);
        path
    }

    #[test]
    fn keys_survive_a_round_trip_through_a_file() {
        let path = scratch("round-trip");
        let keys = Keys::generate().unwrap();
        keys.save_new(&path).unwrap();
        let loaded = Keys::load(&path).unwrap();
        let _ = std::fs::remove_file(&path);
        assert_eq!(loaded.public(), keys.public());
        assert_eq!(loaded.private(), keys.private());
    }

    #[test]
    fn saving_never_replaces_a_key_file() {
        let path = scratch("existing");
        Keys::generate().unwrap().save_new(&path).unwrap();
        let second = Keys::generate().unwrap().save_new(&path);
        let _ = std::fs::remove_file(&path);
        let error = std::io::Error::from(second.unwrap_err());
        assert_eq!(error.kind(), std::io::ErrorKind::AlreadyExists);
    }

    #[test]
    fn a_missing_key_file_is_not_found() {
        let error = std::io::Error::from(Keys::load(&scratch("missing")).unwrap_err());
        assert_eq!(error.kind(), std::io::ErrorKind::NotFound);
    }

    #[cfg(unix)]
    #[test]
    fn only_the_owner_can_read_a_key_file() {
        use std::os::unix::fs::PermissionsExt;
        let path = scratch("mode");
        Keys::generate().unwrap().save_new(&path).unwrap();
        let mode = std::fs::metadata(&path).unwrap().permissions().mode();
        let _ = std::fs::remove_file(&path);
        assert_eq!(mode & 0o777, 0o600);
    }

    #[test]
    fn public_keys_round_trip_through_hex() {
        let key = Keys::generate().unwrap().public();
        assert_eq!(PublicKey::from_hex(&key.to_hex()), Some(key));
        assert_eq!(key.to_hex().len(), 64);
        assert_eq!(PublicKey::from_hex("zz"), None);
        assert_eq!(PublicKey::from_hex("ab"), None);
    }

    #[test]
    fn a_fingerprint_is_the_start_of_the_key_in_four_groups() {
        let key = PublicKey([0xab; 32]);
        assert_eq!(key.fingerprint(), "abab:abab:abab:abab");
    }

    #[test]
    fn debug_output_hides_the_private_key() {
        let keys = Keys::generate().unwrap();
        let private = encode_hex(keys.private());
        assert!(!format!("{keys:?}").contains(&private));
    }
}
