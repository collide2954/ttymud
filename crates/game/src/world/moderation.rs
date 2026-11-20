use std::collections::BTreeSet;

use ttymud_text::Message;
use ttymud_text::ini;

use super::bans::Bans;
use super::longline::is_remote;
use super::reports::Reports;
use super::save::Text;
use super::{Output, PlayerId, Step, World, minute_word};
use crate::content::ContentError;
use crate::date::stamp;

pub(super) const MODERATORS: &str = "moderators.ini";
pub(super) const AUDIT: &str = "audit.log";

#[derive(Debug, Default)]
pub(super) struct Moderation {
    moderators: BTreeSet<String>,
    audit: Vec<String>,
    audit_dirty: bool,
    drops: Vec<(PlayerId, String)>,
    pub(super) reports: Reports,
    pub(super) bans: Bans,
}

impl Moderation {
    pub(super) fn save_audit(&mut self) -> Option<(String, String)> {
        std::mem::take(&mut self.audit_dirty).then(|| {
            let lines = self.audit.iter().flat_map(|line| [line.as_str(), "\n"]);
            let text = lines.collect();
            (AUDIT.to_owned(), text)
        })
    }

    pub(super) fn forget_moderator(&mut self, id: &str) {
        self.moderators.remove(id);
    }

    pub(super) fn read_audit(&mut self, text: &str) {
        let lines = text.lines().filter(|line| !line.trim().is_empty());
        self.audit = lines.map(str::to_owned).collect();
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

impl World {
    #[must_use]
    pub fn moderators(&self) -> Vec<String> {
        let ids = self.moderation.moderators.iter();
        let mut names: Vec<String> = ids
            .map(|id| self.person_label(id).unwrap_or(id).to_owned())
            .collect();
        names.sort();
        names
    }

    pub fn add_moderator(&mut self, handle: &str) -> Result<String, String> {
        let (id, name) = self
            .saved_handle(handle)
            .ok_or_else(|| format!("No handle named {handle}."))?;
        if !self.moderation.moderators.insert(id) {
            return Err(format!("{name} is already a moderator."));
        }
        Ok(name)
    }

    pub fn remove_moderator(&mut self, word: &str) -> Result<String, String> {
        let (id, name) = self
            .saved_handle(word)
            .unwrap_or_else(|| (word.to_owned(), word.to_owned()));
        if !self.moderation.moderators.remove(&id) {
            return Err(format!("{name} is not a moderator."));
        }
        Ok(name)
    }

    #[must_use]
    pub fn moderators_text(&self) -> String {
        let mut text = Text::new("# The moderators of this server.");
        for id in &self.moderation.moderators {
            text.section("moderator", id);
            let account = self.accounts.get(id);
            if let Some(handle) = account.and_then(|account| account.handle.as_ref()) {
                text.pair("handle", &handle.name);
            }
        }
        text.finish()
    }

    pub(super) fn is_moderator(&self, player: PlayerId) -> bool {
        let account = self
            .players
            .get(&player)
            .and_then(|state| state.account.as_ref());
        account.is_some_and(|account| self.moderation.moderators.contains(account))
    }

    pub(super) fn moderator_only(&self, player: PlayerId, verb: &str) -> Result<(), Message> {
        if self.is_moderator(player) {
            Ok(())
        } else {
            Err(Message::error(format!("Only moderators can use {verb}.")))
        }
    }

    pub(super) fn subject(&self, word: &str, verb: &str) -> Result<(String, String), Message> {
        let (key, name) = self.find_person(word)?;
        if self.moderation.moderators.contains(&key) {
            return Err(Message::error(format!("You cannot {verb} a moderator.")));
        }
        if is_remote(&key) && verb != "mute" {
            return Err(Message::error(format!(
                "{name} is in a linked city. You can only mute a handle of a linked city."
            )));
        }
        Ok((key, name))
    }

    pub(super) fn disconnect(&mut self, key: &str, reason: &str) -> usize {
        let online = self.players_of(key);
        let count = online.len();
        let drops = online.into_iter().map(|id| (id, reason.to_owned()));
        self.moderation.drops.extend(drops);
        count
    }

    pub(super) fn mute(&mut self, player: PlayerId, args: &[String]) -> Result<Step, Message> {
        self.moderator_only(player, "mute")?;
        let usage = || Message::error("Usage: mute <handle> <minutes>.");
        let [word, count] = args else {
            return Err(usage());
        };
        let count = count.parse::<u32>().ok().filter(|&count| count > 0);
        let count = u64::from(count.ok_or_else(usage)?);
        let (key, name) = self.subject(word, "mute")?;
        self.set_muted(&key, self.now + count * 60);
        let span = format!("{count} {}", minute_word(count));
        self.notice(&key, &format!("A moderator muted you for {span}."));
        self.audit(player, &format!("mutes {name} for {span}."));
        Ok(Step::show(vec![Message::game(format!(
            "You mute {name} for {span}."
        ))]))
    }

    pub(super) fn unmute(&mut self, player: PlayerId, args: &[String]) -> Result<Step, Message> {
        self.moderator_only(player, "unmute")?;
        let [word] = args else {
            return Err(Message::error("Usage: unmute <handle>."));
        };
        let (key, name) = self.find_person(word)?;
        if self.muted_until(&key) <= self.now {
            return Err(Message::error(format!("{name} is not muted.")));
        }
        self.set_muted(&key, 0);
        self.notice(&key, "A moderator unmuted you.");
        self.audit(player, &format!("unmutes {name}."));
        Ok(Step::show(vec![Message::game(format!(
            "You unmute {name}."
        ))]))
    }

    pub(super) fn kick(&mut self, player: PlayerId, args: &[String]) -> Result<Step, Message> {
        self.moderator_only(player, "kick")?;
        let [word] = args else {
            return Err(Message::error("Usage: kick <handle>."));
        };
        let (key, name) = self.subject(word, "kick")?;
        if self.disconnect(&key, "A moderator kicked you from the server.") == 0 {
            return Err(Message::error(format!("{name} is not online.")));
        }
        self.audit(player, &format!("kicks {name}."));
        Ok(Step::show(vec![Message::game(format!("You kick {name}."))]))
    }

    #[must_use = "the server closes the connections of these players"]
    pub fn drops(&mut self) -> Vec<(PlayerId, String)> {
        std::mem::take(&mut self.moderation.drops)
    }

    pub(super) fn muted(&self, player: PlayerId) -> Option<Message> {
        let until = self.players.get(&player)?.social.muted;
        let left = until.checked_sub(self.now).filter(|&left| left > 0)?;
        let left = left.div_ceil(60);
        Some(Message::error(format!(
            "You are muted for {left} more {}.",
            minute_word(left)
        )))
    }

    fn muted_until(&self, key: &str) -> u64 {
        if is_remote(key) {
            return self.remote_muted_until(key);
        }
        match self.players.values().find(|state| state.key() == key) {
            Some(state) => state.social.muted,
            None => self
                .accounts
                .get(key)
                .map_or(0, |account| account.social.muted),
        }
    }

    fn set_muted(&mut self, key: &str, until: u64) {
        if is_remote(key) {
            self.set_remote_muted(key, until);
            return;
        }
        let online = self.players.values_mut().filter(|state| state.key() == key);
        for state in online {
            state.social.muted = until;
        }
        if let Some(account) = self.accounts.get_mut(key) {
            account.social.muted = until;
            self.dirty.insert(key.to_owned());
        }
    }

    pub(super) fn players_of(&self, key: &str) -> Vec<PlayerId> {
        let online = self.players.iter().filter(|(_, state)| state.key() == key);
        online.map(|(&player, _)| player).collect()
    }

    fn notice(&mut self, key: &str, text: &str) {
        let message = Message::game(text);
        for player in self.players_of(key) {
            let message = message.clone();
            self.outbox.push(Output { player, message });
        }
    }

    pub(super) fn audit(&mut self, player: PlayerId, action: &str) {
        let name = self.players.get(&player).map_or("", |state| &state.name);
        let line = format!("{} {name} {action}", stamp(self.now));
        self.moderation.audit.push(line);
        self.moderation.audit_dirty = true;
    }
}

#[cfg(test)]
mod tests {
    use crate::testing::{hacker, heard, plain, world};
    use crate::{PlayerId, World};

    const MODERATORS: &str = "[moderator a]\nhandle = nyx\n";

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

    fn moderated() -> (World, PlayerId, PlayerId) {
        let mut world = world();
        world.restore("moderators.ini", MODERATORS).unwrap();
        world.set_time(60);
        let nyx = hacker(&mut world, "a", "nyx");
        let zer0 = hacker(&mut world, "b", "zer0");
        (world, nyx, zer0)
    }

    #[test]
    fn a_moderator_mutes_and_unmutes() {
        let (mut world, nyx, zer0) = moderated();
        let outputs = world.command(nyx, "mute zer0 10");
        assert_eq!(heard(&outputs, nyx), ["You mute zer0 for 10 minutes."]);
        assert_eq!(
            heard(&outputs, zer0),
            ["A moderator muted you for 10 minutes."]
        );
        world.set_time(120);
        for line in ["say hi", "#city hi", "tell nyx hi"] {
            assert_eq!(
                plain(&world.command(zer0, line)),
                ["You are muted for 9 more minutes."],
                "{line}"
            );
        }
        let outputs = world.command(nyx, "unmute zer0");
        assert_eq!(heard(&outputs, zer0), ["A moderator unmuted you."]);
        assert_eq!(heard(&world.command(zer0, "say hi"), zer0), ["<zer0> hi"]);
    }

    #[test]
    fn a_mute_ends_by_itself_and_lasts_through_a_save() {
        let (mut world, nyx, zer0) = moderated();
        world.command(nyx, "mute zer0 10");
        world.leave(zer0);
        let saves = world.saves();
        let mut fresh = crate::testing::world();
        for (path, text) in &saves {
            fresh.restore(path, text).unwrap();
        }
        fresh.set_time(600);
        let (zer0, _) = fresh.connect("b");
        assert_eq!(
            plain(&fresh.command(zer0, "say hi")),
            ["You are muted for 1 more minute."]
        );
        fresh.set_time(660);
        assert_eq!(plain(&fresh.command(zer0, "say hi")), ["<zer0> hi"]);
    }

    #[test]
    fn only_moderators_can_mute_and_not_a_moderator() {
        let (mut world, nyx, zer0) = moderated();
        assert_eq!(
            plain(&world.command(zer0, "mute nyx 10")),
            ["Only moderators can use mute."]
        );
        assert_eq!(
            plain(&world.command(nyx, "mute nyx 10")),
            ["You cannot mute a moderator."]
        );
        assert_eq!(
            plain(&world.command(nyx, "mute zer0")),
            ["Usage: mute <handle> <minutes>."]
        );
        assert_eq!(
            plain(&world.command(nyx, "unmute zer0")),
            ["zer0 is not muted."]
        );
    }

    #[test]
    fn moderator_actions_go_to_the_audit_log() {
        let (mut world, nyx, _) = moderated();
        world.command(nyx, "mute zer0 10");
        world.command(nyx, "unmute zer0");
        let saves = world.saves();
        let audit = saves
            .iter()
            .find(|(path, _)| path == "audit.log")
            .map(|(_, text)| text.as_str());
        assert_eq!(
            audit,
            Some(
                "1970-01-01 00:01 nyx mutes zer0 for 10 minutes.\n1970-01-01 00:01 nyx unmutes zer0.\n"
            )
        );
        let mut fresh = crate::testing::world();
        fresh
            .restore("audit.log", audit.unwrap_or_default())
            .unwrap();
        fresh.restore("moderators.ini", MODERATORS).unwrap();
        let nyx = hacker(&mut fresh, "a", "nyx");
        hacker(&mut fresh, "b", "zer0");
        fresh.command(nyx, "mute zer0 1");
        let saves = fresh.saves();
        let audit = saves
            .iter()
            .find(|(path, _)| path == "audit.log")
            .and_then(|(_, text)| text.lines().nth(2));
        assert_eq!(audit, Some("1970-01-01 00:00 nyx mutes zer0 for 1 minute."));
    }

    #[test]
    fn a_moderator_kicks_a_handle_off_the_server() {
        let (mut world, nyx, zer0) = moderated();
        assert_eq!(
            plain(&world.command(zer0, "kick nyx")),
            ["Only moderators can use kick."]
        );
        assert_eq!(plain(&world.command(nyx, "kick zer0")), ["You kick zer0."]);
        let reason = "A moderator kicked you from the server.".to_owned();
        assert_eq!(world.drops(), [(zer0, reason)]);
        assert_eq!(world.drops(), []);
        world.leave(zer0);
        let mut said = |line: &str| plain(&world.command(nyx, line));
        assert_eq!(said("kick zer0"), ["zer0 is not online."]);
        assert_eq!(said("kick nyx"), ["You cannot kick a moderator."]);
        assert_eq!(said("kick"), ["Usage: kick <handle>."]);
        let saves = world.saves();
        let audit = saves.iter().find(|(path, _)| path == "audit.log");
        let last = audit.and_then(|(_, text)| text.lines().last());
        assert_eq!(last, Some("1970-01-01 00:01 nyx kicks zer0."));
    }

    #[test]
    fn only_moderators_see_the_moderator_commands() {
        let (mut world, nyx, zer0) = moderated();
        for (player, moderator) in [(nyx, true), (zer0, false)] {
            let available = plain(&world.command(player, "?"))[0].contains("mute");
            let help = plain(&world.command(player, "help"));
            let helped = help.iter().any(|line| line.starts_with("mute "));
            let completed = world.complete(player, "mu").names == ["mute"];
            assert_eq!([available, helped, completed], [moderator; 3]);
        }
    }

    #[test]
    fn the_operator_adds_and_removes_moderators() {
        let mut world = world();
        hacker(&mut world, "a", "nyx");
        hacker(&mut world, "b", "zer0");
        assert_eq!(world.moderators(), Vec::<String>::new());
        assert_eq!(world.add_moderator("NYX"), Ok("nyx".to_owned()));
        assert_eq!(world.add_moderator("zer0"), Ok("zer0".to_owned()));
        assert_eq!(world.moderators(), ["nyx", "zer0"]);
        assert_eq!(
            world.moderators_text(),
            "# The moderators of this server.\n\n[moderator a]\nhandle = nyx\n\n[moderator b]\nhandle = zer0\n"
        );
        let failed = |result: Result<String, String>| result.unwrap_err();
        assert_eq!(
            failed(world.add_moderator("nyx")),
            "nyx is already a moderator."
        );
        assert_eq!(failed(world.add_moderator("ny")), "No handle named ny.");
        assert_eq!(world.remove_moderator("zer0"), Ok("zer0".to_owned()));
        assert_eq!(
            failed(world.remove_moderator("zer0")),
            "zer0 is not a moderator."
        );
        assert_eq!(world.remove_moderator("a"), Ok("a".to_owned()));
        assert_eq!(world.moderators(), Vec::<String>::new());
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
