use std::fmt::Write;

use ttymud_text::{Kind, Message, Style};

use crate::settings::Depth;

const RESET: &str = "\x1b[0m";

#[must_use]
pub(crate) fn paint(message: &Message, depth: Depth) -> String {
    if depth == Depth::None {
        return message.plain();
    }
    let mut text = String::new();
    for span in &message.spans {
        match code(span.style, message.kind) {
            Some(code) => {
                let _ = write!(text, "\x1b[{code}m{}{RESET}", span.text);
            }
            None => text.push_str(&span.text),
        }
    }
    text
}

fn code(style: Style, kind: Kind) -> Option<&'static str> {
    match style {
        Style::Plain => match kind {
            Kind::Error => Some("31"),
            Kind::Telegraph => Some("33"),
            Kind::Game | Kind::Chat => None,
        },
        Style::Title => Some("1"),
        Style::Host => Some("36"),
        Style::File => Some("32"),
        Style::Command => Some("33"),
        Style::Player => Some("35"),
        Style::Ice => Some("31"),
        Style::Danger => Some("1;91"),
    }
}

#[cfg(test)]
mod tests {
    use ttymud_text::{Kind, Message, Style};

    use super::paint;
    use crate::settings::Depth;

    fn scene() -> Message {
        Message::game("Routes: ").span(Style::Host, "relay")
    }

    #[test]
    fn no_color_gives_the_plain_text() {
        assert_eq!(paint(&scene(), Depth::None), "Routes: relay");
    }

    #[test]
    fn styles_get_the_colors_of_the_terminal_theme() {
        assert_eq!(
            paint(&scene(), Depth::Basic),
            "Routes: \x1b[36mrelay\x1b[0m"
        );
        assert_eq!(paint(&scene(), Depth::True), paint(&scene(), Depth::Basic));
        let danger = Message::new(Kind::Telegraph)
            .span(Style::Ice, "hound")
            .text(" starts to compile ")
            .span(Style::Danger, "TRACE BURST");
        assert_eq!(
            paint(&danger, Depth::Basic),
            "\x1b[31mhound\x1b[0m\x1b[33m starts to compile \x1b[0m\x1b[1;91mTRACE BURST\x1b[0m"
        );
    }

    #[test]
    fn errors_are_red_and_chat_is_plain() {
        let error = Message::error("No route.");
        assert_eq!(paint(&error, Depth::Basic), "\x1b[31mNo route.\x1b[0m");
        let chat = Message::new(Kind::Chat)
            .text("<")
            .span(Style::Player, "nyx")
            .text("> hi");
        assert_eq!(paint(&chat, Depth::Indexed), "<\x1b[35mnyx\x1b[0m> hi");
    }
}
