use ttymud_text::{Kind, Message, Style};

use super::account::Phase;
use super::{Output, PlayerId, Step, Thing, World, choose, lines, minute_word};

const NEW_ACCOUNT: u64 = 600;
const NOTE: &str = "Note: the operator of this server can read private messages.";

impl World {
    pub(super) fn online(&self) -> Vec<(PlayerId, &str)> {
        self.players
            .iter()
            .filter(|(_, state)| state.phase == Phase::Playing)
            .map(|(&player, state)| (player, state.name.as_str()))
            .collect()
    }

    pub(super) fn online_names(&self) -> Vec<&str> {
        self.online().into_iter().map(|(_, name)| name).collect()
    }

    pub(super) fn who(&self) -> Step {
        let mut line = Message::game("Online: ");
        for (index, (player, name)) in self.online().into_iter().enumerate() {
            if index > 0 {
                line = line.text(", ");
            }
            line = line.span(Style::Player, name);
            if self.is_moderator(player) {
                line = line.text(" (moderator)");
            }
        }
        lines(vec![line])
    }

    fn new_account_wait(&self, player: PlayerId) -> Option<Message> {
        let state = self.players.get(&player)?;
        state.account.as_ref()?;
        let age = self.now.saturating_sub(state.social.created);
        (age < NEW_ACCOUNT).then(|| {
            let left = (NEW_ACCOUNT - age).div_ceil(60);
            let unit = minute_word(left);
            Message::error(format!(
                "New accounts can send private messages after 10 minutes. Wait {left} more {unit}."
            ))
        })
    }

    pub(super) fn tell(&mut self, player: PlayerId, name: &str, text: &str) -> Vec<Output> {
        if name.is_empty() || text.is_empty() {
            return self.deliver(player, vec![Message::error("Usage: tell <handle> <text>.")]);
        }
        let online = self.online();
        let names: Vec<&str> = online.iter().map(|&(_, name)| name).collect();
        let target = match choose(Thing::Handle, name, &names) {
            Ok(found) => online
                .iter()
                .find(|&&(_, known)| known == found)
                .map(|&(id, _)| id),
            Err(message) => return self.deliver(player, vec![message]),
        };
        let Some(target) = target else {
            return Vec::new();
        };
        if target == player {
            return self.deliver(player, vec![Message::error("You cannot tell yourself.")]);
        }
        let refusal = self
            .muted(player)
            .or_else(|| self.new_account_wait(player))
            .or_else(|| self.repeated(player, text));
        if let Some(refusal) = refusal {
            return self.deliver(player, vec![refusal]);
        }
        let (Some(from), Some(to)) = (self.players.get(&player), self.players.get(&target)) else {
            return Vec::new();
        };
        let (from, to) = (from.name.clone(), to.name.clone());
        if self.blocks(target, player) {
            let refusal = Message::error("")
                .span(Style::Player, &to)
                .text(" does not take private messages from you.");
            return self.deliver(player, vec![refusal]);
        }
        self.remember(player, Some(target), "tell", text);
        if self.hears(target, player) {
            let told = Message::new(Kind::Chat)
                .span(Style::Player, &from)
                .text(format!(" tells you: {text}"));
            if let Some(state) = self.players.get_mut(&target) {
                state.reply_to = Some(from);
            }
            self.outbox.push(Output {
                player: target,
                message: told,
            });
        }
        let mut shown = Vec::new();
        if let Some(state) = self.players.get_mut(&player)
            && !std::mem::replace(&mut state.social.warned, true)
        {
            shown.push(Message::game(NOTE));
        }
        shown.push(
            Message::new(Kind::Chat)
                .text("You tell ")
                .span(Style::Player, to)
                .text(format!(": {text}")),
        );
        self.deliver(player, shown)
    }

    pub(super) fn reply(&mut self, player: PlayerId, text: &str) -> Vec<Output> {
        if text.is_empty() {
            return self.deliver(player, vec![Message::error("Usage: reply <text>.")]);
        }
        let last = self
            .players
            .get(&player)
            .and_then(|state| state.reply_to.clone());
        let Some(name) = last else {
            let nobody = Message::error("Nobody has told you anything yet.");
            return self.deliver(player, vec![nobody]);
        };
        self.tell(player, &name, text)
    }
}

