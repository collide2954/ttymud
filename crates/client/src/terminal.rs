use std::io::{self, IsTerminal};
use std::sync::mpsc;

use crossterm::event::{self, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use crossterm::terminal;

use crate::editor::Key;
use crate::screen::Screen;
use crate::session::{Event, run};
use crate::{Ending, Link};

#[must_use]
pub fn can_edit() -> bool {
    io::stdin().is_terminal()
        && io::stdout().is_terminal()
        && std::env::var_os("TERM").is_none_or(|term| term != "dumb")
        && ansi()
}

pub fn edit_mode(link: Link) -> io::Result<Ending> {
    let _raw = Raw::enable()?;
    let width = terminal::size().map_or(80, |(width, _)| width);
    run(link, read_keys, &mut Screen::new(io::stdout(), width))
}

#[cfg(windows)]
fn ansi() -> bool {
    crossterm::ansi_support::supports_ansi()
}

#[cfg(not(windows))]
fn ansi() -> bool {
    true
}

struct Raw;

impl Raw {
    fn enable() -> io::Result<Self> {
        terminal::enable_raw_mode()?;
        Ok(Self)
    }
}

impl Drop for Raw {
    fn drop(&mut self) {
        let _ = terminal::disable_raw_mode();
    }
}

fn read_keys(events: &mpsc::Sender<Event>) {
    loop {
        let event = match event::read() {
            Ok(event::Event::Key(key)) => match to_key(key) {
                Some(key) => Event::Key(key),
                None => continue,
            },
            Ok(event::Event::Resize(width, _)) => Event::Resize(width),
            Ok(_) => continue,
            Err(_) => Event::InputEnded,
        };
        let ended = matches!(event, Event::InputEnded);
        if events.send(event).is_err() || ended {
            return;
        }
    }
}

fn to_key(event: KeyEvent) -> Option<Key> {
    if event.kind == KeyEventKind::Release {
        return None;
    }
    let control = event.modifiers.contains(KeyModifiers::CONTROL);
    let alt = event.modifiers.contains(KeyModifiers::ALT);
    let key = match event.code {
        KeyCode::Char(c) if control && !alt => control_key(c)?,
        KeyCode::Char(_) if alt && !control => return None,
        KeyCode::Char(c) => Key::Char(c),
        KeyCode::Backspace if control || alt => Key::DeleteWord,
        KeyCode::Backspace => Key::Backspace,
        KeyCode::Enter => Key::Enter,
        KeyCode::Delete => Key::Delete,
        KeyCode::Left => Key::Left,
        KeyCode::Right => Key::Right,
        KeyCode::Home => Key::Home,
        KeyCode::End => Key::End,
        KeyCode::Up => Key::Up,
        KeyCode::Down => Key::Down,
        KeyCode::Tab => Key::Tab,
        _ => return None,
    };
    Some(key)
}

fn control_key(c: char) -> Option<Key> {
    let key = match c.to_ascii_lowercase() {
        'a' => Key::Home,
        'b' => Key::Left,
        'c' => Key::Cancel,
        'd' => Key::EndOfInput,
        'e' => Key::End,
        'f' => Key::Right,
        'h' => Key::Backspace,
        'j' | 'm' => Key::Enter,
        'k' => Key::DeleteToEnd,
        'n' => Key::Down,
        'p' => Key::Up,
        'u' => Key::DeleteToStart,
        'w' => Key::DeleteWord,
        _ => return None,
    };
    Some(key)
}

#[cfg(test)]
mod tests {
    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

    use super::to_key;
    use crate::editor::Key;

    fn press(code: KeyCode, modifiers: KeyModifiers) -> Option<Key> {
        to_key(KeyEvent::new(code, modifiers))
    }

    #[test]
    fn plain_keys_map_to_editor_keys() {
        assert_eq!(
            press(KeyCode::Char('x'), KeyModifiers::NONE),
            Some(Key::Char('x'))
        );
        assert_eq!(
            press(KeyCode::Char('X'), KeyModifiers::SHIFT),
            Some(Key::Char('X'))
        );
        assert_eq!(press(KeyCode::Enter, KeyModifiers::NONE), Some(Key::Enter));
        assert_eq!(press(KeyCode::Left, KeyModifiers::NONE), Some(Key::Left));
        assert_eq!(press(KeyCode::Tab, KeyModifiers::NONE), Some(Key::Tab));
        assert_eq!(press(KeyCode::F(1), KeyModifiers::NONE), None);
    }

    #[test]
    fn control_keys_are_the_usual_shell_keys() {
        let control = |c| press(KeyCode::Char(c), KeyModifiers::CONTROL);
        assert_eq!(control('a'), Some(Key::Home));
        assert_eq!(control('e'), Some(Key::End));
        assert_eq!(control('w'), Some(Key::DeleteWord));
        assert_eq!(control('u'), Some(Key::DeleteToStart));
        assert_eq!(control('k'), Some(Key::DeleteToEnd));
        assert_eq!(control('c'), Some(Key::Cancel));
        assert_eq!(control('d'), Some(Key::EndOfInput));
        assert_eq!(control('j'), Some(Key::Enter));
        assert_eq!(control('z'), None);
    }

    #[test]
    fn key_releases_are_ignored() {
        let release = KeyEvent::new_with_kind(
            KeyCode::Char('x'),
            KeyModifiers::NONE,
            KeyEventKind::Release,
        );
        assert_eq!(to_key(release), None);
    }

    #[test]
    fn altgr_characters_are_typed() {
        let altgr = KeyModifiers::CONTROL | KeyModifiers::ALT;
        assert_eq!(press(KeyCode::Char('@'), altgr), Some(Key::Char('@')));
    }

    #[test]
    fn alt_letters_type_nothing_and_alt_backspace_deletes_a_word() {
        assert_eq!(press(KeyCode::Char('b'), KeyModifiers::ALT), None);
        assert_eq!(
            press(KeyCode::Backspace, KeyModifiers::ALT),
            Some(Key::DeleteWord)
        );
    }
}
