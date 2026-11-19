use std::io::{self, Write};

use crate::Shown;
use crate::editor::{Action, Editor, Key};
use crate::session::{Typed, View};

const CLEAR: &str = "\r\x1b[K";
const SAVE: &str = "\x1b7";
const RESTORE: &str = "\x1b8";

pub(crate) struct Screen<W> {
    output: W,
    editor: Editor,
    prompt: String,
    width: usize,
    start: usize,
}

impl<W: Write> Screen<W> {
    pub(crate) fn new(output: W, width: u16) -> Self {
        Self {
            output,
            editor: Editor::default(),
            prompt: String::new(),
            width: usize::from(width),
            start: 0,
        }
    }

    fn draw(&mut self) -> io::Result<()> {
        let line: Vec<char> = self.editor.line().chars().collect();
        let cursor = self.editor.cursor();
        let room = self
            .width
            .saturating_sub(self.prompt.chars().count() + 1)
            .max(1);
        self.start = self
            .start
            .min(cursor)
            .max((cursor + 1).saturating_sub(room));
        let end = line.len().min(self.start + room);
        let before: String = line[self.start..cursor].iter().collect();
        let after: String = line[cursor..end].iter().collect();
        write!(self.output, "{CLEAR}{}{before}", self.prompt)?;
        if !after.is_empty() {
            write!(self.output, "{SAVE}{after}{RESTORE}")?;
        }
        self.output.flush()
    }

    fn above(&mut self, text: &str) -> io::Result<()> {
        write!(self.output, "{CLEAR}{text}\r\n")
    }
}

impl<W: Write> View for Screen<W> {
    fn show(&mut self, shown: Shown) -> io::Result<()> {
        match shown {
            Shown::Line(text) => self.above(&text)?,
            Shown::Prompt(prompt) => self.prompt = prompt,
            Shown::Closed(reason) => {
                self.above(&reason)?;
                return self.output.flush();
            }
            Shown::Candidates { .. } => return Ok(()),
        }
        self.draw()
    }

    fn press(&mut self, key: Key) -> io::Result<Typed> {
        let typed = match self.editor.press(key) {
            Action::Nothing | Action::Complete(_) => return Ok(Typed::Nothing),
            Action::Quit => return Ok(Typed::Quit),
            Action::Redraw => Typed::Nothing,
            Action::Submit(line) => {
                let shown = format!("{}{line}", self.prompt);
                self.above(&shown)?;
                self.start = 0;
                Typed::Line(line)
            }
        };
        self.draw()?;
        Ok(typed)
    }

    fn resize(&mut self, width: u16) -> io::Result<()> {
        self.width = usize::from(width);
        self.draw()
    }

    fn finish(&mut self) -> io::Result<()> {
        write!(self.output, "{CLEAR}")?;
        self.output.flush()
    }
}

#[cfg(test)]
mod tests {
    use super::Screen;
    use crate::Ending;
    use crate::Shown;
    use crate::editor::Key;
    use crate::session::{Event, Typed, View, run};
    use crate::testing::{echo, link};

    fn screen(width: u16) -> Screen<Vec<u8>> {
        let mut screen = Screen::new(Vec::new(), width);
        screen.show(Shown::Prompt("$ ".to_owned())).unwrap();
        screen
    }

    fn typed(text: &str, width: u16) -> Screen<Vec<u8>> {
        let mut screen = screen(width);
        for c in text.chars() {
            screen.press(Key::Char(c)).unwrap();
        }
        screen.output.clear();
        screen
    }

    fn drawn(screen: &mut Screen<Vec<u8>>) -> String {
        let text = String::from_utf8_lossy(&screen.output).into_owned();
        screen.output.clear();
        text
    }

    #[test]
    fn the_input_line_shows_the_prompt_and_the_typed_text() {
        let mut screen = typed("l", 80);
        screen.press(Key::Char('s')).unwrap();
        assert_eq!(drawn(&mut screen), "\r\x1b[K$ ls");
    }

    #[test]
    fn server_text_goes_above_the_input_line() {
        let mut screen = typed("say hi", 80);
        screen.show(Shown::Line("<anon-1> hey".to_owned())).unwrap();
        assert_eq!(
            drawn(&mut screen),
            "\r\x1b[K<anon-1> hey\r\n\r\x1b[K$ say hi"
        );
    }

    #[test]
    fn the_cursor_stays_in_its_place() {
        let mut screen = typed("abc", 80);
        screen.press(Key::Left).unwrap();
        assert_eq!(drawn(&mut screen), "\r\x1b[K$ ab\x1b7c\x1b8");
    }

    #[test]
    fn enter_keeps_the_line_and_starts_a_new_one() {
        let mut screen = typed("ls", 80);
        assert_eq!(
            screen.press(Key::Enter).unwrap(),
            Typed::Line("ls".to_owned())
        );
        assert_eq!(drawn(&mut screen), "\r\x1b[K$ ls\r\n\r\x1b[K$ ");
    }

    #[test]
    fn a_long_line_scrolls_to_keep_the_cursor_in_view() {
        let mut screen = typed("abcdefghi", 10);
        screen.press(Key::Char('j')).unwrap();
        assert_eq!(drawn(&mut screen), "\r\x1b[K$ efghij");
        screen.press(Key::Home).unwrap();
        assert_eq!(drawn(&mut screen), "\r\x1b[K$ \x1b7abcdefg\x1b8");
    }

    #[test]
    fn a_new_width_draws_the_line_again() {
        let mut screen = typed("abcdefghij", 80);
        screen.resize(10).unwrap();
        assert_eq!(drawn(&mut screen), "\r\x1b[K$ efghij");
    }

    #[test]
    fn a_new_prompt_replaces_the_old_one() {
        let mut screen = typed("ls", 80);
        screen.show(Shown::Prompt("relay $ ".to_owned())).unwrap();
        assert_eq!(drawn(&mut screen), "\r\x1b[Krelay $ ls");
    }

    #[test]
    fn a_close_shows_the_reason_and_no_input_line() {
        let mut screen = typed("ls", 80);
        screen.show(Shown::Closed("Bye.".to_owned())).unwrap();
        assert_eq!(drawn(&mut screen), "\r\x1b[KBye.\r\n");
        screen.finish().unwrap();
        assert_eq!(drawn(&mut screen), "\r\x1b[K");
    }

    #[test]
    fn quit_keys_on_an_empty_line_end_the_session() {
        let mut screen = screen(80);
        assert_eq!(screen.press(Key::EndOfInput).unwrap(), Typed::Quit);
        assert_eq!(screen.press(Key::Cancel).unwrap(), Typed::Quit);
    }

    #[test]
    fn keys_drive_a_session_on_a_server() {
        let keys = [Key::Char('l'), Key::Char('s'), Key::Enter, Key::EndOfInput];
        let feed = move |events: &std::sync::mpsc::Sender<Event>| {
            for key in keys {
                let _ = events.send(Event::Key(key));
            }
        };
        let mut screen = Screen::new(Vec::new(), 80);
        assert_eq!(run(link(echo), feed, &mut screen).unwrap(), Ending::Quit);
        let text = String::from_utf8_lossy(&screen.output).into_owned();
        assert!(text.contains("\r\x1b[Kecho ls\r\n"), "{text:?}");
    }
}
