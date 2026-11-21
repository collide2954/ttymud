use ttymud_text::{Message, Style};
use unicode_width::UnicodeWidthChar;

use crate::canvas::Ink;

const INDENT: usize = 2;
const INDENT_WIDTH: usize = 20;

pub(crate) type Row = Vec<(char, Ink)>;

pub(crate) fn wrap(message: &Message, width: usize) -> Vec<Row> {
    let width = width.max(1);
    let indent = if width >= INDENT_WIDTH { INDENT } else { 0 };
    let pad = (' ', Ink::Text(Style::Plain, message.kind));
    let chars: Row = message
        .spans
        .iter()
        .flat_map(|span| {
            let ink = Ink::Text(span.style, message.kind);
            span.text.chars().map(move |c| (c, ink))
        })
        .collect();
    let mut rows: Vec<Row> = vec![Vec::new()];
    let (mut used, mut start) = (0, 0);
    let next = |rows: &mut Vec<Row>| {
        rows.push(vec![pad; indent]);
        (indent, indent)
    };
    for token in chars.chunk_by(|a, b| (a.0 == ' ') == (b.0 == ' ')) {
        let size: usize = token.iter().map(|&(c, _)| columns(c)).sum();
        if token[0].0 == ' ' {
            if used == start && rows.len() > 1 {
                continue;
            }
            if used + size > width {
                (used, start) = next(&mut rows);
                continue;
            }
        } else if used + size > width {
            if used > start {
                (used, start) = next(&mut rows);
            }
            if used + size > width {
                for &(c, ink) in token {
                    if used + columns(c) > width && used > start {
                        (used, start) = next(&mut rows);
                    }
                    if let Some(row) = rows.last_mut() {
                        row.push((c, ink));
                    }
                    used += columns(c);
                }
                continue;
            }
        }
        if let Some(row) = rows.last_mut() {
            row.extend_from_slice(token);
        }
        used += size;
    }
    for row in &mut rows {
        while row.last().is_some_and(|&(c, _)| c == ' ') {
            row.pop();
        }
    }
    rows
}

pub(crate) fn columns(c: char) -> usize {
    c.width().unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use ttymud_text::{Kind, Message, Style};

    use super::wrap;
    use crate::canvas::Ink;

    fn texts(message: &Message, width: usize) -> Vec<String> {
        let rows = wrap(message, width);
        rows.iter()
            .map(|row| row.iter().map(|&(c, _)| c).collect())
            .collect()
    }

    #[test]
    fn words_move_to_the_next_row() {
        let message = Message::game("The quick brown fox jumps");
        assert_eq!(texts(&message, 10), ["The quick", "brown fox", "jumps"]);
        assert_eq!(texts(&message, 80), ["The quick brown fox jumps"]);
    }

    #[test]
    fn a_long_word_breaks_at_the_width() {
        let message = Message::game("see abcdefghijkl");
        assert_eq!(texts(&message, 5), ["see", "abcde", "fghij", "kl"]);
    }

    #[test]
    fn the_first_row_keeps_its_indent_but_later_rows_do_not() {
        let message = Message::game("  crack: overflow lands");
        assert_eq!(texts(&message, 12), ["  crack:", "overflow", "lands"]);
    }

    #[test]
    fn from_a_width_of_20_later_rows_hang_two_columns_in() {
        let message = Message::game("The ledger server hums behind two firewalls tonight.");
        assert_eq!(
            texts(&message, 24),
            [
                "The ledger server hums",
                "  behind two firewalls",
                "  tonight."
            ]
        );
        let message = Message::game("see abcdefghijklmnopqrstuvwxyz0123456789");
        assert_eq!(
            texts(&message, 20),
            ["see", "  abcdefghijklmnopqr", "  stuvwxyz0123456789"]
        );
    }

    #[test]
    fn wide_characters_count_two_columns() {
        let message = Message::game("日本語テキスト");
        assert_eq!(texts(&message, 5), ["日本", "語テ", "キス", "ト"]);
    }

    #[test]
    fn each_character_keeps_the_ink_of_its_span() {
        let message = Message::game("Routes: ").span(Style::Host, "relay");
        let rows = wrap(&message, 80);
        assert_eq!(rows[0][0], ('R', Ink::Text(Style::Plain, Kind::Game)));
        assert_eq!(rows[0][8], ('r', Ink::Text(Style::Host, Kind::Game)));
    }

    #[test]
    fn an_empty_message_is_one_empty_row() {
        assert_eq!(texts(&Message::game(""), 10), [""]);
    }
}
