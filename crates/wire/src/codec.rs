use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecodeError {
    Empty,
    UnknownType(u8),
    Truncated,
    BadNumber,
    BadText,
    BadValue,
    Trailing,
}

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => f.write_str("the message is empty"),
            Self::UnknownType(tag) => write!(f, "unknown message type {tag}"),
            Self::Truncated => f.write_str("the message ends too early"),
            Self::BadNumber => f.write_str("a number is too long"),
            Self::BadText => f.write_str("a text is not valid UTF-8"),
            Self::BadValue => f.write_str("a value is out of range"),
            Self::Trailing => f.write_str("the message has extra bytes"),
        }
    }
}

impl std::error::Error for DecodeError {}

#[derive(Debug, Default)]
pub(crate) struct Writer {
    bytes: Vec<u8>,
}

impl Writer {
    pub(crate) fn byte(&mut self, value: u8) {
        self.bytes.push(value);
    }

    pub(crate) fn number(&mut self, mut value: u64) {
        loop {
            let low = u8::try_from(value & 0x7f).unwrap_or_default();
            value >>= 7;
            if value == 0 {
                self.bytes.push(low);
                return;
            }
            self.bytes.push(low | 0x80);
        }
    }

    pub(crate) fn text(&mut self, value: &str) {
        self.number(u64::try_from(value.len()).unwrap_or(u64::MAX));
        self.bytes.extend_from_slice(value.as_bytes());
    }

    pub(crate) fn finish(self) -> Vec<u8> {
        self.bytes
    }
}

#[derive(Debug)]
pub(crate) struct Reader<'a> {
    bytes: &'a [u8],
}

impl<'a> Reader<'a> {
    pub(crate) fn new(bytes: &'a [u8]) -> Self {
        Self { bytes }
    }

    pub(crate) fn tag(&mut self) -> Result<u8, DecodeError> {
        self.byte().map_err(|_| DecodeError::Empty)
    }

    pub(crate) fn byte(&mut self) -> Result<u8, DecodeError> {
        let (&first, rest) = self.bytes.split_first().ok_or(DecodeError::Truncated)?;
        self.bytes = rest;
        Ok(first)
    }

    pub(crate) fn number(&mut self) -> Result<u64, DecodeError> {
        let mut value = 0;
        for shift in (0..64).step_by(7) {
            let byte = self.byte()?;
            value |= u64::from(byte & 0x7f) << shift;
            if byte & 0x80 == 0 {
                return Ok(value);
            }
        }
        Err(DecodeError::BadNumber)
    }

    pub(crate) fn count(&mut self) -> Result<usize, DecodeError> {
        let count = usize::try_from(self.number()?).map_err(|_| DecodeError::Truncated)?;
        if count > self.bytes.len() {
            return Err(DecodeError::Truncated);
        }
        Ok(count)
    }

    pub(crate) fn text(&mut self) -> Result<String, DecodeError> {
        let length = self.count()?;
        let (text, rest) = self.bytes.split_at(length);
        self.bytes = rest;
        String::from_utf8(text.to_vec()).map_err(|_| DecodeError::BadText)
    }

    pub(crate) fn finish(self) -> Result<(), DecodeError> {
        if self.bytes.is_empty() {
            Ok(())
        } else {
            Err(DecodeError::Trailing)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{DecodeError, Reader, Writer};

    const NUMBERS: [(u64, &[u8]); 5] = [
        (0, &[0x00]),
        (127, &[0x7f]),
        (128, &[0x80, 0x01]),
        (300, &[0xac, 0x02]),
        (
            u64::MAX,
            &[0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x01],
        ),
    ];

    #[test]
    fn numbers_use_leb128() {
        for (value, bytes) in NUMBERS {
            let mut writer = Writer::default();
            writer.number(value);
            assert_eq!(writer.finish(), bytes, "{value}");
            assert_eq!(Reader::new(bytes).number(), Ok(value), "{value}");
        }
    }

    #[test]
    fn overlong_numbers_fail() {
        assert_eq!(
            Reader::new(&[0xff; 11]).number(),
            Err(DecodeError::BadNumber)
        );
    }

    #[test]
    fn text_has_a_length_prefix() {
        let mut writer = Writer::default();
        writer.text("hi");
        assert_eq!(writer.finish(), [2, b'h', b'i']);
    }

    #[test]
    fn a_count_cannot_exceed_the_bytes_left() {
        assert_eq!(Reader::new(&[5, 1, 2]).count(), Err(DecodeError::Truncated));
        assert_eq!(Reader::new(&[2, 1, 2]).count(), Ok(2));
    }

    #[test]
    fn a_missing_tag_means_an_empty_message() {
        assert_eq!(Reader::new(&[]).tag(), Err(DecodeError::Empty));
    }

    #[test]
    fn leftover_bytes_fail() {
        let mut reader = Reader::new(&[1, 2]);
        assert_eq!(reader.byte(), Ok(1));
        assert_eq!(reader.finish(), Err(DecodeError::Trailing));
    }
}
