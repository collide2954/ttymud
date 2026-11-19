use super::{PlayerId, World};
use crate::content::ContentError;
use crate::ini;

pub(super) const MODERATORS: &str = "moderators.ini";

impl World {
    pub(super) fn is_moderator(&self, player: PlayerId) -> bool {
        let account = self
            .players
            .get(&player)
            .and_then(|state| state.account.as_ref());
        account.is_some_and(|account| self.moderators.contains(account))
    }

    pub(super) fn read_moderators(&mut self, text: &str) -> Result<(), Vec<ContentError>> {
        let (sections, mut errors) = ini::read(text);
        let mut found = Vec::new();
        for section in &sections {
            if section.kind != "moderator" {
                let kind = section.kind;
                errors.push(ContentError {
                    line: section.line,
                    message: format!("unknown section type: {kind}"),
                });
                continue;
            }
            for pair in section.pairs.iter().filter(|pair| pair.key != "handle") {
                let (id, key) = (section.id, pair.key);
                errors.push(ContentError {
                    line: pair.line,
                    message: format!("unknown key in moderator {id}: {key}"),
                });
            }
            found.push(section.id.to_owned());
        }
        if !errors.is_empty() {
            return Err(errors);
        }
        self.moderators.extend(found);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::testing::{plain, world};
    use crate::{PlayerId, World};

    const MODERATORS: &str = "[moderator a]\nhandle = nyx\n";

    fn hacker(world: &mut World, account: &str, name: &str) -> PlayerId {
        let (player, _) = world.connect(account);
        for line in [name, "ghost"] {
            world.command(player, line);
        }
        player
    }

    #[test]
    fn moderators_come_from_their_file() {
        let mut world = world();
        world.restore("moderators.ini", MODERATORS).unwrap();
        let nyx = hacker(&mut world, "a", "nyx");
        hacker(&mut world, "b", "zer0");
        assert_eq!(
            plain(&world.command(nyx, "who")),
            ["Online: nyx (moderator), zer0"]
        );
    }

    #[test]
    fn a_bad_moderator_file_fails() {
        let mut world = world();
        let errors: Vec<String> = world
            .restore("moderators.ini", "[admin a]\n[moderator b]\ncolor = red\n")
            .unwrap_err()
            .iter()
            .map(ToString::to_string)
            .collect();
        assert_eq!(
            errors,
            [
                "line 1: unknown section type: admin",
                "line 3: unknown key in moderator b: color"
            ]
        );
    }
}
