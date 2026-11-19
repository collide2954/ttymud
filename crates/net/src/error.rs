use std::{fmt, io};

#[derive(Debug)]
pub enum Error {
    Io(io::Error),
    Noise(snow::Error),
    Closed,
    TooLarge,
    BadKey,
    NoPeerKey,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "network error: {error}"),
            Self::Noise(error) => write!(f, "secure channel error: {error}"),
            Self::Closed => f.write_str("the connection is closed"),
            Self::TooLarge => f.write_str("the message is too large"),
            Self::BadKey => f.write_str("the key is not valid"),
            Self::NoPeerKey => f.write_str("the other side sent no static key"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            _ => None,
        }
    }
}

impl From<io::Error> for Error {
    fn from(error: io::Error) -> Self {
        if error.kind() == io::ErrorKind::UnexpectedEof {
            Self::Closed
        } else {
            Self::Io(error)
        }
    }
}

impl From<Error> for io::Error {
    fn from(error: Error) -> Self {
        match error {
            Error::Io(error) => error,
            other => Self::other(other),
        }
    }
}

impl From<snow::Error> for Error {
    fn from(error: snow::Error) -> Self {
        Self::Noise(error)
    }
}
