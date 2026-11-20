use ttymud_text::{Message, Style};

use super::longline::is_remote;
use super::{Player, PlayerId, Step, World, lines, listed};
use crate::matching::{self, Found};

#[derive(Clone, Copy, PartialEq, Eq)]
enum List {
    Ignore,
    Block,
}

impl World {
    pub(super) fn person(&self, player: PlayerId) -> Option<String> {
        self.players.get(&player).map(Player::key)
    }

    pub(super) fn hears(&self, listener: PlayerId, speaker: PlayerId) -> bool {
        let key = self.person(speaker);
        key.is_none_or(|key| self.hears_key(listener, &key))
    }

    pub(super) fn hears_key(&self, listener: PlayerId, key: &str) -> bool {
        let Some(state) = self.players.get(&listener) else {
            return true;
        };
        let social = &state.social;
        !social
            .ignores
            .iter()
            .chain(&social.blocks)
            .any(|known| known == key)
    }

    pub(super) fn blocks(&self, listener: PlayerId, speaker: PlayerId) -> bool {
        let (Some(state), Some(key)) = (self.players.get(&listener), self.person(speaker)) else {
            return false;
        };
        state.social.blocks.contains(&key)
    }

    pub(super) fn find_person(&self, word: &str) -> Result<(String, String), Message> {
        if word.contains('@') {
            return self.remote_person(word);
        }
        let online = self.online_names();
        if let Found::One(name) = matching::resolve(word, &online)
            && let Some(state) = self.players.values().find(|state| state.name == name)
        {
            return Ok((state.key(), name.to_owned()));
        }
        self.saved_handle(word)
            .ok_or_else(|| Message::error(format!("No handle named {word}.")))
    }

    pub(super) fn saved_handle(&self, word: &str) -> Option<(String, String)> {
        self.accounts.iter().find_map(|(id, account)| {
            let handle = account.handle.as_ref()?;
            handle
                .name
                .eq_ignore_ascii_case(word)
                .then(|| (id.clone(), handle.name.clone()))
        })
    }

    pub(super) fn person_label<'a>(&'a self, key: &'a str) -> Option<&'a str> {
        if let Some(name) = key.strip_prefix('@') {
            return Some(name);
        }
        if is_remote(key) {
            return Some(key);
        }
        let online = self
            .players
            .values()
            .find(|state| state.account.as_deref() == Some(key));
        online.map(|state| state.name.as_str()).or_else(|| {
            let handle = self.accounts.get(key)?.handle.as_ref()?;
            Some(handle.name.as_str())
        })
    }

    fn person_name(&self, key: &str) -> Option<String> {
        self.person_label(key).map(str::to_owned)
    }

    pub(super) fn ignore(&mut self, player: PlayerId, args: &[String]) -> Result<Step, Message> {
        match args {
            [] => Ok(self.ignore_lists(player)),
            _ => self.change(player, args, List::Ignore, true),
        }
    }

    pub(super) fn unignore(&mut self, player: PlayerId, args: &[String]) -> Result<Step, Message> {
        self.change(player, args, List::Ignore, false)
    }

    pub(super) fn block(&mut self, player: PlayerId, args: &[String]) -> Result<Step, Message> {
        self.change(player, args, List::Block, true)
    }

    pub(super) fn unblock(&mut self, player: PlayerId, args: &[String]) -> Result<Step, Message> {
        self.change(player, args, List::Block, false)
    }

    fn ignore_lists(&self, player: PlayerId) -> Step {
        let Some(state) = self.players.get(&player) else {
            return Step::default();
        };
        let line = |verb: &str, keys: &[String]| {
            let names: Vec<String> = keys
                .iter()
                .filter_map(|key| self.person_name(key))
                .collect();
            if names.is_empty() {
                return Message::game(format!("You {verb} nobody."));
            }
            let message = Message::game(format!("You {verb}: "));
            listed(message, Style::Player, names.iter().map(String::as_str))
        };
        lines(vec![
            line("ignore", &state.social.ignores),
            line("block", &state.social.blocks),
        ])
    }

