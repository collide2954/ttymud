use std::io::{self, BufRead, Write};
use std::sync::mpsc;

use crate::narrate::Narrated;
use crate::session::{Event, View, run};
use crate::{Ending, Shown, Wire};

pub fn line_mode(
    wire: impl Into<Wire>,
    input: impl BufRead + Send + 'static,
    output: impl Write,
    verbose: bool,
) -> io::Result<Ending> {
    let mut plain = Plain {
        output,
        open: false,
    };
    let reader = move |events: &mpsc::Sender<Event>| read(input, events);
    if verbose {
        run(wire.into(), reader, &mut Narrated::new(plain))
    } else {
        run(wire.into(), reader, &mut plain)
    }
}

struct Plain<W> {
    output: W,
    open: bool,
}

impl<W: Write> View for Plain<W> {
    fn show(&mut self, shown: Shown) -> io::Result<()> {
        match shown {
            Shown::Line(message) => self.line(&message.plain())?,
            Shown::Closed(text) => self.line(&text)?,
            Shown::Prompt(prompt) => {
                write!(self.output, "{prompt}")?;
                self.open = true;
            }
            Shown::Candidates { .. } | Shown::Status(_) => return Ok(()),
        }
        self.output.flush()
    }

    fn submitted(&mut self) {
        self.open = false;
    }
}

impl<W: Write> Plain<W> {
    fn line(&mut self, text: &str) -> io::Result<()> {
        if std::mem::take(&mut self.open) {
            writeln!(self.output)?;
        }
        writeln!(self.output, "{text}")
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
    use std::io::{self, Cursor};
    use std::net::{TcpListener, TcpStream};
    use std::sync::mpsc;
    use std::thread;
    use std::time::Duration;

    use ttymud_text::Message;
    use ttymud_wire::{ClientMessage, ServerMessage};

    use super::line_mode;
    use crate::testing::{echo, link, ticker};
    use crate::{Ending, Wire};

    fn session(input: &str) -> (Ending, String) {
        let input = Cursor::new(input.as_bytes().to_vec());
        let mut screen = Vec::new();
        let ending = line_mode(link(echo), input, &mut screen, false).unwrap();
        (ending, String::from_utf8_lossy(&screen).into_owned())
    }

    #[test]
    fn a_refresh_shows_the_prompt_but_is_not_an_answer() {
        let input = Cursor::new(b"look\nquit\n".to_vec());
        let mut screen = Vec::new();
        line_mode(link(ticker), input, &mut screen, false).unwrap();
        let text = String::from_utf8_lossy(&screen);
        assert!(text.contains("tick\n% "), "{text}");
        assert!(text.contains("echo look\n"), "{text}");
    }

    #[test]
    fn text_after_an_open_prompt_starts_on_a_new_line() {
        let input = Cursor::new(b"look\nquit\n".to_vec());
        let mut screen = Vec::new();
        line_mode(link(ticker), input, &mut screen, false).unwrap();
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

    #[test]
    fn a_session_runs_over_a_wire_of_channels() {
        let (to_client, from_server) = mpsc::channel();
        let (to_server, from_client) = mpsc::channel();
        let welcome = ServerMessage::Welcome {
            city: "Test City".to_owned(),
            name: "anon-test".to_owned(),
        };
        for message in [welcome, ServerMessage::Prompt("$ ".to_owned())] {
            to_client.send(message).unwrap();
        }
        thread::spawn(move || {
            for message in from_client {
                if let ClientMessage::Command(line) = message {
                    let echo = Message::game(format!("echo {line}"));
                    let _ = to_client.send(ServerMessage::Text(echo));
                    let _ = to_client.send(ServerMessage::Prompt("$ ".to_owned()));
                }
            }
        });
        let wire = Wire::new(
            move || from_server.recv().ok(),
            move |message| to_server.send(message).map_err(io::Error::other),
            || {},
        );
        let input = Cursor::new(b"look\nquit\n".to_vec());
        let mut screen = Vec::new();
        let ending = line_mode(wire, input, &mut screen, false).unwrap();
        assert_eq!(ending, Ending::Quit);
        let text = String::from_utf8_lossy(&screen);
        assert!(
            text.starts_with("Connected to Test City as anon-test.\n$ "),
            "{text}"
        );
        assert!(text.contains("echo look\n$ "), "{text}");
    }

    #[test]
    fn quit_ends_the_session_when_the_socket_does_not_shut_down() {
        let mut link = link(echo);
        let other = TcpListener::bind("127.0.0.1:0").unwrap();
        link.control = TcpStream::connect(other.local_addr().unwrap()).unwrap();
        let (done, ended) = mpsc::channel();
        thread::spawn(move || {
            let input = Cursor::new(b"look\nquit\n".to_vec());
            let mut screen = Vec::new();
            let _ = done.send(line_mode(link, input, &mut screen, false).ok());
        });
        let ending = ended.recv_timeout(Duration::from_secs(10));
        assert_eq!(ending, Ok(Some(Ending::Quit)));
        drop(other);
    }
}
