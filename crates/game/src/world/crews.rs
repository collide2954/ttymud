use std::collections::BTreeMap;

use ttymud_text::{Kind, Message, Style};

use super::account::{Phase, good_name};
use super::presence::named;
use super::save::{Text, allow, checked, number, problem};
use super::{Output, PlayerId, Step, Thing, World, choose, lines};
use crate::content::ContentError;
use crate::ini::{self, Section};

pub(super) const CREWS: &str = "crews.ini";
pub(super) const MAX_CREW: usize = 4;
pub(super) const ACTIONS: [&str; 6] = ["create", "invite", "join", "leave", "kick", "lead"];
const USAGE: &str = "Usage: crew, crew create <name>, crew invite <handle>, crew join <crew>, crew leave, crew kick <handle>, or crew lead <handle>.";
const RULE: &str = "A crew name has 3 to 16 letters, digits, hyphens, or underscores.";
const NO_CREW: &str = "You are not in a crew. Type crew create <name> to make one.";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Crew {
    pub(super) name: String,
    pub(super) leader: String,
    pub(super) members: Vec<String>,
    pub(super) rep: u32,
}

impl Crew {
    pub(super) fn has(&self, handle: &str) -> bool {
        self.members
            .iter()
            .any(|member| member.eq_ignore_ascii_case(handle))
    }

    fn full(&self) -> Message {
        Message::error(format!(
            "The crew {} is full. A crew has up to {MAX_CREW} handles.",
            self.name
        ))
    }
}

#[derive(Debug, Default)]
pub(super) struct Crews {
    pub(super) list: BTreeMap<String, Crew>,
    invites: Vec<(String, String)>,
    dirty: bool,
}

enum Gone {
    Ended,
    Left(Option<String>),
}

impl Crews {
    pub(super) fn of(&self, handle: &str) -> Option<&Crew> {
        self.list.values().find(|crew| crew.has(handle))
    }

    fn key_of(&self, handle: &str) -> Option<String> {
        let found = self.list.iter().find(|(_, crew)| crew.has(handle));
        found.map(|(key, _)| key.clone())
    }

    pub(super) fn save(&mut self) -> Option<(String, String)> {
        std::mem::take(&mut self.dirty).then(|| (CREWS.to_owned(), self.text()))
    }

    fn text(&self) -> String {
        let mut text = Text::new("# The crews of the server. The server writes this file.");
        for crew in self.list.values() {
            text.section("crew", &crew.name);
            text.pair("leader", &crew.leader);
            text.pair("rep", &crew.rep.to_string());
            for member in &crew.members {
                text.pair("member", member);
            }
        }
        text.finish()
    }

    pub(super) fn read(&mut self, text: &str) -> Result<(), Vec<ContentError>> {
        let (sections, mut errors) = ini::read(text);
        let mut list = BTreeMap::new();
        for section in &sections {
            if section.kind != "crew" {
                let kind = section.kind;
                errors.push(problem(
                    section.line,
                    format!("unknown section type: {kind}"),
                ));
                continue;
            }
            allow(section, &["leader", "rep", "member"], &mut errors);
            if let Some(crew) = read_crew(section, &mut errors) {
                list.insert(crew.name.to_ascii_lowercase(), crew);
            }
        }
        self.list = checked(errors, list)?;
        Ok(())
    }
}

fn read_crew(section: &Section<'_>, errors: &mut Vec<ContentError>) -> Option<Crew> {
    let name = section.id;
    if !good_name(name) {
        errors.push(problem(section.line, format!("bad crew name: {name}")));
        return None;
    }
    let values = |key: &'static str| section.pairs.iter().filter(move |pair| pair.key == key);
    let members: Vec<String> = values("member").map(|pair| pair.value.to_owned()).collect();
    let rep = values("rep")
        .next()
        .and_then(|pair| number(pair, errors))
        .unwrap_or(0);
    let Some(leader) = values("leader").next() else {
        errors.push(problem(section.line, format!("crew {name} has no leader")));
        return None;
    };
    if !members
        .iter()
        .any(|member| member.eq_ignore_ascii_case(leader.value))
    {
        let message = format!("the leader of {name} is not a member: {}", leader.value);
        errors.push(problem(leader.line, message));
        return None;
    }
    if members.len() > MAX_CREW {
        let message = format!("crew {name} has more than {MAX_CREW} members");
        errors.push(problem(section.line, message));
        return None;
    }
    Some(Crew {
        name: name.to_owned(),
        leader: leader.value.to_owned(),
        members,
        rep,
    })
}

