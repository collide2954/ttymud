use ttymud_text::{Kind, Message, Style};

use super::{Output, PlayerId, Step, Thing, World, choose, usage};
use crate::content::ChannelDef;

#[derive(Debug)]
pub(super) struct Channel {
    pub(super) id: String,
    pub(super) desc: String,
    pub(super) auto: bool,
}

impl From<ChannelDef> for Channel {
    fn from(def: ChannelDef) -> Self {
        Self {
            id: def.id,
            desc: def.desc,
            auto: def.auto,
        }
    }
}

impl World {
    pub(super) fn auto_channels(&self) -> Vec<String> {
        let auto = self.channels.iter().filter(|channel| channel.auto);
        auto.map(|channel| channel.id.clone()).collect()
    }

    fn channel_named(&self, word: &str) -> Result<String, Message> {
        let names: Vec<&str> = self
            .channels
            .iter()
            .map(|channel| channel.id.as_str())
            .collect();
        choose(Thing::Channel, &word.to_lowercase(), &names).map(str::to_owned)
    }

    fn joined(&self, player: PlayerId) -> &[String] {
        self.players
            .get(&player)
            .map_or(&[][..], |state| state.channels.as_slice())
    }

    pub(super) fn list_channels(&self, player: PlayerId) -> Step {
        let joined = self.joined(player);
        let mut shown: Vec<Message> = self
            .channels
            .iter()
            .map(|channel| {
                let mut line = Message::game("")
                    .span(Style::Command, format!("#{}", channel.id))
                    .text(format!(": {}", channel.desc));
                if joined.contains(&channel.id) {
                    line = line.text(" You are in this channel.");
                }
                line
            })
            .collect();
        shown.push(
            Message::game("Type ")
                .span(Style::Command, "#<channel> <text>")
                .text(" to talk. Type ")
                .span(Style::Command, "join")
                .text(" or ")
                .span(Style::Command, "leave")
                .text(" to change your channels."),
        );
        super::lines(shown)
    }

    pub(super) fn join_channel(
        &mut self,
        player: PlayerId,
        args: &[String],
    ) -> Result<Step, Message> {
        let id = self.pick_channel("join", args)?;
        let Some(state) = self.players.get_mut(&player) else {
            return Ok(Step::default());
        };
        if state.channels.contains(&id) {
            return Err(Message::error(format!("You are already in #{id}.")));
        }
        state.channels.push(id.clone());
        Ok(Step::show(vec![Message::game(format!("You join #{id}."))]))
    }

    pub(super) fn leave_channel(
        &mut self,
        player: PlayerId,
        args: &[String],
    ) -> Result<Step, Message> {
        let id = self.pick_channel("leave", args)?;
        let Some(state) = self.players.get_mut(&player) else {
            return Ok(Step::default());
        };
        if !state.channels.contains(&id) {
            return Err(Message::error(format!("You are not in #{id}.")));
        }
        state.channels.retain(|known| known != &id);
        Ok(Step::show(vec![Message::game(format!("You leave #{id}."))]))
    }

    fn pick_channel(&self, command: &str, args: &[String]) -> Result<String, Message> {
        let [word] = args else {
            let names: Vec<&str> = self
                .channels
                .iter()
                .map(|channel| channel.id.as_str())
                .collect();
            return Err(usage(
                &format!("{command} <channel>"),
                Thing::Channel,
                &names,
            ));
        };
        self.channel_named(word.trim_start_matches('#'))
    }

    pub(super) fn channel_say(&mut self, player: PlayerId, name: &str, text: &str) -> Vec<Output> {
        let id = match self.channel_named(name) {
            Ok(id) => id,
            Err(message) => return self.deliver(player, vec![message]),
        };
        if text.is_empty() {
            return self.deliver(player, vec![Message::error("Usage: #<channel> <text>.")]);
        }
        if !self.joined(player).contains(&id) {
            let refusal = Message::error(format!("You are not in #{id}. Type "))
                .span(Style::Command, format!("join {id}"))
                .text(" first.");
            return self.deliver(player, vec![refusal]);
        }
        let refusal = self.muted(player).or_else(|| self.repeated(player, text));
        if let Some(refusal) = refusal {
            return self.deliver(player, vec![refusal]);
        }
        let Some(name) = self.players.get(&player).map(|state| state.name.clone()) else {
            return Vec::new();
        };
        let message = Message::new(Kind::Chat)
            .text(format!("[{id}] <"))
            .span(Style::Player, name)
            .text("> ")
            .text(text);
        let members: Vec<PlayerId> = self
            .players
            .iter()
            .filter(|&(&other, state)| {
                other != player
                    && state.phase == super::account::Phase::Playing
                    && state.channels.contains(&id)
            })
            .map(|(&other, _)| other)
            .collect();
        let members: Vec<PlayerId> = members
            .into_iter()
            .filter(|&other| self.hears(other, player))
            .collect();
        self.outbox.extend(members.into_iter().map(|other| Output {
            player: other,
            message: message.clone(),
        }));
        self.remember(player, None, &format!("#{id}"), text);
        self.deliver(player, vec![message])
    }
}

#[cfg(test)]
mod tests {
    use ttymud_text::Kind;

    use crate::testing::{heard, plain, run, world};
    use crate::{PlayerId, World};

    fn three() -> (World, PlayerId, PlayerId, PlayerId) {
        let mut world = world();
        let (ghost, _) = world.join("ghost");
        let (nyx, _) = world.join("nyx");
        let (zer0, _) = world.join("zer0");
        world.command(zer0, "go market");
        (world, ghost, nyx, zer0)
    }

    #[test]
    fn a_channel_message_reaches_the_members_on_all_hosts() {
        let (mut world, ghost, nyx, zer0) = three();
        let outputs = world.command(ghost, "#city hello all");
        for player in [ghost, nyx, zer0] {
            assert_eq!(heard(&outputs, player), ["[city] <ghost> hello all"]);
        }
        assert!(
            outputs
                .iter()
                .all(|output| output.message.kind == Kind::Chat)
        );
    }

    #[test]
    fn only_members_hear_a_channel() {
        let (mut world, ghost, nyx, _) = three();
        world.command(nyx, "leave city");
        let outputs = world.command(ghost, "#city hi");
        assert_eq!(heard(&outputs, nyx), Vec::<String>::new());
    }

    #[test]
    fn channels_lists_the_channels_and_marks_yours() {
        assert_eq!(
            run("channels"),
            [
                "#city: All of the test city. You are in this channel.",
                "#trade: Buy and sell.",
                "Type #<channel> <text> to talk. Type join or leave to change your channels.",
            ]
        );
    }

    #[test]
    fn join_and_leave_change_the_channels() {
        let (mut world, ghost, _, _) = three();
        let mut said = |line: &str| plain(&world.command(ghost, line));
        assert_eq!(said("join trade"), ["You join #trade."]);
        assert_eq!(said("join TRADE"), ["You are already in #trade."]);
        assert_eq!(said("leave tr"), ["You leave #trade."]);
        assert_eq!(said("leave trade"), ["You are not in #trade."]);
        assert_eq!(
            said("join nope"),
            ["No channel named nope. Channels: city, trade"]
        );
    }

    #[test]
    fn a_channel_message_needs_a_member_and_text() {
        assert_eq!(
            run("#trade hi"),
            ["You are not in #trade. Type join trade first."]
        );
        assert_eq!(run("#city"), ["Usage: #<channel> <text>."]);
        assert_eq!(
            run("#nope hi"),
            ["No channel named nope. Channels: city, trade"]
        );
        assert_eq!(run("#c a | b"), ["[city] <ghost> a | b"]);
    }
}
