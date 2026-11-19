use std::io::{self, BufRead, Write};
use std::net::{Shutdown, TcpStream};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use ttymud_net::Receiver;
use ttymud_text::clean;
use ttymud_wire::{ClientMessage, ServerMessage};

use crate::{Link, Shown, render};

const DRAIN: Duration = Duration::from_secs(5);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ending {
    Quit,
    Closed,
}

enum Event {
    Line(String),
    InputEnded,
    Prompted,
    LinkEnded,
}

pub fn line_mode(
    link: Link,
    input: impl BufRead + Send + 'static,
    output: impl Write + Send + 'static,
) -> io::Result<Ending> {
    let Link {
        receiver,
        mut sender,
        control,
        ..
    } = link;
    let (events, inbox) = mpsc::channel();
    let display = {
        let events = events.clone();
        thread::Builder::new()
            .name("display".to_owned())
            .spawn(move || {
                let result = show(receiver, output, &events);
                let _ = events.send(Event::LinkEnded);
                result
            })?
    };
    thread::Builder::new()
        .name("input".to_owned())
        .spawn(move || read(input, &events))?;
    let (mut sent, mut prompts, mut draining) = (0_usize, 0_usize, false);
    let mut ending = Ending::Quit;
    loop {
        let next = if draining {
            inbox.recv_timeout(DRAIN).ok()
        } else {
            inbox.recv().ok()
        };
        let Some(event) = next else {
            break;
        };
        match event {
            Event::Line(_) if draining => {}
            Event::Line(command) if is_quit(&command) => draining = true,
            Event::Line(command) => {
                if sender
                    .send(&ClientMessage::Command(command).encode())
                    .is_err()
                {
                    ending = Ending::Closed;
                    break;
                }
                sent += 1;
            }
            Event::InputEnded => draining = true,
            Event::Prompted => prompts += 1,
            Event::LinkEnded => {
                ending = Ending::Closed;
                break;
            }
        }
        if draining && prompts > sent {
            break;
        }
    }
    let _ = control.shutdown(Shutdown::Both);
    display
        .join()
        .map_err(|_| io::Error::other("the display thread stopped"))??;
    Ok(ending)
}

pub(crate) fn is_quit(line: &str) -> bool {
    clean(line).trim().eq_ignore_ascii_case("quit")
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

fn show(
    mut receiver: Receiver<TcpStream>,
    mut output: impl Write,
    events: &mpsc::Sender<Event>,
) -> io::Result<()> {
    while let Ok(bytes) = receiver.receive() {
        let Ok(message) = ServerMessage::decode(&bytes) else {
            break;
        };
        match render(&message) {
            Shown::Line(text) => writeln!(output, "{text}")?,
            Shown::Prompt(prompt) => write!(output, "{prompt}")?,
            Shown::Closed(reason) => {
                writeln!(output, "{reason}")?;
                output.flush()?;
                break;
            }
        }
        output.flush()?;
        if matches!(message, ServerMessage::Prompt(_)) {
            let _ = events.send(Event::Prompted);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::io::{self, Cursor, Write};
    use std::sync::{Arc, Mutex};

    use super::{Ending, line_mode};
    use crate::connect;
    use crate::testing::{echo, server};

    #[derive(Clone, Default)]
    struct Screen(Arc<Mutex<Vec<u8>>>);

    impl Write for Screen {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(bytes);
            Ok(bytes.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    impl Screen {
        fn text(&self) -> String {
            String::from_utf8_lossy(&self.0.lock().unwrap()).into_owned()
        }
    }

    fn session(input: &str) -> (Ending, String) {
        let (address, _) = server(echo);
        let dir = std::env::temp_dir().join(format!(
            "ttymud-line-{}-{}",
            std::process::id(),
            input.len()
        ));
        let keys = ttymud_net::Keys::generate().unwrap();
        let link = connect(&address.to_string(), &keys, &dir, |_, _| Ok(true)).unwrap();
        let _ = std::fs::remove_file(&dir);
        let screen = Screen::default();
        let input = Cursor::new(input.as_bytes().to_vec());
        let ending = line_mode(link, input, screen.clone()).unwrap();
        (ending, screen.text())
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
