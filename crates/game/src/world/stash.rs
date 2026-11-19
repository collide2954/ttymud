use ttymud_text::{Message, Style};

use super::{PlayerId, Step, Thing, World, choose, listed, usage};

const STASH: usize = 3;

impl World {
    pub(super) fn stash(&mut self, player: PlayerId, args: &[String]) -> Result<Step, Message> {
        let Some(state) = self.players.get(&player) else {
            return Ok(Step::default());
        };
        let [word] = args else {
            if !args.is_empty() {
                return Err(Message::error("Usage: stash [file]."));
            }
            if state.stash.is_empty() {
                return Ok(Step::show(vec![Message::game("Your stash is empty.")]));
            }
            let names = state.stash.iter().map(|file| file.name.as_str());
            let line = listed(Message::game("Your stash: "), Style::File, names);
            return Ok(Step::show(vec![line]));
        };
        self.at_home(player, "use your stash")?;
        let Some(state) = self.players.get_mut(&player) else {
            return Ok(Step::default());
        };
        let names: Vec<&str> = state
            .rig
            .files
            .iter()
            .map(|file| file.name.as_str())
            .collect();
        let name = choose(Thing::File, word, &names)?.to_owned();
        if state.stash.len() >= STASH {
            return Err(Message::error(format!(
                "Your stash is full. It holds {STASH} files."
            )));
        }
        let index = state.rig.files.iter().position(|file| file.name == name);
        if let Some(index) = index {
            state.stash.push(state.rig.files.remove(index));
        }
        let count = state.stash.len();
        Ok(Step::show(vec![
            Message::game("You put ")
                .span(Style::File, &name)
                .text(format!(
                    " into your stash. It holds {count} of {STASH} files."
                )),
        ]))
    }

    pub(super) fn unstash(&mut self, player: PlayerId, args: &[String]) -> Result<Step, Message> {
        self.at_home(player, "use your stash")?;
        let Some(state) = self.players.get_mut(&player) else {
            return Ok(Step::default());
        };
        let names: Vec<&str> = state.stash.iter().map(|file| file.name.as_str()).collect();
        let [word] = args else {
            return Err(usage("unstash <file>", Thing::File, &names));
        };
        let name = choose(Thing::File, word, &names)?.to_owned();
        if state.rig.files.iter().any(|file| file.name == name) {
            return Err(Message::error(format!("{name} is already on your rig.")));
        }
        let index = state.stash.iter().position(|file| file.name == name);
        if let Some(index) = index {
            state.rig.files.push(state.stash.remove(index));
        }
        Ok(Step::show(vec![
            Message::game("You take ")
                .span(Style::File, &name)
                .text(" from your stash."),
        ]))
    }
}

#[cfg(test)]
mod tests {
    use crate::testing::{plain, run, walk, world};

    const HOME: &str = "go back-room";

    #[test]
    fn an_empty_stash_says_so() {
        assert_eq!(run("stash"), ["Your stash is empty."]);
    }

    #[test]
    fn stash_moves_a_file_from_the_rig() {
        let lines = ["get motd.txt", HOME, "stash motd.txt"];
        assert_eq!(
            walk(&lines).0,
            ["You put motd.txt into your stash. It holds 1 of 3 files."]
        );
        let after = |line: &str| walk(&[&lines[..], &[line]].concat()).0;
        assert_eq!(after("inventory"), ["Your rig has no files."]);
        assert_eq!(after("stash"), ["Your stash: motd.txt"]);
    }

    #[test]
    fn unstash_moves_a_file_back_to_the_rig() {
        let lines = ["get motd.txt", HOME, "stash motd.txt", "unstash motd.txt"];
        assert_eq!(walk(&lines).0, ["You take motd.txt from your stash."]);
        let lines = [&lines[..], &["inventory"]].concat();
        assert_eq!(walk(&lines).0, ["motd.txt"]);
    }

    #[test]
    fn the_stash_works_only_at_home() {
        assert_eq!(
            walk(&["get motd.txt", "stash motd.txt"]).0,
            ["You can use your stash only at back-room."]
        );
    }

    #[test]
    fn the_stash_holds_three_files() {
        let lines = [
            "get motd.txt",
            "get rules.txt",
            "go bazaar",
            "get burn-list.txt",
            "go market",
            "get prices.db",
            "go relay",
            HOME,
            "stash motd.txt",
            "stash rules.txt",
            "stash prices.db",
            "stash burn-list.txt",
        ];
        assert_eq!(walk(&lines).0, ["Your stash is full. It holds 3 files."]);
    }

    #[test]
    fn the_stash_survives_a_burn() {
        let mut world = world();
        let (player, _) = world.connect("a");
        for line in ["nyx", "ghost", "get motd.txt", HOME, "stash motd.txt"] {
            world.command(player, line);
        }
        for line in ["go gate", "go core"] {
            world.command(player, line);
        }
        if let Some(state) = world.players.get_mut(&player) {
            state.heat = 4;
            if let Some(run) = state.run.as_mut() {
                run.trace = 98;
            }
        }
        world.command(player, "scan");
        for line in ["zer0", "breaker"] {
            world.command(player, line);
        }
        assert_eq!(
            plain(&world.command(player, "stash")),
            ["Your stash: motd.txt"]
        );
        world.leave(player);
        let (player, _) = world.connect("a");
        assert_eq!(
            plain(&world.command(player, "stash")),
            ["Your stash: motd.txt"]
        );
    }
}
