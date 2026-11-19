use ttymud_text::{Kind, Message, RunStatus, Span, Status, Style};

use crate::codec::{DecodeError, Reader, Writer};

const COMMAND: u8 = 1;
const COMPLETE: u8 = 2;

const WELCOME: u8 = 1;
const TEXT: u8 = 2;
const PROMPT: u8 = 3;
const CLOSE: u8 = 4;
const CANDIDATES: u8 = 5;
const REFRESH: u8 = 6;
const STATUS: u8 = 7;

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
    Refresh(String),
    Status(Status),
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
            Self::Refresh(prompt) => {
                writer.byte(REFRESH);
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
            Self::Status(status) => {
                writer.byte(STATUS);
                write_status(&mut writer, status);
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
            REFRESH => Self::Refresh(reader.text()?),
            CLOSE => Self::Close(reader.text()?),
            CANDIDATES => Self::Candidates {
                word: reader.text()?,
                names: read_names(&mut reader)?,
            },
            STATUS => Self::Status(read_status(&mut reader)?),
            other => return Err(DecodeError::UnknownType(other)),
        };
        reader.finish()?;
        Ok(message)
    }
}

fn write_status(writer: &mut Writer, status: &Status) {
    writer.text(&status.host);
    match &status.run {
        Some(run) => {
            writer.byte(1);
            writer.text(&run.network);
            writer.text(&run.access);
            writer.number(u64::from(run.trace));
            writer.text(&run.alert);
        }
        None => writer.byte(0),
    }
    let numbers = [
        status.integrity,
        status.heat,
        status.heat_max,
        status.ram_used,
        status.ram_total,
        status.cycles,
        status.cycles_max,
        status.credits,
    ];
    for number in numbers {
        writer.number(u64::from(number));
    }
    writer.text(&status.headline);
}

fn read_status(reader: &mut Reader<'_>) -> Result<Status, DecodeError> {
    let host = reader.text()?;
    let run = match reader.byte()? {
        0 => None,
        1 => Some(RunStatus {
            network: reader.text()?,
            access: reader.text()?,
            trace: small(reader)?,
            alert: reader.text()?,
        }),
        _ => return Err(DecodeError::BadValue),
    };
    Ok(Status {
        host,
        run,
        integrity: small(reader)?,
        heat: small(reader)?,
        heat_max: small(reader)?,
        ram_used: small(reader)?,
        ram_total: small(reader)?,
        cycles: small(reader)?,
        cycles_max: small(reader)?,
        credits: small(reader)?,
        headline: reader.text()?,
    })
}

fn small(reader: &mut Reader<'_>) -> Result<u32, DecodeError> {
    u32::try_from(reader.number()?).map_err(|_| DecodeError::BadValue)
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
        Kind::Telegraph => 3,
    }
}

fn kind_from(tag: u8) -> Result<Kind, DecodeError> {
    match tag {
        0 => Ok(Kind::Game),
        1 => Ok(Kind::Error),
        2 => Ok(Kind::Chat),
        3 => Ok(Kind::Telegraph),
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
        Style::Ice => 6,
        Style::Danger => 7,
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
        6 => Ok(Style::Ice),
        7 => Ok(Style::Danger),
        _ => Err(DecodeError::BadValue),
    }
}

#[cfg(test)]
mod tests {
    use ttymud_text::{Kind, Message, RunStatus, Status, Style};

    use super::{ClientMessage, ServerMessage};
    use crate::codec::DecodeError;

    const KINDS: [Kind; 4] = [Kind::Game, Kind::Error, Kind::Chat, Kind::Telegraph];
    const STYLES: [Style; 8] = [
        Style::Plain,
        Style::Title,
        Style::Host,
        Style::File,
        Style::Command,
        Style::Player,
        Style::Ice,
        Style::Danger,
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
        server_round_trip(&ServerMessage::Refresh(
            "[trace 4% integ 100] $ ".to_owned(),
        ));
        server_round_trip(&ServerMessage::Close("The server stops.".to_owned()));
    }

    fn status(run: Option<RunStatus>) -> Status {
        Status {
            host: "arcadia-db".to_owned(),
            run,
            integrity: 82,
            heat: 2,
            heat_max: 5,
            ram_used: 6,
            ram_total: 8,
            cycles: 3,
            cycles_max: 4,
            credits: 1_250,
            headline: "Arcadia Bank raises its alert to red.".to_owned(),
        }
    }

    #[test]
    fn a_status_round_trips_in_and_out_of_a_run() {
        server_round_trip(&ServerMessage::Status(status(None)));
        let run = RunStatus {
            network: "Arcadia Health".to_owned(),
            access: "user".to_owned(),
            trace: 31,
            alert: "orange".to_owned(),
        };
        server_round_trip(&ServerMessage::Status(status(Some(run))));
        server_round_trip(&ServerMessage::Status(Status::default()));
    }

    #[test]
    fn a_status_with_a_bad_run_flag_or_a_huge_value_fails() {
        let bytes = ServerMessage::Status(status(None)).encode();
        let flag = 2 + "arcadia-db".len();
        let mut bad = bytes.clone();
        bad[flag] = 2;
        assert_eq!(ServerMessage::decode(&bad), Err(DecodeError::BadValue));
        let mut huge = bytes[..=flag].to_vec();
        huge.extend([0xff, 0xff, 0xff, 0xff, 0x7f]);
        huge.extend(&bytes[flag + 2..]);
        assert_eq!(ServerMessage::decode(&huge), Err(DecodeError::BadValue));
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
