use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::thread;

use crate::identity::KEYS;

const ACCOUNTS: &str = "accounts";
const BURNS: &str = "burns.ini";
const NEWS: &str = "news.ini";
const CREWS: &str = "crews.ini";
const WORLD: &str = "world.ini";
pub(crate) const MODERATORS: &str = "moderators.ini";
const AUDIT: &str = "audit.log";
const REPORTS: &str = "reports.ini";
const BANS: &str = "bans.ini";

pub struct Store(mpsc::Sender<(String, String)>);

impl Store {
    pub fn start(dir: &Path) -> io::Result<Self> {
        let dir = dir.to_path_buf();
        let (sender, inbox) = mpsc::channel::<(String, String)>();
        thread::Builder::new()
            .name("storage".to_owned())
            .spawn(move || {
                for (path, text) in inbox {
                    if let Err(error) = write(&dir, &path, &text) {
                        eprintln!("ttymud-server: cannot save {path}: {error}");
                    }
                }
            })?;
        Ok(Self(sender))
    }

    pub(crate) fn sender(self) -> mpsc::Sender<(String, String)> {
        self.0
    }
}

pub(crate) fn load_saves(dir: &Path) -> io::Result<Vec<(String, String)>> {
    let mut saves = Vec::new();
    load_file(dir, WORLD, &mut saves)?;
    let accounts = dir.join(ACCOUNTS);
    if accounts.is_dir() {
        let mut names = Vec::new();
        for entry in fs::read_dir(&accounts)? {
            let name = entry?.file_name().to_string_lossy().into_owned();
            if Path::new(&name).extension().is_some_and(|ext| ext == "ini") {
                names.push(name);
            }
        }
        names.sort();
        for name in names {
            let text = fs::read_to_string(accounts.join(&name))?;
            saves.push((format!("{ACCOUNTS}/{name}"), text));
        }
    }
    for name in [BURNS, NEWS, CREWS, MODERATORS, AUDIT, REPORTS, BANS] {
        load_file(dir, name, &mut saves)?;
    }
    Ok(saves)
}

fn load_file(dir: &Path, name: &str, saves: &mut Vec<(String, String)>) -> io::Result<()> {
    let path = dir.join(name);
    if path.is_file() {
        saves.push((name.to_owned(), fs::read_to_string(path)?));
    }
    Ok(())
}

pub(crate) fn write(dir: &Path, path: &str, text: &str) -> io::Result<()> {
    let target = target(dir, path).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("not a save path: {path}"),
        )
    })?;
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut temporary = target.clone().into_os_string();
    temporary.push(".tmp");
    let mut file = fs::File::create(&temporary)?;
    file.write_all(text.as_bytes())?;
    file.sync_all()?;
    drop(file);
    fs::rename(&temporary, &target)
}

fn target(dir: &Path, path: &str) -> Option<PathBuf> {
    if [
        BURNS, NEWS, CREWS, WORLD, MODERATORS, AUDIT, REPORTS, BANS, KEYS,
    ]
    .contains(&path)
    {
        return Some(dir.join(path));
    }
    let name = path.strip_prefix("accounts/")?.strip_suffix(".ini")?;
    let safe = !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    safe.then(|| dir.join(ACCOUNTS).join(format!("{name}.ini")))
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::{load_saves, write};
    use crate::testing::scratch;

    #[test]
    fn saves_come_back_after_a_write() {
        let dir = scratch("store-back");
        write(&dir, "accounts/a1.ini", "[account a1]\n").unwrap();
        write(&dir, "burns.ini", "[burn nyx]\nwhen = 1\n").unwrap();
        write(&dir, "accounts/a1.ini", "[account a1]\nknow = scan\n").unwrap();
        write(&dir, "world.ini", "[world c]\nseed = 7\n").unwrap();
        write(&dir, "news.ini", "[news c]\n").unwrap();
        write(&dir, "crews.ini", "[crew Moths]\n").unwrap();
        write(&dir, "moderators.ini", "[moderator a1]\n").unwrap();
        write(&dir, "audit.log", "x mutes y.\n").unwrap();
        write(&dir, "reports.ini", "[report 1]\n").unwrap();
        write(&dir, "bans.ini", "[ban a1]\n").unwrap();
        assert_eq!(
            load_saves(&dir).unwrap(),
            [
                ("world.ini".to_owned(), "[world c]\nseed = 7\n".to_owned()),
                (
                    "accounts/a1.ini".to_owned(),
                    "[account a1]\nknow = scan\n".to_owned()
                ),
                ("burns.ini".to_owned(), "[burn nyx]\nwhen = 1\n".to_owned()),
                ("news.ini".to_owned(), "[news c]\n".to_owned()),
                ("crews.ini".to_owned(), "[crew Moths]\n".to_owned()),
                ("moderators.ini".to_owned(), "[moderator a1]\n".to_owned()),
                ("audit.log".to_owned(), "x mutes y.\n".to_owned()),
                ("reports.ini".to_owned(), "[report 1]\n".to_owned()),
                ("bans.ini".to_owned(), "[ban a1]\n".to_owned()),
            ]
        );
        assert!(!dir.join("accounts/a1.ini.tmp").exists());
        assert!(!dir.join("audit.log.tmp").exists());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_new_directory_has_no_saves() {
        let dir = scratch("store-empty");
        fs::create_dir_all(&dir).unwrap();
        assert_eq!(load_saves(&dir).unwrap(), Vec::<(String, String)>::new());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn only_save_paths_can_be_written() {
        let dir = scratch("store-paths");
        for path in [
            "../x.ini",
            "/x.ini",
            "accounts/../x.ini",
            "x.txt",
            "accounts/.ini",
            "server.key",
        ] {
            assert!(write(&dir, path, "x").is_err(), "{path}");
        }
        assert!(!dir.join("server.key").exists());
        let _ = fs::remove_dir_all(&dir);
    }
}
