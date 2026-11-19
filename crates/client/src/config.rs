use std::ffi::OsString;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use ttymud_net::Keys;

const IDENTITY_FILE: &str = "identity.key";

#[derive(Debug)]
pub struct Identity {
    pub keys: Keys,
    pub created: bool,
}

#[must_use]
pub fn config_dir() -> Option<PathBuf> {
    config_dir_from(|name| std::env::var_os(name), cfg!(windows))
}

fn config_dir_from(lookup: impl Fn(&str) -> Option<OsString>, windows: bool) -> Option<PathBuf> {
    let value = |name: &str| lookup(name).filter(|value| !value.is_empty());
    if let Some(dir) = value("TTYMUD_CONFIG") {
        return Some(PathBuf::from(dir));
    }
    let base = if windows {
        value("APPDATA").map(PathBuf::from)
    } else {
        value("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| value("HOME").map(|home| PathBuf::from(home).join(".config")))
    };
    base.map(|base| base.join("ttymud"))
}

pub fn identity(dir: &Path) -> io::Result<Identity> {
    fs::create_dir_all(dir)?;
    let path = dir.join(IDENTITY_FILE);
    match Keys::load(&path) {
        Ok(keys) => Ok(Identity {
            keys,
            created: false,
        }),
        Err(ttymud_net::Error::Io(error)) if error.kind() == io::ErrorKind::NotFound => {
            let keys = Keys::generate()?;
            match keys.save_new(&path) {
                Ok(()) => Ok(Identity {
                    keys,
                    created: true,
                }),
                Err(ttymud_net::Error::Io(error))
                    if error.kind() == io::ErrorKind::AlreadyExists =>
                {
                    Ok(Identity {
                        keys: Keys::load(&path)?,
                        created: false,
                    })
                }
                Err(error) => Err(error.into()),
            }
        }
        Err(error) => Err(error.into()),
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::fs;
    use std::path::{Path, PathBuf};

    use super::{config_dir_from, identity};
    use crate::testing::scratch;

    fn lookup(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<OsString> + use<> {
        let pairs: Vec<(String, String)> = pairs
            .iter()
            .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
            .collect();
        move |name| {
            pairs
                .iter()
                .find(|(known, _)| known == name)
                .map(|(_, value)| OsString::from(value))
        }
    }

    #[test]
    fn an_override_wins_everywhere() {
        let env = lookup(&[
            ("TTYMUD_CONFIG", "custom"),
            ("APPDATA", "app"),
            ("HOME", "home"),
        ]);
        assert_eq!(config_dir_from(&env, true), Some(PathBuf::from("custom")));
        assert_eq!(config_dir_from(&env, false), Some(PathBuf::from("custom")));
    }

    #[test]
    fn windows_uses_appdata() {
        let env = lookup(&[("APPDATA", "app")]);
        assert_eq!(
            config_dir_from(env, true),
            Some(Path::new("app").join("ttymud"))
        );
    }

    #[test]
    fn other_systems_use_xdg_or_home() {
        let xdg = lookup(&[("XDG_CONFIG_HOME", "xdg"), ("HOME", "home")]);
        assert_eq!(
            config_dir_from(xdg, false),
            Some(Path::new("xdg").join("ttymud"))
        );
        let home = lookup(&[("XDG_CONFIG_HOME", ""), ("HOME", "home")]);
        let expected = Path::new("home").join(".config").join("ttymud");
        assert_eq!(config_dir_from(home, false), Some(expected));
    }

    #[test]
    fn no_known_place_gives_no_directory() {
        assert_eq!(config_dir_from(lookup(&[]), false), None);
    }

    #[test]
    fn the_first_run_creates_an_identity_and_later_runs_load_it() {
        let dir = scratch("identity");
        let first = identity(&dir).unwrap();
        let second = identity(&dir).unwrap();
        let _ = fs::remove_dir_all(&dir);
        assert!(first.created);
        assert!(!second.created);
        assert_eq!(first.keys.public(), second.keys.public());
    }

    #[test]
    fn a_broken_identity_is_an_error_and_stays_untouched() {
        let dir = scratch("broken");
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("identity.key");
        fs::write(&path, "not a key").unwrap();
        let result = identity(&dir);
        let content = fs::read_to_string(&path).unwrap();
        let _ = fs::remove_dir_all(&dir);
        assert!(result.is_err());
        assert_eq!(content, "not a key");
    }
}
