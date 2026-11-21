use std::io::{self, Write};
use std::sync::mpsc;

use crate::hud::Hud;
use crate::keys::Decoder;
use crate::line::Plain;
use crate::narrate::Narrated;
use crate::screen::Screen;
use crate::session::{Event, View, run};
use crate::settings::{Choices, Depth, Layer, Settings, detect};
use crate::{Ending, Wire};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Input {
    Bytes(Vec<u8>),
    Resize(u16, u16),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Remote {
    pub settings: Settings,
    pub width: u16,
    pub height: u16,
}

impl Remote {
    #[must_use]
    pub fn new(
        vars: impl Fn(&str) -> Option<String>,
        size: Option<(u16, u16)>,
        choices: Choices,
    ) -> Self {
        let mut settings = choices.settle(detect(vars, size.is_some(), false));
        if size.is_some() {
            settings.layer = settings.layer.max(Layer::Edit);
        }
        let (width, height) = size.unwrap_or((80, 24));
        Self {
            settings,
            width,
            height,
        }
    }
}

pub fn remote(
    wire: impl Into<Wire>,
    mut input: impl FnMut() -> Option<Input> + Send + 'static,
    output: impl Write,
    terminal: Remote,
) -> io::Result<Ending> {
    let Remote {
        settings,
        width,
        height,
    } = terminal;
    let wire = wire.into();
    let reader = move |events: &mpsc::Sender<Event>| match settings.layer {
        Layer::Line => lines(&mut input, events),
        Layer::Edit | Layer::Color | Layer::Hud => keys(&mut input, events),
    };
    let verbose = settings.verbose;
    match settings.layer {
        Layer::Line => narrated(wire, reader, Plain::new(output), verbose),
        Layer::Edit => narrated(
            wire,
            reader,
            Screen::new(output, width, Depth::None),
            verbose,
        ),
        Layer::Color => narrated(
            wire,
            reader,
            Screen::new(output, width, settings.color),
            verbose,
        ),
        Layer::Hud => {
            let mut hud = Hud::new(output, settings, width, height)?;
            if settings.mouse {
                hud.capture_mouse()?;
            }
            narrated(wire, reader, hud, verbose)
        }
    }
}

fn narrated(
    wire: Wire,
    reader: impl FnOnce(&mpsc::Sender<Event>) + Send + 'static,
    mut view: impl View,
    verbose: bool,
) -> io::Result<Ending> {
    if verbose {
        run(wire, reader, &mut Narrated::new(view))
    } else {
        run(wire, reader, &mut view)
    }
}

fn keys(input: &mut impl FnMut() -> Option<Input>, events: &mpsc::Sender<Event>) {
    let mut decoder = Decoder::default();
    while let Some(item) = input() {
        let batch: Vec<Event> = match item {
            Input::Bytes(bytes) => decoder.feed(&bytes).into_iter().map(Event::Key).collect(),
            Input::Resize(width, height) => vec![Event::Resize(width, height)],
        };
        for event in batch {
            if events.send(event).is_err() {
                return;
            }
        }
    }
    let _ = events.send(Event::InputEnded);
}

fn lines(input: &mut impl FnMut() -> Option<Input>, events: &mpsc::Sender<Event>) {
    let mut pending = Vec::new();
    let line = |bytes: &[u8]| {
        let text = String::from_utf8_lossy(bytes);
        Event::Line(text.trim_end_matches(['\r', '\n']).to_owned())
    };
    while let Some(item) = input() {
        let Input::Bytes(bytes) = item else {
            continue;
        };
        pending.extend_from_slice(&bytes);
        while let Some(end) = pending.iter().position(|&byte| byte == b'\n') {
            let whole: Vec<u8> = pending.drain(..=end).collect();
            if events.send(line(&whole)).is_err() {
                return;
            }
        }
    }
    if !pending.is_empty() {
        let _ = events.send(line(&pending));
    }
    let _ = events.send(Event::InputEnded);
}

#[cfg(test)]
mod tests {
    use super::{Input, Remote, remote};
    use crate::Ending;
    use crate::settings::{Choices, Depth, Layer};
    use crate::testing::echo_wire;

    fn vars(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
        let pairs: Vec<(String, String)> = pairs
            .iter()
            .map(|&(name, value)| (name.to_owned(), value.to_owned()))
            .collect();
        move |name| {
            let found = pairs.iter().find(|(known, _)| known == name);
            found.map(|(_, value)| value.clone())
        }
    }

    fn session<const N: usize>(input: [Input; N], terminal: Remote) -> (Ending, String) {
        let mut items = input.into_iter();
        let mut screen = Vec::new();
        let ending = remote(echo_wire(), move || items.next(), &mut screen, terminal).unwrap();
        (ending, String::from_utf8_lossy(&screen).into_owned())
    }

    fn bytes(text: &[u8]) -> Input {
        Input::Bytes(text.to_vec())
    }

    #[test]
    fn the_terminal_and_the_choices_set_the_layer_colors_and_symbols() {
        let xterm = [("TERM", "xterm-256color"), ("LANG", "en_US.UTF-8")];
        let found = Remote::new(vars(&xterm), Some((100, 30)), Choices::default());
        assert_eq!(
            (
                found.settings.layer,
                found.settings.color,
                found.settings.ascii
            ),
            (Layer::Hud, Depth::Indexed, false)
        );
        assert_eq!((found.width, found.height), (100, 30));
        let plain = Remote::new(vars(&[]), None, Choices::default());
        assert_eq!(
            (plain.settings.layer, plain.settings.ascii),
            (Layer::Line, true)
        );
        let line = Choices {
            layer: Some(Layer::Line),
            ..Choices::default()
        };
        let typed = Remote::new(vars(&xterm), Some((80, 24)), line);
        assert_eq!(typed.settings.layer, Layer::Edit);
    }

    #[test]
    fn without_a_pty_the_remote_terminal_sends_whole_lines() {
        let terminal = Remote::new(vars(&[]), None, Choices::default());
        let input = [bytes(b"lo"), bytes(b"ok\r\n"), bytes(b"quit")];
        let (ending, text) = session(input, terminal);
        assert_eq!(ending, Ending::Quit);
        assert!(
            text.starts_with("Connected to Test City as anon-test.\n$ "),
            "{text}"
        );
        assert!(text.contains("echo look\n$ "), "{text}");
    }

    #[test]
    fn with_a_pty_the_keys_drive_the_line_editor() {
        let edit = Choices {
            layer: Some(Layer::Edit),
            ..Choices::default()
        };
        let terminal = Remote::new(vars(&[("TERM", "xterm")]), Some((80, 24)), edit);
        let input = [bytes(b"ls\r"), bytes(b"\x1b[A\r"), bytes(b"\x04")];
        let (ending, text) = session(input, terminal);
        assert_eq!(ending, Ending::Quit);
        assert_eq!(text.matches("\r\x1b[Kecho ls\r\n").count(), 2, "{text:?}");
    }

    #[test]
    fn the_hud_fills_the_remote_screen_and_follows_a_resize() {
        let terminal = Remote::new(
            vars(&[("TERM", "xterm")]),
            Some((80, 24)),
            Choices::default(),
        );
        let input = [bytes(b"ls\r"), Input::Resize(100, 30), bytes(b"\x04")];
        let (ending, text) = session(input, terminal);
        assert_eq!(ending, Ending::Quit);
        assert!(text.starts_with("\x1b[?1049h"), "{text:?}");
        assert!(text.contains("echo ls"), "{text:?}");
    }
}
