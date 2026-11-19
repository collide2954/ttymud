use ttymud_text::{Kind, Message, Style, sanitize};

use super::{Output, PlayerId, World};

#[derive(Debug, Clone, Copy)]
pub(super) enum Chat<'a> {
    Say(&'a str),
    Channel(&'a str, &'a str),
    Tell(&'a str, &'a str),
    Reply(&'a str),
}

const REPEAT: u64 = 30;

#[must_use]
pub fn is_chat(line: &str) -> bool {
    parse(&sanitize(line)).is_some()
}

pub(super) fn parse(line: &str) -> Option<Chat<'_>> {
    let line = line.trim_start();
    if let Some(rest) = line.strip_prefix('\'') {
        return Some(Chat::Say(rest.trim()));
    }
    if let Some(rest) = line.strip_prefix('#') {
        let (name, text) = rest.split_once(char::is_whitespace).unwrap_or((rest, ""));
        return Some(Chat::Channel(name, text.trim()));
    }
    let (word, rest) = line.split_once(char::is_whitespace).unwrap_or((line, ""));
    let rest = rest.trim();
    match word.to_ascii_lowercase().as_str() {
        "say" => Some(Chat::Say(rest)),
        "reply" => Some(Chat::Reply(rest)),
        "tell" => {
            let (name, text) = rest.split_once(char::is_whitespace).unwrap_or((rest, ""));
            Some(Chat::Tell(name, text.trim()))
        }
        _ => None,
    }
}

impl World {
    pub(super) fn chat(&mut self, player: PlayerId, chat: Chat<'_>) -> Vec<Output> {
        match chat {
            Chat::Say(text) => self.say(player, text),
            Chat::Channel(name, text) => self.channel_say(player, name, text),
            Chat::Tell(name, text) => self.tell(player, name, text),
            Chat::Reply(text) => self.reply(player, text),
        }
    }

    pub(super) fn repeated(&mut self, player: PlayerId, text: &str) -> Option<Message> {
        let now = self.now;
        let state = self.players.get_mut(&player)?;
        if let Some((last, when)) = &state.last_chat
            && last == text
            && now.saturating_sub(*when) < REPEAT
        {
            return Some(Message::error(format!(
                "You sent the same message less than {REPEAT} seconds ago."
            )));
        }
        state.last_chat = Some((text.to_owned(), now));
        None
    }

    pub(super) fn say(&mut self, player: PlayerId, text: &str) -> Vec<Output> {
        if text.is_empty() {
            return self.deliver(player, vec![Message::error("Usage: say <text>.")]);
        }
        let refusal = self.muted(player).or_else(|| self.repeated(player, text));
        if let Some(refusal) = refusal {
            return self.deliver(player, vec![refusal]);
        }
        let Some(name) = self.players.get(&player).map(|state| state.name.clone()) else {
            return Vec::new();
        };
        let message = Message::new(Kind::Chat)
            .text("<")
            .span(Style::Player, name)
            .text("> ")
            .text(text);
        let here = self.here(player);
        let listeners: Vec<PlayerId> = self
            .others(here, player)
            .map(|(other, _)| other)
            .filter(|&other| self.hears(other, player))
            .collect();
        self.outbox
            .extend(listeners.into_iter().map(|other| Output {
                player: other,
                message: message.clone(),
            }));
        let place = self.hosts[here.0].id.clone();
        self.remember(player, None, &place, text);
        self.deliver(player, vec![message])
    }
}

#[cfg(test)]
mod tests {
    use ttymud_text::Kind;

    use crate::testing::{heard, run, world};
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
    fn say_reaches_everyone_on_the_host() {
        let (mut world, ghost, nyx, zer0) = three();
        let outputs = world.command(ghost, "say hello world");
        assert_eq!(heard(&outputs, ghost), ["<ghost> hello world"]);
        assert_eq!(heard(&outputs, nyx), ["<ghost> hello world"]);
        assert_eq!(heard(&outputs, zer0), Vec::<String>::new());
    }

    #[test]
    fn a_quote_is_short_for_say() {
        assert_eq!(run("'hello"), ["<ghost> hello"]);
    }

    #[test]
    fn say_ignores_case() {
        assert_eq!(run("SAY hi"), ["<ghost> hi"]);
    }

    #[test]
    fn say_keeps_pipes_and_quotes_as_text() {
        assert_eq!(
            run("say try ls | grep \"txt\""),
            ["<ghost> try ls | grep \"txt\""]
        );
    }

    #[test]
    fn say_needs_text() {
        assert_eq!(run("say"), ["Usage: say <text>."]);
        assert_eq!(run("'   "), ["Usage: say <text>."]);
    }

    #[test]
    fn chat_has_the_chat_kind() {
        let (mut world, ghost, _, _) = three();
        let outputs = world.command(ghost, "say hi");
        assert!(
            outputs
                .iter()
                .all(|output| output.message.kind == Kind::Chat)
        );
    }

    #[test]
    fn say_takes_no_input_from_a_pipe() {
        assert_eq!(run("ls | say hi"), ["say takes no input from a pipe."]);
    }

    #[test]
    fn chat_text_is_sanitized() {
        assert_eq!(run("say \u{1b}[31mred\u{202e}"), ["<ghost> [31mred"]);
    }

    #[test]
    fn chat_lines_are_known_as_chat() {
        use crate::is_chat;
        assert!(is_chat("say hi"));
        assert!(is_chat("'hi"));
        assert!(is_chat("#city hi"));
        assert!(is_chat("s\u{200b}ay hi"));
        assert!(!is_chat("look"));
        assert!(!is_chat("sayx"));
    }

    #[test]
    fn a_word_that_starts_with_say_is_not_say() {
        assert_eq!(run("sayx"), ["Unknown command: sayx. Did you mean say?"]);
    }
}
