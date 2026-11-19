use std::io::{self, BufRead, Write};
use std::sync::mpsc;

use crate::session::{Event, View, run};
use crate::{Ending, Link, Shown};

pub fn line_mode(
    link: Link,
    input: impl BufRead + Send + 'static,
    output: impl Write,
) -> io::Result<Ending> {
    let mut plain = Plain {
        output,
        open: false,
    };
    run(link, move |events| read(input, events), &mut plain)
}

struct Plain<W> {
    output: W,
    open: bool,
}

impl<W: Write> View for Plain<W> {
    fn show(&mut self, shown: Shown) -> io::Result<()> {
        match shown {
            Shown::Line(text) | Shown::Closed(text) => {
                if std::mem::take(&mut self.open) {
                    writeln!(self.output)?;
                }
                writeln!(self.output, "{text}")?;
            }
            Shown::Prompt(prompt) => {
                write!(self.output, "{prompt}")?;
                self.open = true;
            }
            Shown::Candidates { .. } => return Ok(()),
        }
        self.output.flush()
    }

    fn submitted(&mut self) {
        self.open = false;
    }
}

fn read(input: impl BufRead, events: &mpsc::Sender<Event>) {
    for line in input.lines() {
        let Ok(line) = line else {
            break;
        };
        if events.send(Event::Line(line)).is_err() {
            return;
        }
    }
    let _ = events.send(Event::InputEnded);
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::line_mode;
    use crate::Ending;
    use crate::testing::{echo, link, ticker};

    fn session(input: &str) -> (Ending, String) {
        let input = Cursor::new(input.as_bytes().to_vec());
        let mut screen = Vec::new();
        let ending = line_mode(link(echo), input, &mut screen).unwrap();
        (ending, String::from_utf8_lossy(&screen).into_owned())
    }

    #[test]
    fn a_refresh_shows_the_prompt_but_is_not_an_answer() {
        let input = Cursor::new(b"look\nquit\n".to_vec());
        let mut screen = Vec::new();
        line_mode(link(ticker), input, &mut screen).unwrap();
        let text = String::from_utf8_lossy(&screen);
        assert!(text.contains("tick\n% "), "{text}");
        assert!(text.contains("echo look\n"), "{text}");
    }

    #[test]
    fn text_after_an_open_prompt_starts_on_a_new_line() {
        let input = Cursor::new(b"look\nquit\n".to_vec());
        let mut screen = Vec::new();
        line_mode(link(ticker), input, &mut screen).unwrap();
        let text = String::from_utf8_lossy(&screen);
        assert!(text.contains("% \necho look\n$ "), "{text}");
    }

    #[test]
    fn the_welcome_and_the_prompt_are_shown() {
        let (_, text) = session("quit\n");
        assert!(text.starts_with("Connected to Test City as anon-test.\nHello.\n$ "));
    }

    #[test]
    fn lines_go_to_the_server_and_answers_come_back() {
        let (ending, text) = session("look\nquit\n");
        assert_eq!(ending, Ending::Quit);
        assert!(text.contains("echo look\n$ "), "{text}");
    }

    #[test]
    fn a_close_from_the_server_ends_the_session() {
        let (ending, text) = session("close\nlook\n");
        assert_eq!(ending, Ending::Closed);
        assert!(text.contains("Bye.\n"), "{text}");
        assert!(!text.contains("echo look"), "{text}");
    }

    #[test]
    fn invisible_characters_do_not_hide_quit() {
        let (ending, text) = session("\u{feff}quit\n");
        assert_eq!(ending, Ending::Quit);
        assert!(!text.contains("echo"), "{text}");
    }

    #[test]
    fn the_end_of_the_input_quits() {
        let (ending, _) = session("");
        assert_eq!(ending, Ending::Quit);
    }
}
