use ttymud_text::{RunStatus, Status, clean};
use ttymud_wire::ServerMessage;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Shown {
    Line(String),
    Prompt(String),
    Closed(String),
    Candidates { word: String, names: Vec<String> },
    Status(Status),
}

#[must_use]
pub fn render(message: &ServerMessage) -> Shown {
    match message {
        ServerMessage::Welcome { city, name } => {
            let (city, name) = (clean(city), clean(name));
            Shown::Line(format!("Connected to {city} as {name}."))
        }
        ServerMessage::Text(text) => Shown::Line(clean(&text.plain())),
        ServerMessage::Prompt(prompt) | ServerMessage::Refresh(prompt) => {
            Shown::Prompt(clean(prompt))
        }
        ServerMessage::Close(reason) => Shown::Closed(clean(reason)),
        ServerMessage::Candidates { word, names } => Shown::Candidates {
            word: clean(word),
            names: names.iter().map(|name| clean(name)).collect(),
        },
        ServerMessage::Status(status) => Shown::Status(Status {
            host: clean(&status.host),
            run: status.run.as_ref().map(|run| RunStatus {
                network: clean(&run.network),
                access: clean(&run.access),
                trace: run.trace,
            }),
            ..status.clone()
        }),
    }
}

#[cfg(test)]
mod tests {
    use ttymud_text::{Message, RunStatus, Status, Style};
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
    fn a_refresh_shows_as_a_prompt() {
        let refresh = ServerMessage::Refresh("[trace 4%] $ \u{1b}".to_owned());
        assert_eq!(render(&refresh), Shown::Prompt("[trace 4%] $ ".to_owned()));
    }

    #[test]
    fn candidates_are_cleaned() {
        let candidates = ServerMessage::Candidates {
            word: "m\u{1b}a".to_owned(),
            names: vec!["mar\u{202e}ket".to_owned()],
        };
        assert_eq!(
            render(&candidates),
            Shown::Candidates {
                word: "ma".to_owned(),
                names: vec!["market".to_owned()],
            }
        );
    }

    #[test]
    fn a_status_is_cleaned() {
        let run = |network: &str| RunStatus {
            network: network.to_owned(),
            access: "user".to_owned(),
            trace: 31,
        };
        let status = Status {
            host: "arc\u{1b}adia".to_owned(),
            run: Some(run("Arcadia\u{202e} Health")),
            ..Status::default()
        };
        let clean = Status {
            host: "arcadia".to_owned(),
            run: Some(run("Arcadia Health")),
            ..Status::default()
        };
        assert_eq!(render(&ServerMessage::Status(status)), Shown::Status(clean));
    }

    #[test]
    fn a_server_cannot_send_escape_codes() {
        let text = ServerMessage::Text(Message::game("a\u{1b}]52;c;bad\u{7}b\u{202e}"));
        assert_eq!(render(&text), Shown::Line("a]52;c;badb".to_owned()));
        let prompt = ServerMessage::Prompt("\u{1b}[2J$ ".to_owned());
        assert_eq!(render(&prompt), Shown::Prompt("[2J$ ".to_owned()));
    }
}
