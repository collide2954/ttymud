#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Char(char),
    Enter,
    Backspace,
    Delete,
    Left,
    Right,
    Home,
    End,
    Up,
    Down,
    Tab,
    DeleteWord,
    DeleteToStart,
    DeleteToEnd,
    Cancel,
    EndOfInput,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    Nothing,
    Redraw,
    Submit(String),
    Complete(String),
    Quit,
}

#[derive(Debug, Default)]
pub struct Editor {
    buffer: Vec<char>,
    cursor: usize,
    history: Vec<String>,
    recall: Option<usize>,
    draft: Vec<char>,
}

impl Editor {
    pub fn press(&mut self, key: Key) -> Action {
        match key {
            Key::Char(c) => {
                self.insert(c.encode_utf8(&mut [0; 4]));
                Action::Redraw
            }
            Key::Enter => self.submit(),
            Key::Backspace => self.backspace(),
            Key::Cancel | Key::EndOfInput if self.buffer.is_empty() => Action::Quit,
            Key::Delete | Key::EndOfInput => self.delete(),
            Key::Left => self.move_to(self.cursor.saturating_sub(1)),
            Key::Right => self.move_to((self.cursor + 1).min(self.buffer.len())),
            Key::Home => self.move_to(0),
            Key::End => self.move_to(self.buffer.len()),
            Key::Up => self.older(),
            Key::Down => self.newer(),
            Key::Tab => Action::Complete(self.buffer[..self.cursor].iter().collect()),
            Key::DeleteWord => self.delete_word(),
            Key::DeleteToStart => {
                self.buffer.drain(..self.cursor);
                self.cursor = 0;
                Action::Redraw
            }
            Key::DeleteToEnd => {
                self.buffer.truncate(self.cursor);
                Action::Redraw
            }
            Key::Cancel => {
                self.replace(Vec::new());
                Action::Redraw
            }
        }
    }

    #[must_use]
    pub fn line(&self) -> String {
        self.buffer.iter().collect()
    }

    #[must_use]
    pub fn cursor(&self) -> usize {
        self.cursor
    }

    pub fn insert(&mut self, text: &str) {
        for c in text.chars().filter(|c| !c.is_control()) {
            self.buffer.insert(self.cursor, c);
            self.cursor += 1;
        }
    }

    fn submit(&mut self) -> Action {
        let line = self.line();
        self.replace(Vec::new());
        self.recall = None;
        if !line.trim().is_empty() && self.history.last() != Some(&line) {
            self.history.push(line.clone());
        }
        Action::Submit(line)
    }

    fn backspace(&mut self) -> Action {
        if self.cursor == 0 {
            return Action::Nothing;
        }
        self.cursor -= 1;
        self.buffer.remove(self.cursor);
        Action::Redraw
    }

    fn delete(&mut self) -> Action {
        if self.cursor == self.buffer.len() {
            return Action::Nothing;
        }
        self.buffer.remove(self.cursor);
        Action::Redraw
    }

    fn delete_word(&mut self) -> Action {
        let mut start = self.cursor;
        while start > 0 && self.buffer[start - 1].is_whitespace() {
            start -= 1;
        }
        while start > 0 && !self.buffer[start - 1].is_whitespace() {
            start -= 1;
        }
        if start == self.cursor {
            return Action::Nothing;
        }
        self.buffer.drain(start..self.cursor);
        self.cursor = start;
        Action::Redraw
    }

    fn move_to(&mut self, position: usize) -> Action {
        self.cursor = position;
        Action::Redraw
    }

    fn older(&mut self) -> Action {
        let index = match self.recall {
            None if self.history.is_empty() => return Action::Nothing,
            None => {
                self.draft = self.buffer.clone();
                self.history.len() - 1
            }
            Some(0) => return Action::Nothing,
            Some(index) => index - 1,
        };
        self.recall = Some(index);
        self.replace(self.history[index].chars().collect());
        Action::Redraw
    }

    fn newer(&mut self) -> Action {
        let Some(index) = self.recall else {
            return Action::Nothing;
        };
        if index + 1 == self.history.len() {
            self.recall = None;
            let draft = std::mem::take(&mut self.draft);
            self.replace(draft);
        } else {
            self.recall = Some(index + 1);
            self.replace(self.history[index + 1].chars().collect());
        }
        Action::Redraw
    }

    fn replace(&mut self, buffer: Vec<char>) {
        self.cursor = buffer.len();
        self.buffer = buffer;
    }
}

#[cfg(test)]
mod tests {
    use super::{Action, Editor, Key};

    fn typed(text: &str) -> Editor {
        let mut editor = Editor::default();
        for c in text.chars() {
            editor.press(Key::Char(c));
        }
        editor
    }

    fn press(editor: &mut Editor, keys: &[Key]) {
        for &key in keys {
            editor.press(key);
        }
    }

