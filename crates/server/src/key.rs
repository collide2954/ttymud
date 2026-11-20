use std::fmt::Write as _;

use ttymud_net::PublicKey;

const SSH: &str = "ssh-";
const BASE64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AccountKey {
    Noise(PublicKey),
    #[cfg_attr(not(feature = "ssh"), allow(dead_code))]
    Ssh([u8; 32]),
}

impl From<PublicKey> for AccountKey {
    fn from(key: PublicKey) -> Self {
        Self::Noise(key)
    }
}

impl AccountKey {
    pub(crate) fn id(&self) -> String {
        match self {
            Self::Noise(key) => key.to_hex(),
            Self::Ssh(digest) => format!("{SSH}{}", hex(digest)),
        }
    }

    pub(crate) fn anon(&self) -> String {
        let id = self.id();
        let tail = id.strip_prefix(SSH).unwrap_or(&id);
        format!("anon-{}", &tail[..8])
    }
}

pub(crate) fn fingerprint(id: &str) -> String {
    if let Some(digest) = id.strip_prefix(SSH).and_then(unhex) {
        return format!("SHA256:{}", base64(&digest));
    }
    PublicKey::from_hex(id).map_or_else(|| id.to_owned(), |key| key.fingerprint())
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().fold(String::new(), |mut text, byte| {
        let _ = write!(text, "{byte:02x}");
        text
    })
}

fn unhex(text: &str) -> Option<Vec<u8>> {
    if !text.len().is_multiple_of(2) {
        return None;
    }
    (0..text.len())
        .step_by(2)
        .map(|at| u8::from_str_radix(text.get(at..at + 2)?, 16).ok())
        .collect()
}

fn base64(bytes: &[u8]) -> String {
    let mut text = String::new();
    for chunk in bytes.chunks(3) {
        let byte = |at: usize| u32::from(chunk.get(at).copied().unwrap_or(0));
        let group = byte(0) << 16 | byte(1) << 8 | byte(2);
        for at in 0..=chunk.len() {
            let index = (group >> (18 - 6 * at)) & 63;
            text.push(char::from(BASE64[usize::try_from(index).unwrap_or(0)]));
        }
    }
    text
}

#[cfg(test)]
mod tests {
    use ttymud_net::Keys;

    use super::{AccountKey, base64, fingerprint};

    #[test]
    fn base64_has_no_padding_as_in_openssh() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"M"), "TQ");
        assert_eq!(base64(b"Ma"), "TWE");
        assert_eq!(base64(b"Man"), "TWFu");
        assert_eq!(base64(&[0xfb, 0xff]), "+/8");
    }

    #[test]
    fn an_ssh_key_shows_a_sha256_fingerprint_as_ssh_keygen_does() {
        let key = AccountKey::Ssh([7; 32]);
        assert_eq!(key.id(), format!("ssh-{}", "07".repeat(32)));
        assert_eq!(key.anon(), "anon-07070707");
        assert_eq!(
            fingerprint(&key.id()),
            format!("SHA256:{}Bwc", "BwcH".repeat(10))
        );
    }

    #[test]
    fn a_noise_key_keeps_its_hex_id_and_its_fingerprint() {
        let public = Keys::generate().unwrap().public();
        let key = AccountKey::from(public);
        assert_eq!(key.id(), public.to_hex());
        assert_eq!(key.anon(), format!("anon-{}", &public.to_hex()[..8]));
        assert_eq!(fingerprint(&key.id()), public.fingerprint());
        assert_eq!(fingerprint("ssh-zz"), "ssh-zz");
    }
}