impl World {
    pub(super) fn crew(&mut self, player: PlayerId, args: &[String]) -> Result<Step, Message> {
        let name = self.handle(player).unwrap_or_default().to_owned();
        let action = args.first().map(|word| word.to_ascii_lowercase());
        let shown = match (action.as_deref(), args.get(1..).unwrap_or_default()) {
            (None, _) => self.crew_show(&name),
            (Some("create"), [crew]) => self.crew_create(&name, crew)?,
            (Some("invite"), [handle]) => self.crew_invite(player, &name, handle)?,
            (Some("join"), [crew]) => self.crew_join(player, &name, crew)?,
            (Some("leave"), []) => {
                let (crew, ended) = self
                    .quit_crew(player, &name, "leaves")
                    .ok_or_else(|| Message::error(NO_CREW))?;
                let end = if ended { " The crew ends." } else { "" };
                vec![Message::game(format!("You leave the crew {crew}.{end}"))]
            }
            (Some("kick"), [handle]) => self.crew_kick(player, &name, handle)?,
            (Some("lead"), [handle]) => self.crew_lead(player, &name, handle)?,
            _ => return Err(Message::error(USAGE)),
        };
        Ok(lines(shown))
    }

    fn crew_show(&self, name: &str) -> Vec<Message> {
        let Some(crew) = self.crews.of(name) else {
            return vec![Message::game(NO_CREW)];
        };
        let mut members = Message::game("Members: ");
        for (index, member) in crew.members.iter().enumerate() {
            if index > 0 {
                members = members.text(", ");
            }
            members = members.span(Style::Player, member);
            if member.eq_ignore_ascii_case(&crew.leader) {
                members = members.text(" (leader)");
            }
        }
        let title = format!("Crew: {}. Reputation: {}.", crew.name, crew.rep);
        vec![Message::game(title), members]
    }

    fn crew_create(&mut self, name: &str, crew: &str) -> Result<Vec<Message>, Message> {
        if let Some(own) = self.crews.of(name) {
            return Err(Message::error(format!(
                "You are already in the crew {}. Type crew leave first.",
                own.name
            )));
        }
        if !good_name(crew) {
            return Err(Message::error(RULE));
        }
        let key = crew.to_ascii_lowercase();
        if let Some(taken) = self.crews.list.get(&key) {
            let taken = &taken.name;
            return Err(Message::error(format!("The crew name {taken} is taken.")));
        }
        let made = Crew {
            name: crew.to_owned(),
            leader: name.to_owned(),
            members: vec![name.to_owned()],
            rep: 0,
        };
        self.crews.list.insert(key, made);
        self.crews.dirty = true;
        Ok(vec![Message::game(format!(
            "You make the crew {crew}, and you lead it. Type crew invite <handle> to add a handle."
        ))])
    }

    fn led_crew(&self, name: &str) -> Result<String, Message> {
        let key = self
            .crews
            .key_of(name)
            .ok_or_else(|| Message::error(NO_CREW))?;
        let crew = &self.crews.list[&key];
        if !crew.leader.eq_ignore_ascii_case(name) {
            return Err(Message::error(format!(
                "Only the leader of {} can do that. The leader is {}.",
                crew.name, crew.leader
            )));
        }
        Ok(key)
    }

