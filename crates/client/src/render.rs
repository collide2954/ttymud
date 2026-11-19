use ttymud_text::clean;
use ttymud_wire::ServerMessage;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Shown {
    Line(String),
    Prompt(String),
    Closed(String),
}

#[must_use]
pub fn render(message: &ServerMessage) -> Shown {
    match message {
        ServerMessage::Welcome { city, name } => {
            let (city, name) = (clean(city), clean(name));
            Shown::Line(format!("Connected to {city} as {name}."))
        }
        ServerMessage::Text(text) => Shown::Line(clean(&text.plain())),
        ServerMessage::Prompt(prompt) => Shown::Prompt(clean(prompt)),
        ServerMessage::Close(reason) => Shown::Closed(clean(reason)),
    }
}

#[cfg(test)]
mod tests {
    use ttymud_text::{Message, Style};
    use ttymud_wire::ServerMessage;

    use super::{Shown, render};

    #[test]
    fn a_welcome_names_the_city_and_the_player() {
        let welcome = ServerMessage::Welcome {
            city: "Port Ostra".to_owned(),
            name: "anon-3f9a2c71".to_owned(),
        };
        assert_eq!(
            render(&welcome),
            Shown::Line("Connected to Port Ostra as anon-3f9a2c71.".to_owned())
        );
    }

    #[test]
    fn text_shows_as_one_plain_line() {
        let text = ServerMessage::Text(Message::game("Routes: ").span(Style::Host, "relay"));
        assert_eq!(render(&text), Shown::Line("Routes: relay".to_owned()));
    }

    #[test]
    fn prompts_and_closes_have_their_own_kind() {
        let prompt = ServerMessage::Prompt("ghost@relay $ ".to_owned());
        assert_eq!(render(&prompt), Shown::Prompt("ghost@relay $ ".to_owned()));
        let close = ServerMessage::Close("Bye.".to_owned());
        assert_eq!(render(&close), Shown::Closed("Bye.".to_owned()));
    }

    #[test]
    fn a_server_cannot_send_escape_codes() {
        let text = ServerMessage::Text(Message::game("a\u{1b}]52;c;bad\u{7}b\u{202e}"));
        assert_eq!(render(&text), Shown::Line("a]52;c;badb".to_owned()));
        let prompt = ServerMessage::Prompt("\u{1b}[2J$ ".to_owned());
        assert_eq!(render(&prompt), Shown::Prompt("[2J$ ".to_owned()));
    }
}