    #[test]
    fn typing_builds_the_line() {
        let editor = typed("look");
        assert_eq!((editor.line().as_str(), editor.cursor()), ("look", 4));
    }

    #[test]
    fn arrows_and_home_and_end_move_the_cursor() {
        let mut editor = typed("abc");
        press(&mut editor, &[Key::Left, Key::Left]);
        assert_eq!(editor.cursor(), 1);
        press(&mut editor, &[Key::Home, Key::Left]);
        assert_eq!(editor.cursor(), 0);
        press(&mut editor, &[Key::End, Key::Right]);
        assert_eq!(editor.cursor(), 3);
    }

    #[test]
    fn characters_go_in_at_the_cursor() {
        let mut editor = typed("lk");
        editor.press(Key::Left);
        editor.press(Key::Char('o'));
        editor.press(Key::Char('o'));
        assert_eq!((editor.line().as_str(), editor.cursor()), ("look", 3));
    }

    #[test]
    fn backspace_and_delete_remove_one_character() {
        let mut editor = typed("look");
        press(&mut editor, &[Key::Backspace, Key::Home, Key::Delete]);
        assert_eq!(editor.line(), "oo");
        assert_eq!(editor.press(Key::Backspace), Action::Nothing);
        editor.press(Key::End);
        assert_eq!(editor.press(Key::Delete), Action::Nothing);
    }

    #[test]
    fn delete_word_removes_the_word_before_the_cursor() {
        let mut editor = typed("go night-market  ");
        editor.press(Key::DeleteWord);
        assert_eq!(editor.line(), "go ");
    }

    #[test]
    fn whole_parts_of_the_line_can_go_at_once() {
        let mut editor = typed("go market");
        press(&mut editor, &[Key::Left, Key::Left, Key::DeleteToEnd]);
        assert_eq!(editor.line(), "go mark");
        editor.press(Key::DeleteToStart);
        assert_eq!((editor.line().as_str(), editor.cursor()), ("", 0));
    }

    #[test]
    fn enter_submits_and_clears_the_line() {
        let mut editor = typed("ls");
        assert_eq!(editor.press(Key::Enter), Action::Submit("ls".to_owned()));
        assert_eq!((editor.line().as_str(), editor.cursor()), ("", 0));
    }

    #[test]
    fn up_and_down_walk_the_history_and_keep_the_draft() {
        let mut editor = Editor::default();
        for line in ["look", "ls"] {
            editor.insert(line);
            editor.press(Key::Enter);
        }
        editor.insert("go");
        editor.press(Key::Up);
        assert_eq!(editor.line(), "ls");
        editor.press(Key::Up);
        assert_eq!(editor.line(), "look");
        assert_eq!(editor.press(Key::Up), Action::Nothing);
        press(&mut editor, &[Key::Down, Key::Down]);
        assert_eq!((editor.line().as_str(), editor.cursor()), ("go", 2));
    }

    #[test]
    fn the_history_skips_empty_lines_and_repeats() {
        let mut editor = Editor::default();
        for line in ["ls", "ls", ""] {
            editor.insert(line);
            editor.press(Key::Enter);
        }
        editor.press(Key::Up);
        assert_eq!(editor.line(), "ls");
        assert_eq!(editor.press(Key::Up), Action::Nothing);
    }

    #[test]
    fn cancel_clears_a_line_or_quits_on_an_empty_line() {
        let mut editor = typed("look");
        assert_eq!(editor.press(Key::Cancel), Action::Redraw);
        assert_eq!(editor.line(), "");
        assert_eq!(editor.press(Key::Cancel), Action::Quit);
    }

    #[test]
    fn end_of_input_quits_only_on_an_empty_line() {
        let mut editor = typed("ab");
        editor.press(Key::Home);
        assert_eq!(editor.press(Key::EndOfInput), Action::Redraw);
        assert_eq!(editor.line(), "b");
        editor.press(Key::Delete);
        assert_eq!(editor.press(Key::EndOfInput), Action::Quit);
    }

    #[test]
    fn tab_asks_to_complete_the_text_before_the_cursor() {
        let mut editor = typed("go nig market");
        for _ in 0.." market".len() {
            editor.press(Key::Left);
        }
        assert_eq!(
            editor.press(Key::Tab),
            Action::Complete("go nig".to_owned())
        );
    }

    #[test]
    fn control_characters_never_go_into_the_line() {
        let mut editor = typed("a\u{1b}");
        editor.insert("b\u{7}\r\nc\u{9b}");
        assert_eq!((editor.line().as_str(), editor.cursor()), ("abc", 3));
    }

    #[test]
    fn insert_puts_text_at_the_cursor() {
        let mut editor = typed("go ");
        editor.insert("night-market");
        assert_eq!(
            (editor.line().as_str(), editor.cursor()),
            ("go night-market", 15)
        );
    }
}