    fn crew_invite(
        &mut self,
        player: PlayerId,
        name: &str,
        handle: &str,
    ) -> Result<Vec<Message>, Message> {
        let key = self.led_crew(name)?;
        let online = self.online();
        let names: Vec<&str> = online.iter().map(|&(_, known)| known).collect();
        let found = choose(Thing::Handle, handle, &names)?;
        let Some((target, invited)) = online
            .iter()
            .find(|&&(_, known)| known == found)
            .map(|&(id, known)| (id, known.to_owned()))
        else {
            return Ok(Vec::new());
        };
        if self.crews.of(&invited).is_some() {
            return Err(Message::error(format!("{invited} is already in a crew.")));
        }
        let crew = &self.crews.list[&key];
        if crew.members.len() >= MAX_CREW {
            return Err(crew.full());
        }
        if self.blocks(target, player) {
            return Err(Message::error(format!(
                "{invited} does not take invites from you."
            )));
        }
        let crew = crew.name.clone();
        let invite = (key, invited.to_ascii_lowercase());
        if !self.crews.invites.contains(&invite) {
            self.crews.invites.push(invite);
        }
        if self.hears(target, player) {
            let message = named(name, &format!(" invites you to the crew {crew}. Type "))
                .span(Style::Command, format!("crew join {crew}"))
                .text(" to join.");
            self.outbox.push(Output {
                player: target,
                message,
            });
        }
        Ok(vec![
            Message::game("You invite ")
                .span(Style::Player, invited)
                .text(format!(" to the crew {crew}.")),
        ])
    }

    fn crew_join(
        &mut self,
        player: PlayerId,
        name: &str,
        crew: &str,
    ) -> Result<Vec<Message>, Message> {
        if let Some(own) = self.crews.of(name) {
            return Err(Message::error(format!(
                "You are already in the crew {}. Type crew leave first.",
                own.name
            )));
        }
        let key = crew.to_ascii_lowercase();
        let me = name.to_ascii_lowercase();
        let invited = self.crews.invites.contains(&(key.clone(), me.clone()));
        let Some(found) = self.crews.list.get(&key).filter(|_| invited) else {
            let shown = self.crews.list.get(&key).map_or(crew, |found| &found.name);
            return Err(Message::error(format!(
                "The crew {shown} did not invite you."
            )));
        };
        if found.members.len() >= MAX_CREW {
            return Err(found.full());
        }
        let crew = found.name.clone();
        self.crews.invites.retain(|(_, invited)| *invited != me);
        if let Some(found) = self.crews.list.get_mut(&key) {
            found.members.push(name.to_owned());
        }
        self.crews.dirty = true;
        self.tell_crew(
            &key,
            player,
            &named(name, &format!(" joins the crew {crew}.")),
        );
        Ok(vec![Message::game(format!("You join the crew {crew}."))])
    }

    fn crew_member(&self, key: &str, handle: &str) -> Result<String, Message> {
        let crew = &self.crews.list[key];
        let members: Vec<&str> = crew.members.iter().map(String::as_str).collect();
        Ok(choose(Thing::Member, handle, &members)?.to_owned())
    }

    fn crew_kick(
        &mut self,
        player: PlayerId,
        name: &str,
        handle: &str,
    ) -> Result<Vec<Message>, Message> {
        let key = self.led_crew(name)?;
        let kicked = self.crew_member(&key, handle)?;
        if kicked.eq_ignore_ascii_case(name) {
            return Err(Message::error("Type crew leave to leave the crew."));
        }
        let crew = self.crews.list[&key].name.clone();
        self.drop_member(&key, &kicked);
        let told = named(name, " kicks ")
            .span(Style::Player, &kicked)
            .text(format!(" out of the crew {crew}."));
        self.tell_crew(&key, player, &told);
        if let Some(target) = self.online_player(&kicked) {
            let message = named(name, &format!(" kicks you out of the crew {crew}."));
            self.outbox.push(Output {
                player: target,
                message,
            });
        }
        Ok(vec![
            Message::game("You kick ")
                .span(Style::Player, kicked)
                .text(format!(" out of the crew {crew}.")),
        ])
    }

