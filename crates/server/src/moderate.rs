use std::io;
use std::path::Path;

use crate::city::load_world;
use crate::identity::load_identity;
use crate::store::{CREWS, MODERATORS, remove, write};

const RESTART: &str = "Restart the server to use the change.";

#[derive(Debug, PartialEq, Eq)]
pub enum Moderation {
    List,
    Add(String),
    Remove(String),
}

pub fn moderate(dir: &Path, action: &Moderation) -> io::Result<String> {
    let mut world = load_world(dir)?;
    let done = match action {
        Moderation::List => {
            let names = world.moderators();
            if names.is_empty() {
                return Ok("This server has no moderators.".to_owned());
            }
            return Ok(format!("Moderators: {}", names.join(", ")));
        }
        Moderation::Add(handle) => world
            .add_moderator(handle)
            .map(|name| format!("{name} is now a moderator.")),
        Moderation::Remove(handle) => world
            .remove_moderator(handle)
            .map(|name| format!("{name} is no longer a moderator.")),
    };
    let done = done.map_err(io::Error::other)?;
    write(dir, MODERATORS, &world.moderators_text())?;
    Ok(format!("{done} {RESTART}"))
}

pub fn delete(dir: &Path, handle: &str) -> io::Result<String> {
    let mut world = load_world(dir)?;
    let mut identity = load_identity(dir)?;
    let moderators = world.moderators_text();
    let (id, name) = world.delete_account(handle).map_err(io::Error::other)?;
    identity.forget(&id);
    for (path, text) in world.saves() {
        if path == CREWS {
            write(dir, &path, &text)?;
        }
    }
    if world.moderators_text() != moderators {
        write(dir, MODERATORS, &world.moderators_text())?;
    }
    if let Some((path, text)) = identity.save() {
        write(dir, &path, &text)?;
    }
    remove(dir, &format!("accounts/{id}.ini"))?;
    Ok(format!("Deleted the account of {name} and its data."))
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::{Moderation, delete, moderate};
    use crate::store::write;
    use crate::testing::scratch;

    const NYX: &str =
        "[account a1]\n[handle nyx]\nkit = ghost\nheat = 0\ncooled = 0\nintegrity = 100\n";
    const ZER0: &str =
        "[account b2]\n[handle zer0]\nkit = ghost\nheat = 0\ncooled = 0\nintegrity = 100\n";

    #[test]
    fn moderators_change_in_the_data_directory() {
        let dir = scratch("moderate");
        write(&dir, "accounts/a1.ini", NYX).unwrap();
        let run = |action: Moderation| moderate(&dir, &action).map_err(|error| error.to_string());
        assert_eq!(
            run(Moderation::List),
            Ok("This server has no moderators.".to_owned())
        );
        assert_eq!(
            run(Moderation::Add("NYX".to_owned())),
            Ok("nyx is now a moderator. Restart the server to use the change.".to_owned())
        );
        assert_eq!(run(Moderation::List), Ok("Moderators: nyx".to_owned()));
        let file = fs::read_to_string(dir.join("moderators.ini")).unwrap();
        assert!(file.contains("[moderator a1]\nhandle = nyx\n"), "{file}");
        assert_eq!(
            run(Moderation::Add("zer0".to_owned())),
            Err("No handle named zer0.".to_owned())
        );
        assert_eq!(
            run(Moderation::Remove("nyx".to_owned())),
            Ok("nyx is no longer a moderator. Restart the server to use the change.".to_owned())
        );
        assert_eq!(
            run(Moderation::List),
            Ok("This server has no moderators.".to_owned())
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn delete_removes_an_account_and_its_keys_from_the_data_directory() {
        let dir = scratch("delete");
        write(&dir, "accounts/a1.ini", NYX).unwrap();
        write(&dir, "accounts/b2.ini", ZER0).unwrap();
        let keys = "[account a1]\nkey = a1\nkey = c3\n\n[account b2]\nkey = b2\n";
        write(&dir, "keys.ini", keys).unwrap();
        write(&dir, "moderators.ini", "[moderator a1]\nhandle = nyx\n").unwrap();
        let run = |word: &str| delete(&dir, word).map_err(|error| error.to_string());
        assert_eq!(
            run("NYX"),
            Ok("Deleted the account of nyx and its data.".to_owned())
        );
        let read = |name: &str| fs::read_to_string(dir.join(name)).unwrap_or_default();
        let (keys, moderators) = (read("keys.ini"), read("moderators.ini"));
        let gone = !dir.join("accounts/a1.ini").exists();
        let kept = dir.join("accounts/b2.ini").exists();
        assert_eq!(run("nyx"), Err("No handle named nyx.".to_owned()));
        let _ = fs::remove_dir_all(&dir);
        assert!(gone && kept);
        assert!(!keys.contains("a1") && !keys.contains("c3"), "{keys}");
        assert!(keys.contains("[account b2]\nkey = b2\n"), "{keys}");
        assert!(!moderators.contains("a1"), "{moderators}");
    }
}
