use std::io;
use std::path::Path;

use crate::city::load_world;
use crate::store::{MODERATORS, write};

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

#[cfg(test)]
mod tests {
    use std::fs;

    use super::{Moderation, moderate};
    use crate::store::write;
    use crate::testing::scratch;

    const NYX: &str =
        "[account a1]\n[handle nyx]\nkit = ghost\nheat = 0\ncooled = 0\nintegrity = 100\n";

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
}