    fn change(
        &mut self,
        player: PlayerId,
        args: &[String],
        list: List,
        add: bool,
    ) -> Result<Step, Message> {
        let verb = match (list, add) {
            (List::Ignore, true) => "ignore",
            (List::Ignore, false) => "unignore",
            (List::Block, true) => "block",
            (List::Block, false) => "unblock",
        };
        let [word] = args else {
            return Err(Message::error(format!("Usage: {verb} <handle>.")));
        };
        let (key, name) = self.find_person(word)?;
        if self.person(player).as_ref() == Some(&key) {
            return Err(Message::error("You cannot ignore yourself."));
        }
        let Some(state) = self.players.get_mut(&player) else {
            return Ok(Step::default());
        };
        let keys = match list {
            List::Ignore => &mut state.social.ignores,
            List::Block => &mut state.social.blocks,
        };
        let (base, done) = match list {
            List::Ignore => ("ignore", "ignoring"),
            List::Block => ("block", "blocking"),
        };
        let message = if add {
            if keys.contains(&key) {
                return Err(Message::error(format!("You already {base} {name}.")));
            }
            keys.push(key);
            match list {
                List::Ignore => format!("You ignore {name}. You no longer see their messages."),
                List::Block => format!(
                    "You block {name}. They cannot send you private messages, and you no longer see their messages."
                ),
            }
        } else {
            if !keys.contains(&key) {
                return Err(Message::error(format!("You do not {base} {name}.")));
            }
            keys.retain(|known| known != &key);
            format!("You stop {done} {name}.")
        };
        Ok(Step::show(vec![Message::game(message)]))
    }
}

#[cfg(test)]
mod tests {
    use crate::testing::{heard, plain, world};
    use crate::{PlayerId, World};

    fn three() -> (World, PlayerId, PlayerId, PlayerId) {
        let mut world = world();
        let (ghost, _) = world.join("ghost");
        let (nyx, _) = world.join("nyx");
        let (zer0, _) = world.join("zer0");
        (world, ghost, nyx, zer0)
    }

    #[test]
    fn ignore_hides_say_channel_and_tell() {
        let (mut world, ghost, nyx, zer0) = three();
        assert_eq!(
            plain(&world.command(nyx, "ignore ghost")),
            ["You ignore ghost. You no longer see their messages."]
        );
        for line in ["say one", "#city two", "tell nyx three"] {
            let outputs = world.command(ghost, line);
            assert_eq!(heard(&outputs, nyx), Vec::<String>::new(), "{line}");
            assert!(!heard(&outputs, ghost).is_empty(), "{line}");
        }
        let outputs = world.command(ghost, "say four");
        assert_eq!(heard(&outputs, zer0), ["<ghost> four"]);
    }

    #[test]
    fn block_also_refuses_tells() {
        let (mut world, ghost, nyx, _) = three();
        assert_eq!(
            plain(&world.command(nyx, "block ghost")),
            [
                "You block ghost. They cannot send you private messages, and you no longer see their messages."
            ]
        );
        let outputs = world.command(ghost, "tell nyx hi");
        assert_eq!(
            heard(&outputs, ghost),
            ["nyx does not take private messages from you."]
        );
        assert_eq!(heard(&outputs, nyx), Vec::<String>::new());
    }

    #[test]
    fn ignore_lists_both_lists_and_can_be_undone() {
        let (mut world, _, nyx, _) = three();
        let mut said = |line: &str| plain(&world.command(nyx, line));
        assert_eq!(said("ignore"), ["You ignore nobody.", "You block nobody."]);
        said("ignore ghost");
        said("block zer0");
        assert_eq!(said("ignore"), ["You ignore: ghost", "You block: zer0"]);
        assert_eq!(said("unignore ghost"), ["You stop ignoring ghost."]);
        assert_eq!(said("unblock zer0"), ["You stop blocking zer0."]);
        assert_eq!(said("unblock zer0"), ["You do not block zer0."]);
    }

    #[test]
    fn ignore_checks_its_handle() {
        let (mut world, _, nyx, _) = three();
        let mut said = |line: &str| plain(&world.command(nyx, line));
        assert_eq!(said("ignore nyx"), ["You cannot ignore yourself."]);
        assert_eq!(said("ignore nobody"), ["No handle named nobody."]);
        said("ignore gh");
        assert_eq!(said("ignore ghost"), ["You already ignore ghost."]);
        assert_eq!(said("block"), ["Usage: block <handle>."]);
    }

    #[test]
    fn ignore_finds_an_offline_handle_and_lasts() {
        let mut old = world();
        let (zer0, _) = old.connect("b");
        for line in ["zer0", "ghost"] {
            old.command(zer0, line);
        }
        old.leave(zer0);
        let (nyx, _) = old.connect("a");
        for line in ["nyx", "ghost", "ignore zer0"] {
            old.command(nyx, line);
        }
        old.leave(nyx);
        let saves = old.saves();
        let mut fresh = world();
        for (path, text) in &saves {
            fresh.restore(path, text).unwrap();
        }
        let (zer0, _) = fresh.connect("b");
        let (nyx, _) = fresh.connect("a");
        fresh.command(zer0, "go back-room");
        fresh.set_time(100);
        let outputs = fresh.command(zer0, "#city hello");
        assert_eq!(heard(&outputs, nyx), Vec::<String>::new());
        assert_eq!(plain(&fresh.command(nyx, "ignore"))[0], "You ignore: zer0");
    }
}