    fn crew_lead(
        &mut self,
        player: PlayerId,
        name: &str,
        handle: &str,
    ) -> Result<Vec<Message>, Message> {
        let key = self.led_crew(name)?;
        let leader = self.crew_member(&key, handle)?;
        let Some(crew) = self.crews.list.get_mut(&key) else {
            return Ok(Vec::new());
        };
        crew.leader.clone_from(&leader);
        let told = named(&leader, &format!(" leads the crew {} now.", crew.name));
        self.crews.dirty = true;
        self.tell_crew(&key, player, &told);
        Ok(vec![told])
    }

    pub(super) fn quit_crew(
        &mut self,
        player: PlayerId,
        name: &str,
        how: &str,
    ) -> Option<(String, bool)> {
        let key = self.crews.key_of(name)?;
        let crew = self.crews.list.get(&key)?.name.clone();
        let me = name.to_ascii_lowercase();
        self.crews.invites.retain(|(_, invited)| *invited != me);
        let told = match self.drop_member(&key, name) {
            Gone::Ended => return Some((crew, true)),
            Gone::Left(Some(leader)) => format!(" {how} the crew {crew}. {leader} leads it now."),
            Gone::Left(None) => format!(" {how} the crew {crew}."),
        };
        self.tell_crew(&key, player, &named(name, &told));
        Some((crew, false))
    }

    fn drop_member(&mut self, key: &str, name: &str) -> Gone {
        self.crews.dirty = true;
        let Some(crew) = self.crews.list.get_mut(key) else {
            return Gone::Ended;
        };
        crew.members
            .retain(|member| !member.eq_ignore_ascii_case(name));
        let Some(first) = crew.members.first().cloned() else {
            self.crews.list.remove(key);
            self.crews.invites.retain(|(crew, _)| crew != key);
            return Gone::Ended;
        };
        if crew.leader.eq_ignore_ascii_case(name) {
            crew.leader.clone_from(&first);
            return Gone::Left(Some(first));
        }
        Gone::Left(None)
    }

    pub(super) fn crew_say(&mut self, player: PlayerId, text: &str) -> Vec<Output> {
        let name = self.handle(player).unwrap_or_default().to_owned();
        let Some(key) = self.crews.key_of(&name) else {
            return self.deliver(player, vec![Message::error(NO_CREW)]);
        };
        if text.is_empty() {
            return self.deliver(player, vec![Message::error("Usage: #crew <text>.")]);
        }
        if let Some(refusal) = self.muted(player).or_else(|| self.repeated(player, text)) {
            return self.deliver(player, vec![refusal]);
        }
        let message = Message::new(Kind::Chat)
            .text("[crew] <")
            .span(Style::Player, &name)
            .text("> ")
            .text(text);
        let crew = &self.crews.list[&key];
        let mates: Vec<PlayerId> = self
            .players
            .iter()
            .filter(|&(&other, state)| {
                other != player && state.phase == Phase::Playing && crew.has(&state.name)
            })
            .map(|(&other, _)| other)
            .filter(|&other| self.hears(other, player))
            .collect();
        self.outbox.extend(mates.into_iter().map(|other| Output {
            player: other,
            message: message.clone(),
        }));
        self.remember(player, None, "#crew", text);
        self.deliver(player, vec![message])
    }

    pub(super) fn tell_crew(&mut self, key: &str, except: PlayerId, message: &Message) {
        let Some(crew) = self.crews.list.get(key) else {
            return;
        };
        let targets: Vec<PlayerId> = self
            .players
            .iter()
            .filter(|&(&id, state)| {
                id != except && state.phase == Phase::Playing && crew.has(&state.name)
            })
            .map(|(&id, _)| id)
            .collect();
        self.outbox.extend(targets.into_iter().map(|player| Output {
            player,
            message: message.clone(),
        }));
    }

