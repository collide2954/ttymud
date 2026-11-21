use crate::editor::{Key, control_key};

const ESC: u8 = 0x1b;
const LONGEST: usize = 16;
const MODIFIERS: u16 = 0b1_1100;

#[derive(Debug, Default)]
pub(crate) struct Decoder {
    pending: Vec<u8>,
    after_return: bool,
}

enum Step {
    Key(Key, usize),
    Skip(usize),
    More,
}

impl Decoder {
    pub(crate) fn feed(&mut self, bytes: &[u8]) -> Vec<Key> {
        self.pending.extend_from_slice(bytes);
        let mut keys = Vec::new();
        let mut at = 0;
        while let Some(&first) = self.pending.get(at) {
            if std::mem::take(&mut self.after_return) && matches!(first, b'\n' | 0) {
                at += 1;
                continue;
            }
            match decode(&self.pending[at..]) {
                Step::Key(key, used) => {
                    self.after_return = first == b'\r';
                    keys.push(key);
                    at += used;
                }
                Step::Skip(used) => at += used,
                Step::More => break,
            }
        }
        self.pending.drain(..at);
        keys
    }
}

fn decode(bytes: &[u8]) -> Step {
    let Some((&first, rest)) = bytes.split_first() else {
        return Step::More;
    };
    match first {
        ESC => escape(rest),
        b'\r' | b'\n' => Step::Key(Key::Enter, 1),
        0x7f => Step::Key(Key::Backspace, 1),
        b'\t' => Step::Key(Key::Tab, 1),
        0x01..=0x1a => found(control_key(char::from(b'a' + first - 1)), 1),
        0x00..=0x1f => Step::Skip(1),
        _ => character(bytes),
    }
}

fn escape(rest: &[u8]) -> Step {
    match rest {
        [] => Step::Skip(1),
        [b'[', tail @ ..] => csi(tail),
        [b'O'] => Step::More,
        [b'O', last, ..] => found(ss3(*last), 3),
        [0x7f | 0x08, ..] => Step::Key(Key::DeleteWord, 2),
        [_, ..] => Step::Skip(2),
    }
}

fn csi(tail: &[u8]) -> Step {
    let Some(end) = tail.iter().position(|byte| (0x40..=0x7e).contains(byte)) else {
        return if tail.len() < LONGEST {
            Step::More
        } else {
            Step::Skip(2 + tail.len())
        };
    };
    if let Some(report) = tail[..end].strip_prefix(b"<") {
        return found(wheel(report), end + 3);
    }
    let first = std::str::from_utf8(&tail[..end])
        .ok()
        .and_then(|params| params.split(';').next()?.parse::<u16>().ok());
    let key = match (tail[end], first) {
        (b'A', _) => Some(Key::Up),
        (b'B', _) => Some(Key::Down),
        (b'C', _) => Some(Key::Right),
        (b'D', _) => Some(Key::Left),
        (b'H', _) | (b'~', Some(1 | 7)) => Some(Key::Home),
        (b'F', _) | (b'~', Some(4 | 8)) => Some(Key::End),
        (b'~', Some(3)) => Some(Key::Delete),
        (b'~', Some(5)) => Some(Key::PageUp),
        (b'~', Some(6)) => Some(Key::PageDown),
        _ => None,
    };
    found(key, end + 3)
}

fn wheel(report: &[u8]) -> Option<Key> {
    let button = std::str::from_utf8(report).ok()?.split(';').next()?;
    match button.parse::<u16>().ok()? & !MODIFIERS {
        64 => Some(Key::ScrollUp),
        65 => Some(Key::ScrollDown),
        _ => None,
    }
}

fn ss3(last: u8) -> Option<Key> {
    match last {
        b'A' => Some(Key::Up),
        b'B' => Some(Key::Down),
        b'C' => Some(Key::Right),
        b'D' => Some(Key::Left),
        b'H' => Some(Key::Home),
        b'F' => Some(Key::End),
        _ => None,
    }
}

fn character(bytes: &[u8]) -> Step {
    let width = match bytes.first() {
        Some(0x20..=0x7e) => 1,
        Some(0xc2..=0xdf) => 2,
        Some(0xe0..=0xef) => 3,
        Some(0xf0..=0xf4) => 4,
        _ => return Step::Skip(1),
    };
    let Some(slice) = bytes.get(..width) else {
        return Step::More;
    };
    match std::str::from_utf8(slice)
        .ok()
        .and_then(|text| text.chars().next())
    {
        Some(c) => Step::Key(Key::Char(c), width),
        None => Step::Skip(1),
    }
}

