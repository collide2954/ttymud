use ttymud_text::{Kind, Message, Span, Style};

use crate::codec::{DecodeError, Reader, Writer};

const COMMAND: u8 = 1;
const COMPLETE: u8 = 2;

const WELCOME: u8 = 1;
const TEXT: u8 = 2;
const PROMPT: u8 = 3;
const CLOSE: u8 = 4;
const CANDIDATES: u8 = 5;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClientMessage {
    Command(String),
    Complete(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServerMessage {
    Welcome { city: String, name: String },
    Text(Message),
    Prompt(String),
    Close(String),
    Candidates { word: String, names: Vec<String> },
}

impl ClientMessage {
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = Writer::default();
        let (tag, text) = match self {
            Self::Command(line) => (COMMAND, line),
            Self::Complete(text) => (COMPLETE, text),
        };
        writer.byte(tag);
        writer.text(text);
        writer.finish()
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, DecodeError> {
        let mut reader = Reader::new(bytes);
        let message = match reader.tag()? {
            COMMAND => Self::Command(reader.text()?),
            COMPLETE => Self::Complete(reader.text()?),
            other => return Err(DecodeError::UnknownType(other)),
        };
        reader.finish()?;
        Ok(message)
    }
}

impl ServerMessage {
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = Writer::default();
        match self {
            Self::Welcome { city, name } => {
                writer.byte(WELCOME);
                writer.text(city);
                writer.text(name);
            }
            Self::Text(message) => {
                writer.byte(TEXT);
                write_message(&mut writer, message);
            }
            Self::Prompt(prompt) => {
                writer.byte(PROMPT);
                writer.text(prompt);
            }
            Self::Close(reason) => {
                writer.byte(CLOSE);
                writer.text(reason);
            }
            Self::Candidates { word, names } => {
                writer.byte(CANDIDATES);
                writer.text(word);
                writer.number(u64::try_from(names.len()).unwrap_or(u64::MAX));
                for name in names {
                    writer.text(name);
                }
            }
        }
        writer.finish()
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, DecodeError> {
        let mut reader = Reader::new(bytes);
        let message = match reader.tag()? {
            WELCOME => Self::Welcome {
                city: reader.text()?,
                name: reader.text()?,
            },
            TEXT => Self::Text(read_message(&mut reader)?),
            PROMPT => Self::Prompt(reader.text()?),
            CLOSE => Self::Close(reader.text()?),
            CANDIDATES => Self::Candidates {
                word: reader.text()?,
                names: read_names(&mut reader)?,
            },
            other => return Err(DecodeError::UnknownType(other)),
        };
        reader.finish()?;
        Ok(message)
    }
}

fn write_message(writer: &mut Writer, message: &Message) {
    writer.byte(kind_tag(message.kind));
    writer.number(u64::try_from(message.spans.len()).unwrap_or(u64::MAX));
    for span in &message.spans {
        writer.byte(style_tag(span.style));
        writer.text(&span.text);
    }
}

fn read_message(reader: &mut Reader<'_>) -> Result<Message, DecodeError> {
    let kind = kind_from(reader.byte()?)?;
    let count = reader.count()?;
    let mut spans = Vec::with_capacity(count);
    for _ in 0..count {
        let style = style_from(reader.byte()?)?;
        spans.push(Span {
            style,
            text: reader.text()?,
        });
    }
    Ok(Message { kind, spans })
}

fn read_names(reader: &mut Reader<'_>) -> Result<Vec<String>, DecodeError> {
    let count = reader.count()?;
    (0..count).map(|_| reader.text()).collect()
}

fn kind_tag(kind: Kind) -> u8 {
    match kind {
        Kind::Game => 0,
        Kind::Error => 1,
        Kind::Chat => 2,
    }
}

fn kind_from(tag: u8) -> Result<Kind, DecodeError> {
    match tag {
        0 => Ok(Kind::Game),
        1 => Ok(Kind::Error),
        2 => Ok(Kind::Chat),
        _ => Err(DecodeError::BadValue),
    }
}

fn style_tag(style: Style) -> u8 {
    match style {
        Style::Plain => 0,
        Style::Title => 1,
        Style::Host => 2,
        Style::File => 3,
        Style::Command => 4,
        Style::Player => 5,
    }
}

fn style_from(tag: u8) -> Result<Style, DecodeError> {
    match tag {
        0 => Ok(Style::Plain),
        1 => Ok(Style::Title),
        2 => Ok(Style::Host),
        3 => Ok(Style::File),
        4 => Ok(Style::Command),
        5 => Ok(Style::Player),
        _ => Err(DecodeError::BadValue),
    }
}

#[cfg(test)]
mod tests {
    use ttymud_text::{Kind, Message, Style};

    use super::{ClientMessage, ServerMessage};
    use crate::codec::DecodeError;

    const KINDS: [Kind; 3] = [Kind::Game, Kind::Error, Kind::Chat];
    const STYLES: [Style; 6] = [
        Style::Plain,
        Style::Title,
        Style::Host,
        Style::File,
        Style::Command,
        Style::Player,
    ];

    fn server_round_trip(message: &ServerMessage) {
        assert_eq!(
            ServerMessage::decode(&message.encode()).as_ref(),
            Ok(message)
        );
    }

    #[test]
    fn commands_round_trip() {
        for line in ["ls | grep txt", "", "say Привет 🙂"] {
            let message = ClientMessage::Command(line.to_owned());
            assert_eq!(ClientMessage::decode(&message.encode()), Ok(message));
        }
    }

    #[test]
    fn completion_requests_round_trip() {
        let message = ClientMessage::Complete("go ma".to_owned());
        assert_eq!(ClientMessage::decode(&message.encode()), Ok(message));
    }

    #[test]
    fn candidates_round_trip() {
        server_round_trip(&ServerMessage::Candidates {
            word: "ma".to_owned(),
            names: vec!["market".to_owned(), "maze".to_owned()],
        });
        server_round_trip(&ServerMessage::Candidates {
            word: String::new(),
            names: Vec::new(),
        });
    }

    #[test]
    fn huge_candidate_counts_fail_without_a_large_allocation() {
        let mut bytes = ServerMessage::Candidates {
            word: String::new(),
            names: Vec::new(),
        }
        .encode();
        bytes.pop();
        bytes.extend([0xff, 0xff, 0xff, 0xff, 0x0f]);
        assert_eq!(ServerMessage::decode(&bytes), Err(DecodeError::Truncated));
    }

    #[test]
    fn server_messages_round_trip() {
        server_round_trip(&ServerMessage::Welcome {
            city: "Port Ostra".to_owned(),
            name: "anon-3f9a2c71".to_owned(),
        });
        server_round_trip(&ServerMessage::Text(
            Message::game("Routes: ").span(Style::Host, "relay"),
        ));
        server_round_trip(&ServerMessage::Text(Message::new(Kind::Chat)));
        server_round_trip(&ServerMessage::Prompt("ghost@relay $ ".to_owned()));
        server_round_trip(&ServerMessage::Close("The server stops.".to_owned()));
    }

    #[test]
    fn every_kind_and_style_round_trips() {
        for kind in KINDS {
            for style in STYLES {
                server_round_trip(&ServerMessage::Text(Message::new(kind).span(style, "x")));
            }
        }
    }

    #[test]
    fn empty_input_fails() {
        assert_eq!(ClientMessage::decode(&[]), Err(DecodeError::Empty));
        assert_eq!(ServerMessage::decode(&[]), Err(DecodeError::Empty));
    }

    #[test]
    fn unknown_types_fail() {
        assert_eq!(
            ClientMessage::decode(&[99]),
            Err(DecodeError::UnknownType(99))
        );
        assert_eq!(
            ServerMessage::decode(&[99]),
            Err(DecodeError::UnknownType(99))
        );
    }

    #[test]
    fn cut_messages_fail() {
        let bytes = ClientMessage::Command("hello".to_owned()).encode();
        assert_eq!(
            ClientMessage::decode(&bytes[..bytes.len() - 1]),
            Err(DecodeError::Truncated)
        );
    }

    #[test]
    fn bad_utf8_fails() {
        let mut bytes = ClientMessage::Command("ab".to_owned()).encode();
        let last = bytes.len() - 1;
        bytes[last] = 0xff;
        assert_eq!(ClientMessage::decode(&bytes), Err(DecodeError::BadText));
    }

    #[test]
    fn extra_bytes_fail() {
        let mut bytes = ServerMessage::Prompt("$ ".to_owned()).encode();
        bytes.push(0);
        assert_eq!(ServerMessage::decode(&bytes), Err(DecodeError::Trailing));
    }

    #[test]
    fn unknown_kinds_and_styles_fail() {
        let text = ServerMessage::Text(Message::game("x")).encode();
        let mut kind = text.clone();
        kind[1] = 99;
        assert_eq!(ServerMessage::decode(&kind), Err(DecodeError::BadValue));
        let mut style = text;
        style[3] = 99;
        assert_eq!(ServerMessage::decode(&style), Err(DecodeError::BadValue));
    }

    #[test]
    fn huge_counts_fail_without_a_large_allocation() {
        let text = ServerMessage::Text(Message::new(Kind::Game)).encode();
        let mut bytes = text[..2].to_vec();
        bytes.extend([0xff, 0xff, 0xff, 0xff, 0x0f]);
        assert_eq!(ServerMessage::decode(&bytes), Err(DecodeError::Truncated));
    }

    #[test]
    fn random_bytes_never_panic() {
        let mut state: u64 = 0x9e37_79b9_7f4a_7c15;
        for _ in 0..10_000 {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            let bytes = state.to_le_bytes();
            let length = usize::from(bytes[0] % 9);
            let _ = ClientMessage::decode(&bytes[..length]);
            let _ = ServerMessage::decode(&bytes[..length]);
        }
    }
}
