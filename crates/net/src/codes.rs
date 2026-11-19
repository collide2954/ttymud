use snow::error::InitStage;
use snow::params::HashChoice;
use snow::resolvers::{CryptoResolver, DefaultResolver};

use crate::Error;
use crate::keys::encode_hex;

const DIGEST_LENGTH: usize = 32;

pub fn random_code(bytes: usize) -> Result<String, Error> {
    let mut random = DefaultResolver
        .resolve_rng()
        .ok_or(snow::Error::Init(InitStage::GetRngImpl))?;
    let mut buffer = vec![0; bytes];
    random.try_fill_bytes(&mut buffer)?;
    let groups: Vec<String> = buffer.chunks(2).map(encode_hex).collect();
    Ok(groups.join("-"))
}

pub fn digest(code: &str) -> Result<String, Error> {
    let normal: String = code
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|c| c.to_ascii_lowercase())
        .collect();
    let mut hash = DefaultResolver
        .resolve_hash(&HashChoice::Blake2s)
        .ok_or(snow::Error::Init(InitStage::GetHashImpl))?;
    hash.input(normal.as_bytes());
    let mut out = [0; DIGEST_LENGTH];
    hash.result(&mut out);
    Ok(encode_hex(&out))
}

#[cfg(test)]
mod tests {
    use super::{digest, random_code};

    #[test]
    fn a_random_code_is_hex_in_groups_of_four() {
        let code = random_code(8).unwrap();
        assert_eq!(code.len(), 19);
        let groups: Vec<&str> = code.split('-').collect();
        assert_eq!(groups.len(), 4);
        assert!(
            groups
                .iter()
                .all(|group| group.len() == 4 && group.chars().all(|c| c.is_ascii_hexdigit()))
        );
        assert_ne!(random_code(8).unwrap(), code);
    }

    #[test]
    fn a_digest_is_blake2s_of_the_code_without_case_or_dashes() {
        assert_eq!(
            digest("abc").unwrap(),
            "508c5e8c327c14e2e1a72ba34eeb452f37458b209ed63a294d999b4c86675982"
        );
        assert_eq!(digest("A-B C").unwrap(), digest("abc").unwrap());
    }
}
