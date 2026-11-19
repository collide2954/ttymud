use std::io;
use std::net::{Shutdown, TcpStream};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use ttymud_net::{Receiver, Sender};
use ttymud_text::clean;
use ttymud_wire::{ClientMessage, ServerMessage};

use crate::{Link, Shown, render};

const DRAIN: Duration = Duration::from_secs(5);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ending {
    Quit,
    Closed,
}

pub(crate) enum Event {
    Line(String),
    InputEnded,
    Server(ServerMessage),
    LinkEnded,
}

enum Typed {
    Nothing,
    Line(String),
    Quit,
}

pub(crate) trait View {
    fn show(&mut self, shown: Shown) -> io::Result<()>;

    fn finish(&mut self) -> io::Result<()> {
        Ok(())
    }
}

pub(crate) fn run(
    link: Link,
    input: impl FnOnce(&mpsc::Sender<Event>) + Send + 'static,
    view: &mut impl View,
) -> io::Result<Ending> {
    let Link {
        receiver,
        mut sender,
        control,
        ..
    } = link;
    let (events, inbox) = mpsc::channel();
    let network = {
        let events = events.clone();
        thread::Builder::new()
            .name("network".to_owned())
            .spawn(move || listen(receiver, &events))?
    };
    thread::Builder::new()
        .name("input".to_owned())
        .spawn(move || input(&events))?;
    let ending = play(&inbox, &mut sender, view).and_then(|ending| view.finish().map(|()| ending));
    let _ = control.shutdown(Shutdown::Both);
    network
        .join()
        .map_err(|_| io::Error::other("the network thread stopped"))?;
    ending
}

pub(crate) fn is_quit(line: &str) -> bool {
    clean(line).trim().eq_ignore_ascii_case("quit")
}

fn listen(mut receiver: Receiver<TcpStream>, events: &mpsc::Sender<Event>) {
    while let Ok(bytes) = receiver.receive() {
        let Ok(message) = ServerMessage::decode(&bytes) else {
            break;
        };
        let close = matches!(message, ServerMessage::Close(_));
        if events.send(Event::Server(message)).is_err() || close {
            return;
        }
    }
    let _ = events.send(Event::LinkEnded);
}

fn play(
    inbox: &mpsc::Receiver<Event>,
    sender: &mut Sender<TcpStream>,
    view: &mut impl View,
) -> io::Result<Ending> {
    let (mut sent, mut prompts, mut draining) = (0_usize, 0_usize, false);
    loop {
        let next = if draining {
            inbox.recv_timeout(DRAIN).ok()
        } else {
            inbox.recv().ok()
        };
        let Some(event) = next else {
            return Ok(Ending::Quit);
        };
        let typed = match event {
            Event::Server(message) => {
                let close = matches!(message, ServerMessage::Close(_));
                prompts += usize::from(matches!(message, ServerMessage::Prompt(_)));
                view.show(render(&message))?;
                if close {
                    return Ok(Ending::Closed);
                }
                Typed::Nothing
            }
            Event::LinkEnded => return Ok(Ending::Closed),
            Event::InputEnded => Typed::Quit,
            Event::Line(line) => Typed::Line(line),
        };
        match typed {
            Typed::Line(line) if !draining && !is_quit(&line) => {
                if sender.send(&ClientMessage::Command(line).encode()).is_err() {
                    return Ok(Ending::Closed);
                }
                sent += 1;
            }
            Typed::Line(_) | Typed::Quit => draining = true,
            Typed::Nothing => {}
        }
        if draining && prompts > sent {
            return Ok(Ending::Quit);
        }
    }
}
