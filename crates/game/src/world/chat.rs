use ttymud_text::{Kind, Message, Style, sanitize};

use super::{Output, PlayerId, World};

#[must_use]
pub fn is_chat(line: &str) -> bool {
    chat_text(&sanitize(line)).is_some()
}

pub(super) fn chat_text(line: &str) -> Option<&str> {
    let line = line.trim_start();
    if let Some(rest) = line.strip_prefix('\'') {
        return Some(rest.trim());
    }
    let (word, rest) = line.split_once(char::is_whitespace).unwrap_or((line, ""));
    word.eq_ignore_ascii_case("say").then_some(rest.trim())
}

impl World {
    pub(super) fn say(&mut self, player: PlayerId, text: &str) -> Vec<Output> {
        if text.is_empty() {
            return self.deliver(player, vec![Message::error("Usage: say <text>.")]);
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
        self.notify(here, player, &message);
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
        assert!(is_chat("s\u{200b}ay hi"));
        assert!(!is_chat("look"));
        assert!(!is_chat("sayx"));
    }

    #[test]
    fn a_word_that_starts_with_say_is_not_say() {
        assert_eq!(run("sayx"), ["Unknown command: sayx. Did you mean say?"]);
    }
}
