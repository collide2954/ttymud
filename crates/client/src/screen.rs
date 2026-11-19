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
    asked: Option<String>,
}

impl<W: Write> Screen<W> {
    pub(crate) fn new(output: W, width: u16) -> Self {
        Self {
            output,
            editor: Editor::default(),
            prompt: String::new(),
            width: usize::from(width),
            start: 0,
            asked: None,
        }
    }

    fn complete(&mut self, word: &str, names: &[String]) -> io::Result<()> {
        let Some(asked) = self.asked.take() else {
            return Ok(());
        };
        if asked != self.editor.before() || !asked.ends_with(word) || names.is_empty() {
            return Ok(());
        }
        let count = word.chars().count();
        let common = common_start(names);
        if let [name] = names {
            self.editor.replace_before(count, &format!("{name} "));
        } else if common.chars().count() > count {
            self.editor.replace_before(count, &common);
        } else {
            self.above(&names.join("  "))?;
        }
        self.draw()
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
            Shown::Candidates { word, names } => return self.complete(&word, &names),
            Shown::Status(_) => return Ok(()),
        }
        self.draw()
    }

    fn press(&mut self, key: Key) -> io::Result<Typed> {
        let typed = match self.editor.press(key) {
            Action::Nothing => return Ok(Typed::Nothing),
            Action::Complete(text) => {
                self.asked = Some(text.clone());
                return Ok(Typed::Complete(text));
            }
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

fn common_start(names: &[String]) -> String {
    let Some((first, rest)) = names.split_first() else {
        return String::new();
    };
    let mut length = first.chars().count();
    for name in rest {
        length = first
            .chars()
            .zip(name.chars())
            .take(length)
            .take_while(|(a, b)| a == b)
            .count();
    }
    first.chars().take(length).collect()
}

#[cfg(test)]
mod tests {
    use std::net::TcpStream;

    use ttymud_net::{Receiver, Sender};
    use ttymud_wire::{ClientMessage, ServerMessage};

    use super::Screen;
    use crate::Ending;
    use crate::Shown;
    use crate::editor::Key;
    use crate::session::{Event, Typed, View, run};
    use crate::testing::{echo, link, send};

    fn completer(mut receiver: Receiver<TcpStream>, mut sender: Sender<TcpStream>) {
        send(&mut sender, &ServerMessage::Prompt("$ ".to_owned()));
        while let Ok(bytes) = receiver.receive() {
            if let Ok(ClientMessage::Complete(_)) = ClientMessage::decode(&bytes) {
                let candidates = ServerMessage::Candidates {
                    word: "ma".to_owned(),
                    names: vec!["market".to_owned()],
                };
                send(&mut sender, &candidates);
                send(&mut sender, &ServerMessage::Close("Bye.".to_owned()));
                while receiver.receive().is_ok() {}
                return;
            }
        }
    }

    fn feed(keys: Vec<Key>) -> impl FnOnce(&std::sync::mpsc::Sender<Event>) + Send + 'static {
        move |events| {
            for key in keys {
                let _ = events.send(Event::Key(key));
            }
        }
    }

    fn asked(text: &str) -> Screen<Vec<u8>> {
        let mut screen = typed(text, 80);
        assert_eq!(
            screen.press(Key::Tab).unwrap(),
            Typed::Complete(text.to_owned())
        );
        screen.output.clear();
        screen
    }

    fn candidates(word: &str, names: &[&str]) -> Shown {
        Shown::Candidates {
            word: word.to_owned(),
            names: names.iter().map(|&name| name.to_owned()).collect(),
        }
    }

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
    fn one_candidate_completes_the_word() {
        let mut screen = asked("go MA");
        screen.show(candidates("MA", &["market"])).unwrap();
        assert_eq!(drawn(&mut screen), "\r\x1b[K$ go market ");
    }

    #[test]
    fn many_candidates_complete_their_common_start() {
        let mut screen = asked("go b");
        screen
            .show(candidates("b", &["back-room", "backdoor"]))
            .unwrap();
        assert_eq!(drawn(&mut screen), "\r\x1b[K$ go back");
    }

    #[test]
    fn candidates_without_more_in_common_show_above_the_line() {
        let mut screen = asked("go n");
        screen
            .show(candidates("n", &["night-market", "newsstand"]))
            .unwrap();
        assert_eq!(
            drawn(&mut screen),
            "\r\x1b[Knight-market  newsstand\r\n\r\x1b[K$ go n"
        );
    }

    #[test]
    fn late_unasked_or_empty_candidates_change_nothing() {
        let mut late = asked("go ma");
        late.press(Key::Char('x')).unwrap();
        late.output.clear();
        late.show(candidates("ma", &["market"])).unwrap();
        let mut unasked = typed("go ma", 80);
        unasked.show(candidates("ma", &["market"])).unwrap();
        let mut empty = asked("xyzzy");
        empty.show(candidates("xyzzy", &[])).unwrap();
        for mut screen in [late, unasked, empty] {
            assert_eq!(drawn(&mut screen), "");
        }
    }

    #[test]
    fn tab_completes_through_the_server() {
        let keys = "go ma".chars().map(Key::Char).chain([Key::Tab]).collect();
        let mut screen = Screen::new(Vec::new(), 80);
        let ending = run(link(completer), feed(keys), &mut screen).unwrap();
        assert_eq!(ending, Ending::Closed);
        let text = String::from_utf8_lossy(&screen.output).into_owned();
        assert!(text.contains("\r\x1b[K$ go market "), "{text:?}");
    }

    #[test]
    fn keys_drive_a_session_on_a_server() {
        let keys = vec![Key::Char('l'), Key::Char('s'), Key::Enter, Key::EndOfInput];
        let mut screen = Screen::new(Vec::new(), 80);
        let ending = run(link(echo), feed(keys), &mut screen).unwrap();
        assert_eq!(ending, Ending::Quit);
        let text = String::from_utf8_lossy(&screen.output).into_owned();
        assert!(text.contains("\r\x1b[Kecho ls\r\n"), "{text:?}");
    }
}