    fn online_player(&self, handle: &str) -> Option<PlayerId> {
        self.players
            .iter()
            .find(|(_, state)| {
                state.phase == Phase::Playing && state.name.eq_ignore_ascii_case(handle)
            })
            .map(|(&id, _)| id)
    }

    pub(super) fn crew_choices(&self, player: PlayerId, action: &str) -> Vec<&str> {
        let name = self.handle(player).unwrap_or_default();
        match action.to_ascii_lowercase().as_str() {
            "invite" => self
                .online_names()
                .into_iter()
                .filter(|other| !other.eq_ignore_ascii_case(name))
                .collect(),
            "kick" | "lead" => self
                .crews
                .of(name)
                .into_iter()
                .flat_map(|crew| &crew.members)
                .map(String::as_str)
                .filter(|member| !member.eq_ignore_ascii_case(name))
                .collect(),
            "join" => {
                let me = name.to_ascii_lowercase();
                self.crews
                    .invites
                    .iter()
                    .filter(|(_, invited)| *invited == me)
                    .filter_map(|(key, _)| self.crews.list.get(key))
                    .map(|crew| crew.name.as_str())
                    .collect()
            }
            _ => Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::testing::{heard, plain, world};
    use crate::{PlayerId, World};

    fn online(names: &[&str]) -> (World, Vec<PlayerId>) {
        let mut world = world();
        let players = names.iter().map(|name| world.join(name).0).collect();
        (world, players)
    }

    fn crewed() -> (World, Vec<PlayerId>) {
        let (mut world, players) = online(&["nyx", "zer0", "kid"]);
        world.command(players[0], "crew create Ghostwire");
        world.command(players[0], "crew invite zer0");
        world.command(players[1], "crew join ghostwire");
        (world, players)
    }

    #[test]
    fn a_handle_makes_a_crew_and_leads_it() {
        let (mut world, players) = online(&["nyx"]);
        let nyx = players[0];
        assert_eq!(
            plain(&world.command(nyx, "crew")),
            ["You are not in a crew. Type crew create <name> to make one."]
        );
        assert_eq!(
            plain(&world.command(nyx, "crew create Ghostwire")),
            [
                "You make the crew Ghostwire, and you lead it. Type crew invite <handle> to add a handle."
            ]
        );
        assert_eq!(
            plain(&world.command(nyx, "crew")),
            ["Crew: Ghostwire. Reputation: 0.", "Members: nyx (leader)"]
        );
    }

    #[test]
    fn a_crew_name_follows_the_handle_rules_and_is_unique() {
        let (mut world, players) = online(&["nyx", "zer0"]);
        let (nyx, zer0) = (players[0], players[1]);
        assert_eq!(
            plain(&world.command(nyx, "crew create x")),
            ["A crew name has 3 to 16 letters, digits, hyphens, or underscores."]
        );
        world.command(nyx, "crew create Ghostwire");
        assert_eq!(
            plain(&world.command(zer0, "crew create GHOSTWIRE")),
            ["The crew name Ghostwire is taken."]
        );
        assert_eq!(
            plain(&world.command(nyx, "crew create Other")),
            ["You are already in the crew Ghostwire. Type crew leave first."]
        );
        assert_eq!(
            plain(&world.command(nyx, "crew dance")),
            [
                "Usage: crew, crew create <name>, crew invite <handle>, crew join <crew>, crew leave, crew kick <handle>, or crew lead <handle>."
            ]
        );
    }

    #[test]
    fn the_leader_invites_a_handle_and_it_joins() {
        let (mut world, players) = online(&["nyx", "zer0", "kid"]);
        let (nyx, zer0, kid) = (players[0], players[1], players[2]);
        world.command(nyx, "crew create Ghostwire");
        let outputs = world.command(nyx, "crew invite zer0");
        assert_eq!(
            heard(&outputs, nyx),
            ["You invite zer0 to the crew Ghostwire."]
        );
        assert_eq!(
            heard(&outputs, zer0),
            ["nyx invites you to the crew Ghostwire. Type crew join Ghostwire to join."]
        );
        assert_eq!(
            plain(&world.command(kid, "crew join ghostwire")),
            ["The crew Ghostwire did not invite you."]
        );
        let outputs = world.command(zer0, "crew join ghostwire");
        assert_eq!(heard(&outputs, zer0), ["You join the crew Ghostwire."]);
        assert_eq!(heard(&outputs, nyx), ["zer0 joins the crew Ghostwire."]);
        assert_eq!(
            plain(&world.command(zer0, "crew"))[1],
            "Members: nyx (leader), zer0"
        );
    }

    #[test]
    fn only_the_leader_invites_and_a_crew_has_at_most_four_handles() {
        let (mut world, players) = online(&["nyx", "zer0", "kid", "moth", "wisp"]);
        let nyx = players[0];
        world.command(nyx, "crew create Ghostwire");
        for (index, name) in ["zer0", "kid", "moth"].into_iter().enumerate() {
            world.command(nyx, &format!("crew invite {name}"));
            world.command(players[index + 1], "crew join Ghostwire");
        }
        assert_eq!(
            plain(&world.command(players[1], "crew invite wisp")),
            ["Only the leader of Ghostwire can do that. The leader is nyx."]
        );
        assert_eq!(
            plain(&world.command(nyx, "crew invite wisp")),
            ["The crew Ghostwire is full. A crew has up to 4 handles."]
        );
        assert_eq!(
            plain(&world.command(nyx, "crew invite zer0")),
            ["zer0 is already in a crew."]
        );
    }

    #[test]
    fn a_blocked_leader_cannot_invite() {
        let (mut world, players) = online(&["nyx", "zer0"]);
        let (nyx, zer0) = (players[0], players[1]);
        world.command(zer0, "block nyx");
        world.command(nyx, "crew create Ghostwire");
        assert_eq!(
            plain(&world.command(nyx, "crew invite zer0")),
            ["zer0 does not take invites from you."]
        );
    }

    #[test]
    fn the_lead_passes_on_when_the_leader_leaves_and_the_last_handle_ends_the_crew() {
        let (mut world, players) = crewed();
        let (nyx, zer0) = (players[0], players[1]);
        let outputs = world.command(nyx, "crew leave");
        assert_eq!(heard(&outputs, nyx), ["You leave the crew Ghostwire."]);
        assert_eq!(
            heard(&outputs, zer0),
            ["nyx leaves the crew Ghostwire. zer0 leads it now."]
        );
        assert_eq!(
            plain(&world.command(zer0, "crew leave")),
            ["You leave the crew Ghostwire. The crew ends."]
        );
        assert_eq!(
            plain(&world.command(nyx, "crew leave")),
            ["You are not in a crew. Type crew create <name> to make one."]
        );
        assert_eq!(
            plain(&world.command(nyx, "crew create Ghostwire"))[0],
            "You make the crew Ghostwire, and you lead it. Type crew invite <handle> to add a handle."
        );
    }

    #[test]
    fn the_leader_can_kick_a_member_or_hand_over_the_lead() {
        let (mut world, players) = crewed();
        let (nyx, zer0, kid) = (players[0], players[1], players[2]);
        world.command(nyx, "crew invite kid");
        world.command(kid, "crew join ghostwire");
        let outputs = world.command(nyx, "crew kick kid");
        assert_eq!(
            heard(&outputs, nyx),
            ["You kick kid out of the crew Ghostwire."]
        );
        assert_eq!(
            heard(&outputs, kid),
            ["nyx kicks you out of the crew Ghostwire."]
        );
        assert_eq!(
            heard(&outputs, zer0),
            ["nyx kicks kid out of the crew Ghostwire."]
        );
        let outputs = world.command(nyx, "crew lead zer0");
        assert_eq!(heard(&outputs, nyx), ["zer0 leads the crew Ghostwire now."]);
        assert_eq!(
            heard(&outputs, zer0),
            ["zer0 leads the crew Ghostwire now."]
        );
        assert_eq!(
            plain(&world.command(nyx, "crew kick zer0")),
            ["Only the leader of Ghostwire can do that. The leader is zer0."]
        );
        assert_eq!(
            plain(&world.command(zer0, "crew kick zer0")),
            ["Type crew leave to leave the crew."]
        );
        assert_eq!(
            plain(&world.command(zer0, "crew kick kid")),
            ["No crewmate named kid. Crew: nyx, zer0"]
        );
    }

    #[test]
    fn crew_chat_reaches_the_crewmates_on_all_hosts() {
        let (mut world, players) = crewed();
        let (nyx, zer0, kid) = (players[0], players[1], players[2]);
        world.command(zer0, "go market");
        let outputs = world.command(nyx, "#crew meet at the gate");
        assert_eq!(heard(&outputs, nyx), ["[crew] <nyx> meet at the gate"]);
        assert_eq!(heard(&outputs, zer0), ["[crew] <nyx> meet at the gate"]);
        assert_eq!(heard(&outputs, kid), Vec::<String>::new());
        assert_eq!(
            plain(&world.command(kid, "#crew hi")),
            ["You are not in a crew. Type crew create <name> to make one."]
        );
        assert_eq!(
            plain(&world.command(nyx, "#CREW")),
            ["Usage: #crew <text>."]
        );
        world.command(zer0, "ignore nyx");
        let outputs = world.command(nyx, "#crew again");
        assert_eq!(heard(&outputs, zer0), Vec::<String>::new());
    }

    #[test]
    fn channels_lists_the_crew_channel_for_a_crewmate() {
        let (mut world, players) = crewed();
        let shown = plain(&world.command(players[0], "channels"));
        let line = "#crew: The channel of the crew Ghostwire. Only your crewmates hear it.";
        assert!(shown.contains(&line.to_owned()), "{shown:?}");
        let other = plain(&world.command(players[2], "channels"));
        assert!(
            !other.iter().any(|line| line.starts_with("#crew")),
            "{other:?}"
        );
    }

    #[test]
    fn a_burn_takes_the_handle_out_of_its_crew() {
        let (mut world, players) = crewed();
        let (nyx, zer0) = (players[0], players[1]);
        world.burn(zer0);
        let shown = heard(&world.command(nyx, "crew"), nyx);
        assert_eq!(
            shown[..3],
            [
                "Crew: Ghostwire. Reputation: 0.",
                "Members: nyx (leader)",
                "zer0 burns and leaves the crew Ghostwire."
            ]
        );
    }

    #[test]
    fn crews_come_back_from_crews_ini() {
        let (mut world, _) = crewed();
        let saves = world.saves();
        let text = saves
            .iter()
            .find(|(path, _)| path == "crews.ini")
            .map_or_default(|(_, text)| text.clone());
        assert_eq!(
            text,
            "# The crews of the server. The server writes this file.\n\n[crew Ghostwire]\nleader = nyx\nrep = 0\nmember = nyx\nmember = zer0\n"
        );
        let mut fresh = crate::testing::world();
        fresh.restore("crews.ini", &text).unwrap();
        let (nyx, _) = fresh.join("nyx");
        assert_eq!(
            plain(&fresh.command(nyx, "crew"))[1],
            "Members: nyx (leader), zer0"
        );
        let errors: Vec<String> = fresh
            .restore(
                "crews.ini",
                "[crew x]\nleader = a\nmember = a\n[crew Moths]\nleader = b\nmember = a\n",
            )
            .unwrap_err()
            .iter()
            .map(ToString::to_string)
            .collect();
        assert_eq!(
            errors,
            [
                "line 1: bad crew name: x",
                "line 5: the leader of Moths is not a member: b"
            ]
        );
    }
}
