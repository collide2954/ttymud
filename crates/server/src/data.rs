use std::fs;
use std::io;
use std::path::Path;

use ttymud_net::Keys;

const KEY_FILE: &str = "server.key";

pub fn init(dir: &Path) -> io::Result<bool> {
    fs::create_dir_all(dir)?;
    let path = dir.join(KEY_FILE);
    if path.exists() {
        return Ok(false);
    }
    Keys::generate()?.save_new(&path)?;
    Ok(true)
}

pub fn load_keys(dir: &Path) -> io::Result<Keys> {
    Ok(Keys::load(&dir.join(KEY_FILE))?)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io::ErrorKind;
    use std::path::PathBuf;

    use super::{init, load_keys};

    fn scratch(name: &str) -> PathBuf {
        let id = std::process::id();
        let path = std::env::temp_dir().join(format!("ttymud-{id}-{name}"));
        let _ = fs::remove_dir_all(&path);
        path
    }

    #[test]
    fn init_makes_a_directory_with_a_key() {
        let dir = scratch("init");
        assert!(init(&dir).unwrap());
        assert!(dir.join("server.key").is_file());
        assert!(load_keys(&dir).is_ok());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn init_keeps_an_existing_key() {
        let dir = scratch("again");
        init(&dir).unwrap();
        let first = load_keys(&dir).unwrap().public();
        assert!(!init(&dir).unwrap());
        assert_eq!(load_keys(&dir).unwrap().public(), first);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_directory_without_init_has_no_key() {
        let dir = scratch("empty");
        fs::create_dir_all(&dir).unwrap();
        let error = load_keys(&dir).unwrap_err();
        assert_eq!(error.kind(), ErrorKind::NotFound);
        let _ = fs::remove_dir_all(&dir);
    }
}