fn found(key: Option<Key>, used: usize) -> Step {
    key.map_or(Step::Skip(used), |key| Step::Key(key, used))
}

#[cfg(test)]
mod tests {
    use super::Decoder;
    use crate::editor::Key;

    fn keys(bytes: &[u8]) -> Vec<Key> {
        Decoder::default().feed(bytes)
    }

    fn chars(text: &str) -> Vec<Key> {
        text.chars().map(Key::Char).collect()
    }

    #[test]
    fn text_gives_its_characters() {
        assert_eq!(keys("go é ✓".as_bytes()), chars("go é ✓"));
    }

    #[test]
    fn enter_is_cr_lf_or_both_once() {
        for bytes in [&b"\r"[..], b"\n", b"\r\n", b"\r\0"] {
            assert_eq!(keys(bytes), [Key::Enter], "{bytes:?}");
        }
        assert_eq!(
            keys(b"a\r\rb"),
            [Key::Char('a'), Key::Enter, Key::Enter, Key::Char('b')]
        );
        let mut decoder = Decoder::default();
        assert_eq!(
            decoder.feed(b"ls\r"),
            [Key::Char('l'), Key::Char('s'), Key::Enter]
        );
        assert_eq!(decoder.feed(b"\nx"), [Key::Char('x')]);
    }

    #[test]
    fn control_bytes_are_the_shell_keys() {
        assert_eq!(
            keys(b"\x7f\x08\t"),
            [Key::Backspace, Key::Backspace, Key::Tab]
        );
        assert_eq!(
            keys(b"\x03\x04\x17\x15\x0b\x01\x05"),
            [
                Key::Cancel,
                Key::EndOfInput,
                Key::DeleteWord,
                Key::DeleteToStart,
                Key::DeleteToEnd,
                Key::Home,
                Key::End,
            ]
        );
        assert_eq!(keys(b"\x00\x1a\x1c"), []);
    }

    #[test]
    fn escape_sequences_are_the_arrows_and_the_page_keys() {
        let cases: [(&[u8], Key); 14] = [
            (b"\x1b[A", Key::Up),
            (b"\x1b[B", Key::Down),
            (b"\x1b[C", Key::Right),
            (b"\x1b[D", Key::Left),
            (b"\x1bOA", Key::Up),
            (b"\x1bOD", Key::Left),
            (b"\x1b[1;5C", Key::Right),
            (b"\x1b[H", Key::Home),
            (b"\x1b[F", Key::End),
            (b"\x1b[1~", Key::Home),
            (b"\x1b[4~", Key::End),
            (b"\x1b[3~", Key::Delete),
            (b"\x1b[5~", Key::PageUp),
            (b"\x1b[6~", Key::PageDown),
        ];
        for (bytes, key) in cases {
            assert_eq!(keys(bytes), [key], "{bytes:?}");
        }
        assert_eq!(keys(b"\x1b\x7f"), [Key::DeleteWord]);
    }

    #[test]
    fn the_mouse_wheel_scrolls_and_other_mouse_reports_type_nothing() {
        assert_eq!(keys(b"\x1b[<64;10;5M"), [Key::ScrollUp]);
        assert_eq!(keys(b"\x1b[<65;10;5M"), [Key::ScrollDown]);
        assert_eq!(keys(b"\x1b[<68;1;1M"), [Key::ScrollUp]);
        assert_eq!(keys(b"\x1b[<0;10;5M\x1b[<0;10;5mx"), chars("x"));
    }

    #[test]
    fn unknown_sequences_and_alt_letters_type_nothing() {
        assert_eq!(keys(b"\x1b[2~\x1bOP\x1bx"), []);
        assert_eq!(keys(b"\x1b[200~hi\x1b[201~"), chars("hi"));
    }

    #[test]
    fn a_split_sequence_waits_for_its_end() {
        let mut decoder = Decoder::default();
        assert_eq!(decoder.feed(b"\x1b["), []);
        assert_eq!(decoder.feed(b"A"), [Key::Up]);
        assert_eq!(decoder.feed(&[0xc3]), []);
        assert_eq!(decoder.feed(&[0xa9]), [Key::Char('é')]);
        assert_eq!(decoder.feed(b"\x1bO"), []);
        assert_eq!(decoder.feed(b"B"), [Key::Down]);
    }

    #[test]
    fn a_lone_escape_types_nothing_and_bad_bytes_are_skipped() {
        let mut decoder = Decoder::default();
        assert_eq!(decoder.feed(b"\x1b"), []);
        assert_eq!(decoder.feed(b"l"), [Key::Char('l')]);
        assert_eq!(keys(&[0xc3, b'A', 0xff, b'B']), chars("AB"));
    }
}
