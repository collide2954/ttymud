use std::fmt::Write as _;

use ttymud_text::{Kind, Style};
use unicode_width::UnicodeWidthChar;

use crate::paint;
use crate::settings::Depth;

const RESET: &str = "\x1b[0m";
const GREEN: (usize, usize, usize) = (80, 200, 120);
const YELLOW: (usize, usize, usize) = (230, 200, 60);
const RED: (usize, usize, usize) = (230, 80, 80);
const STEPS: [usize; 6] = [71, 107, 143, 179, 173, 167];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum Ink {
    #[default]
    Plain,
    Text(Style, Kind),
    Dim,
    Strong,
    Level(u32),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Cell {
    text: String,
    ink: Ink,
}

impl Cell {
    fn blank() -> Self {
        Self {
            text: " ".to_owned(),
            ink: Ink::Plain,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Canvas {
    width: usize,
    height: usize,
    cells: Vec<Cell>,
}

impl Canvas {
    pub(crate) fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            cells: vec![Cell::blank(); width * height],
        }
    }

    pub(crate) fn put(&mut self, x: usize, y: usize, text: &str, ink: Ink, room: usize) -> usize {
        if y >= self.height || x >= self.width {
            return 0;
        }
        let end = x.saturating_add(room).min(self.width);
        let mut column = x;
        let mut last: Option<usize> = None;
        for c in text.chars() {
            let size = c.width().unwrap_or(0);
            if size == 0 {
                if let Some(index) = last {
                    self.cells[index].text.push(c);
                }
                continue;
            }
            if column + size > end {
                break;
            }
            let index = y * self.width + column;
            for part in index..index + size {
                self.clear_wide(part);
            }
            self.cells[index] = Cell {
                text: c.to_string(),
                ink,
            };
            if size == 2 {
                self.cells[index + 1] = Cell {
                    text: String::new(),
                    ink,
                };
            }
            last = Some(index);
            column += size;
        }
        column - x
    }

    fn clear_wide(&mut self, index: usize) {
        if self.cells[index].text.is_empty() {
            self.cells[index - 1] = Cell::blank();
        } else if (index % self.width) + 1 < self.width && self.cells[index + 1].text.is_empty() {
            self.cells[index + 1] = Cell::blank();
        }
    }

    fn cells_of(&self, y: usize) -> &[Cell] {
        &self.cells[y * self.width..(y + 1) * self.width]
    }

    #[cfg(test)]
    pub(crate) fn rows(&self) -> Vec<String> {
        (0..self.height).map(|y| self.row(y)).collect()
    }

    #[cfg(test)]
    pub(crate) fn row(&self, y: usize) -> String {
        self.cells_of(y)
            .iter()
            .map(|cell| cell.text.as_str())
            .collect()
    }

    pub(crate) fn changes(&self, old: Option<&Self>, depth: Depth) -> String {
        let same = old.filter(|old| old.width == self.width && old.height == self.height);
        let colored = depth != Depth::None;
        let mut out = String::new();
        for y in 0..self.height {
            let row = self.cells_of(y);
            if same.is_some_and(|old| old.cells_of(y) == row) {
                continue;
            }
            let _ = write!(out, "\x1b[{};1H", y + 1);
            if colored {
                out.push_str(RESET);
            }
            let mut active = Ink::Plain;
            for cell in row.iter().filter(|cell| !cell.text.is_empty()) {
                if colored && cell.ink != active {
                    if sgr(active, depth).is_some() {
                        out.push_str(RESET);
                    }
                    if let Some(code) = sgr(cell.ink, depth) {
                        let _ = write!(out, "\x1b[{code}m");
                    }
                    active = cell.ink;
                }
                out.push_str(&cell.text);
            }
            if colored {
                out.push_str(RESET);
            }
        }
        out
    }
}

pub(crate) fn sgr(ink: Ink, depth: Depth) -> Option<String> {
    if depth == Depth::None {
        return None;
    }
    match ink {
        Ink::Plain => None,
        Ink::Text(style, kind) => paint::code(style, kind).map(str::to_owned),
        Ink::Strong => Some("1".to_owned()),
        Ink::Dim => Some(
            match depth {
                Depth::Indexed => "38;5;244",
                Depth::True => "38;2;128;128;128",
                Depth::Basic | Depth::None => "90",
            }
            .to_owned(),
        ),
        Ink::Level(level) => Some(level_code(level, depth)),
    }
}

fn level_code(level: u32, depth: Depth) -> String {
    let level = usize::try_from(level).map_or(100, |level| level.min(100));
    match depth {
        Depth::True => {
            let (from, to, part) = if level <= 50 {
                (GREEN, YELLOW, level)
            } else {
                (RED, YELLOW, 100 - level)
            };
            let mix = |a: usize, b: usize| (a * (50 - part) + b * part) / 50;
            let (red, green, blue) = (mix(from.0, to.0), mix(from.1, to.1), mix(from.2, to.2));
            format!("38;2;{red};{green};{blue}")
        }
        Depth::Indexed => format!("38;5;{}", STEPS[(level * STEPS.len() / 101).min(5)]),
        Depth::Basic | Depth::None => match level {
            0..50 => "32",
            50..80 => "33",
            _ => "91",
        }
        .to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use ttymud_text::{Kind, Style};

    use super::{Canvas, Ink, sgr};
    use crate::settings::Depth;

    #[test]
    fn text_goes_into_cells_and_stops_at_the_room() {
        let mut canvas = Canvas::new(10, 2);
        assert_eq!(canvas.put(1, 0, "hello world", Ink::Plain, 5), 5);
        assert_eq!(canvas.row(0), " hello    ");
        assert_eq!(canvas.row(1), "          ");
        assert_eq!(canvas.put(8, 1, "abc", Ink::Plain, 9), 2);
        assert_eq!(canvas.put(0, 5, "x", Ink::Plain, 9), 0);
    }

    #[test]
    fn wide_characters_take_two_cells_and_marks_join_their_letter() {
        let mut canvas = Canvas::new(6, 1);
        assert_eq!(canvas.put(0, 0, "日本語", Ink::Plain, 5), 4);
        assert_eq!(canvas.row(0), "日本  ");
        let mut marks = Canvas::new(4, 1);
        assert_eq!(marks.put(0, 0, "e\u{301}x", Ink::Plain, 4), 2);
        assert_eq!(marks.row(0), "e\u{301}x  ");
    }

    #[test]
    fn a_letter_over_half_of_a_wide_character_clears_the_other_half() {
        let mut canvas = Canvas::new(4, 1);
        canvas.put(0, 0, "日", Ink::Plain, 4);
        canvas.put(1, 0, "x", Ink::Plain, 3);
        assert_eq!(canvas.row(0), " x  ");
        canvas.put(2, 0, "本", Ink::Plain, 2);
        canvas.put(2, 0, "y", Ink::Plain, 1);
        assert_eq!(canvas.row(0), " xy ");
    }

    #[test]
    fn changes_redraw_only_the_rows_that_differ() {
        let mut old = Canvas::new(4, 2);
        old.put(0, 0, "ab", Ink::Plain, 4);
        let mut new = old.clone();
        new.put(0, 1, "cd", Ink::Plain, 4);
        assert_eq!(new.changes(Some(&old), Depth::None), "\x1b[2;1Hcd  ");
        assert_eq!(new.changes(Some(&new), Depth::None), "");
        assert_eq!(new.changes(None, Depth::None), "\x1b[1;1Hab  \x1b[2;1Hcd  ");
        let small = Canvas::new(4, 1);
        assert_eq!(
            new.changes(Some(&small), Depth::None).matches('H').count(),
            2
        );
    }

    #[test]
    fn changes_switch_the_colors_between_cells() {
        let mut canvas = Canvas::new(5, 1);
        canvas.put(0, 0, "a", Ink::Plain, 5);
        canvas.put(1, 0, "bc", Ink::Text(Style::Host, Kind::Game), 4);
        assert_eq!(
            canvas.changes(None, Depth::Basic),
            "\x1b[1;1H\x1b[0ma\x1b[36mbc\x1b[0m  \x1b[0m"
        );
    }

    #[test]
    fn levels_go_from_green_to_red_at_each_depth() {
        let code = |level, depth| sgr(Ink::Level(level), depth);
        assert_eq!(code(10, Depth::Basic).as_deref(), Some("32"));
        assert_eq!(code(60, Depth::Basic).as_deref(), Some("33"));
        assert_eq!(code(90, Depth::Basic).as_deref(), Some("91"));
        assert_eq!(code(0, Depth::Indexed).as_deref(), Some("38;5;71"));
        assert_eq!(code(100, Depth::Indexed).as_deref(), Some("38;5;167"));
        assert_eq!(code(0, Depth::True).as_deref(), Some("38;2;80;200;120"));
        assert_eq!(code(50, Depth::True).as_deref(), Some("38;2;230;200;60"));
        assert_eq!(code(100, Depth::True).as_deref(), Some("38;2;230;80;80"));
        assert_eq!(sgr(Ink::Dim, Depth::Indexed).as_deref(), Some("38;5;244"));
        assert_eq!(sgr(Ink::Dim, Depth::None), None);
        assert_eq!(sgr(Ink::Plain, Depth::True), None);
    }
}
