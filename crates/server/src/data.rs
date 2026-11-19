use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::Path;

use ttymud_net::Keys;

const KEY_FILE: &str = "server.key";

pub fn init(dir: &Path) -> io::Result<bool> {
    fs::create_dir_all(dir)?;
    let path = dir.join(KEY_FILE);
    if path.exists() {
        return Ok(false);
    }
    let keys = Keys::generate().map_err(io::Error::other)?;
    write_secret(&path, &keys.to_text())?;
    Ok(true)
}

pub fn load_keys(dir: &Path) -> io::Result<Keys> {
    let text = fs::read_to_string(dir.join(KEY_FILE))?;
    Keys::from_text(&text).map_err(io::Error::other)
}

fn write_secret(path: &Path, text: &str) -> io::Result<()> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path)?;
    file.write_all(text.as_bytes())?;
    file.sync_all()
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

    #[cfg(unix)]
    #[test]
    fn only_the_owner_can_read_the_key() {
        use std::os::unix::fs::PermissionsExt;
        let dir = scratch("mode");
        init(&dir).unwrap();
        let mode = fs::metadata(dir.join("server.key"))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600);
        let _ = fs::remove_dir_all(&dir);
    }
}