#[cfg(test)]
mod tests {
    use crate::testing::{heard, plain, run, world};
    use crate::{PlayerId, World};

    const NOTE: &str = "Note: the operator of this server can read private messages.";

    fn three() -> (World, PlayerId, PlayerId, PlayerId) {
        let mut world = world();
        let (ghost, _) = world.join("ghost");
        let (nyx, _) = world.join("nyx");
        let (zer0, _) = world.join("zer0");
        world.command(zer0, "go market");
        (world, ghost, nyx, zer0)
    }

    #[test]
    fn who_lists_the_handles_online() {
        let (mut world, ghost, _, _) = three();
        assert_eq!(
            plain(&world.command(ghost, "who")),
            ["Online: ghost, nyx, zer0"]
        );
    }

    #[test]
    fn tell_reaches_one_handle_on_any_host() {
        let (mut world, ghost, nyx, zer0) = three();
        let outputs = world.command(ghost, "tell zer0 meet me | now");
        assert_eq!(
            heard(&outputs, ghost),
            [NOTE, "You tell zer0: meet me | now"]
        );
        assert_eq!(heard(&outputs, zer0), ["ghost tells you: meet me | now"]);
        assert_eq!(heard(&outputs, nyx), Vec::<String>::new());
        let outputs = world.command(ghost, "tell ZER0 again");
        assert_eq!(heard(&outputs, ghost), ["You tell zer0: again"]);
    }

    #[test]
    fn reply_answers_the_last_handle_that_told_you() {
        let (mut world, ghost, _, zer0) = three();
        world.command(ghost, "tell zer0 hi");
        let outputs = world.command(zer0, "reply hi back");
        assert_eq!(heard(&outputs, ghost), ["zer0 tells you: hi back"]);
        assert_eq!(run("reply hi"), ["Nobody has told you anything yet."]);
    }

    #[test]
    fn tell_needs_an_online_handle_and_text() {
        let (mut world, ghost, _, _) = three();
        let mut said = |line: &str| plain(&world.command(ghost, line));
        assert_eq!(
            said("tell nobody hi"),
            ["No handle online named nobody. Online: ghost, nyx, zer0"]
        );
        assert_eq!(said("tell ghost hi"), ["You cannot tell yourself."]);
        assert_eq!(said("tell nyx"), ["Usage: tell <handle> <text>."]);
        assert_eq!(said("reply"), ["Usage: reply <text>."]);
    }

    #[test]
    fn the_same_message_twice_in_30_seconds_is_refused() {
        let (mut world, ghost, _, _) = three();
        world.set_time(100);
        world.command(ghost, "say hi");
        let refused = ["You sent the same message less than 30 seconds ago."];
        assert_eq!(plain(&world.command(ghost, "say hi")), refused);
        assert_eq!(plain(&world.command(ghost, "#city hi")), refused);
        world.set_time(130);
        assert_eq!(
            heard(&world.command(ghost, "say hi"), ghost),
            ["<ghost> hi"]
        );
    }

    #[test]
    fn a_new_account_waits_10_minutes_to_tell() {
        let mut world = world();
        world.set_time(1_000);
        world.join("ghost");
        let (player, _) = world.connect("a");
        for line in ["nyx", "breaker"] {
            world.command(player, line);
        }
        world.set_time(1_061);
        assert_eq!(
            plain(&world.command(player, "tell ghost hi")),
            ["New accounts can send private messages after 10 minutes. Wait 9 more minutes."]
        );
        world.set_time(1_600);
        assert_eq!(
            heard(&world.command(player, "tell ghost hi"), player),
            [NOTE, "You tell ghost: hi"]
        );
    }

    #[test]
    fn tell_and_reply_lines_are_chat() {
        use crate::is_chat;
        assert!(is_chat("tell nyx hi"));
        assert!(is_chat("REPLY hi"));
        assert!(!is_chat("teller"));
    }
}
