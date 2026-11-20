use crate::codec::{DecodeError, Reader, Writer};

const HELLO: u8 = 32;
const CHAT: u8 = 33;
const NEWS: u8 = 34;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkMessage {
    Hello { city: String },
    Chat { handle: String, text: String },
    News { when: u64, text: String },
}

impl LinkMessage {
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = Writer::default();
        match self {
            Self::Hello { city } => {
                writer.byte(HELLO);
                writer.text(city);
            }
            Self::Chat { handle, text } => {
                writer.byte(CHAT);
                writer.text(handle);
                writer.text(text);
            }
            Self::News { when, text } => {
                writer.byte(NEWS);
                writer.number(*when);
                writer.text(text);
            }
        }
        writer.finish()
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, DecodeError> {
        let mut reader = Reader::new(bytes);
        let message = match reader.tag()? {
            HELLO => Self::Hello {
                city: reader.text()?,
            },
            CHAT => Self::Chat {
                handle: reader.text()?,
                text: reader.text()?,
            },
            NEWS => Self::News {
                when: reader.number()?,
                text: reader.text()?,
            },
            other => return Err(DecodeError::UnknownType(other)),
        };
        reader.finish()?;
        Ok(message)
    }
}

#[cfg(test)]
mod tests {
    use super::LinkMessage;
    use crate::{DecodeError, ServerMessage};

    fn chat() -> LinkMessage {
        LinkMessage::Chat {
            handle: "nyx".to_owned(),
            text: "hi from the dock 🙂".to_owned(),
        }
    }

    #[test]
    fn link_messages_round_trip() {
        let messages = [
            LinkMessage::Hello {
                city: "Port Ostra".to_owned(),
            },
            chat(),
            LinkMessage::News {
                when: 1_700_000_000,
                text: "Kestrel Health loses patients.db in a breach.".to_owned(),
            },
        ];
        for message in messages {
            assert_eq!(LinkMessage::decode(&message.encode()), Ok(message));
        }
    }

    #[test]
    fn link_messages_and_server_messages_never_share_a_type() {
        let hello = LinkMessage::Hello {
            city: "Port Ostra".to_owned(),
        };
        assert_eq!(
            ServerMessage::decode(&hello.encode()),
            Err(DecodeError::UnknownType(32))
        );
        let welcome = ServerMessage::Welcome {
            city: "Port Ostra".to_owned(),
            name: "anon-3f9a2c71".to_owned(),
        };
        assert_eq!(
            LinkMessage::decode(&welcome.encode()),
            Err(DecodeError::UnknownType(1))
        );
    }

    #[test]
    fn bad_link_messages_fail() {
        let bytes = chat().encode();
        assert_eq!(LinkMessage::decode(&[]), Err(DecodeError::Empty));
        assert_eq!(
            LinkMessage::decode(&bytes[..bytes.len() - 1]),
            Err(DecodeError::Truncated)
        );
        let mut long = bytes;
        long.push(0);
        assert_eq!(LinkMessage::decode(&long), Err(DecodeError::Trailing));
    }

    #[test]
    fn random_bytes_never_panic() {
        let mut state: u64 = 0x2545_f491_4f6c_dd1d;
        for _ in 0..10_000 {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            let mut bytes = state.to_le_bytes();
            bytes[0] = 32 + bytes[0] % 4;
            let length = usize::from(bytes[1] % 9);
            let _ = LinkMessage::decode(&bytes[..length]);
        }
    }
}
