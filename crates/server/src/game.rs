use std::collections::HashMap;
use std::sync::mpsc::{self, RecvTimeoutError};
use std::time::Instant;

use ttymud_game::{Completion, Output, PlayerId, World};
use ttymud_net::PublicKey;
use ttymud_wire::ServerMessage;

use crate::ticker::{TICK, Ticker};

const TAKEN_OVER: &str = "You connected from another place.";

pub(crate) enum Event {
    Join {
        connection: u64,
        key: PublicKey,
        outbox: mpsc::Sender<ServerMessage>,
    },
    Command {
        connection: u64,
        line: String,
    },
    Complete {
        connection: u64,
        text: String,
    },
    Leave {
        connection: u64,
    },
}

pub(crate) struct Game {
    world: World,
    connections: HashMap<u64, PlayerId>,
    links: HashMap<PlayerId, Link>,
}

struct Link {
    connection: u64,
    key: PublicKey,
    outbox: mpsc::Sender<ServerMessage>,
}

pub(crate) fn run(world: World, inbox: &mpsc::Receiver<Event>) {
    let mut game = Game::new(world);
    let mut ticker = Ticker::new(TICK, Instant::now());
    loop {
        match inbox.recv_timeout(ticker.left(Instant::now())) {
            Ok(event) => game.handle(event),
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return,
        }
        if ticker.due(Instant::now()) {
            game.tick();
        }
    }
}

pub(crate) fn name_for(key: PublicKey) -> String {
    format!("anon-{}", &key.to_hex()[..8])
}

impl Game {
    pub(crate) fn new(world: World) -> Self {
        Self {
            world,
            connections: HashMap::new(),
            links: HashMap::new(),
        }
    }

    pub(crate) fn handle(&mut self, event: Event) {
        match event {
            Event::Join {
                connection,
                key,
                outbox,
            } => self.join(connection, key, outbox),
            Event::Command { connection, line } => self.command(connection, &line),
            Event::Complete { connection, text } => self.complete(connection, &text),
            Event::Leave { connection } => self.leave(connection),
        }
    }

    fn join(&mut self, connection: u64, key: PublicKey, outbox: mpsc::Sender<ServerMessage>) {
        let earlier = self
            .links
            .iter()
            .find(|(_, link)| link.key == key)
            .map(|(&player, link)| (player, link.connection));
        if let Some((player, old)) = earlier {
            self.send(player, ServerMessage::Close(TAKEN_OVER.to_owned()));
            self.leave(old);
        }
        let name = name_for(key);
        let (player, outputs) = self.world.join(&name);
        let city = self.world.city().to_owned();
        let _ = outbox.send(ServerMessage::Welcome { city, name });
        self.connections.insert(connection, player);
        self.links.insert(
            player,
            Link {
                connection,
                key,
                outbox,
            },
        );
        self.deliver(outputs, Some(player));
        self.prompt(player);
    }

    fn command(&mut self, connection: u64, line: &str) {
        let Some(&player) = self.connections.get(&connection) else {
            return;
        };
        let outputs = self.world.command(player, line);
        self.deliver(outputs, Some(player));
        self.prompt(player);
    }

    fn complete(&self, connection: u64, text: &str) {
        let Some(&player) = self.connections.get(&connection) else {
            return;
        };
        let Completion { word, names } = self.world.complete(player, text);
        self.send(player, ServerMessage::Candidates { word, names });
    }

    fn tick(&mut self) {
        let outputs = self.world.tick();
        self.deliver(outputs, None);
    }

    fn leave(&mut self, connection: u64) {
        let Some(player) = self.connections.remove(&connection) else {
            return;
        };
        self.links.remove(&player);
        let outputs = self.world.leave(player);
        self.deliver(outputs, None);
    }

    fn deliver(&self, outputs: Vec<Output>, answered: Option<PlayerId>) {
        let mut told = Vec::new();
        for output in outputs {
            if Some(output.player) != answered && !told.contains(&output.player) {
                told.push(output.player);
            }
            self.send(output.player, ServerMessage::Text(output.message));
        }
        for player in told {
            if let Some(prompt) = self.world.prompt(player) {
                self.send(player, ServerMessage::Refresh(prompt));
            }
        }
    }

    fn prompt(&self, player: PlayerId) {
        if let Some(prompt) = self.world.prompt(player) {
            self.send(player, ServerMessage::Prompt(prompt));
        }
    }

    fn send(&self, player: PlayerId, message: ServerMessage) {
        if let Some(link) = self.links.get(&player) {
            let _ = link.outbox.send(message);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc;

    use ttymud_game::{Content, World};
    use ttymud_net::{Keys, PublicKey};
    use ttymud_wire::ServerMessage;

    use super::{Event, Game};

    const LAB: &str = "[city lab]\nname = Lab\nstart = hub\n[host hub]\nname = Hub\ndesc = A hub.\n\
                       [network net]\nname = Net\n[host gate]\nname = Gate\ndesc = A gate.\n\
                       network = net\nroute = hub\n[ice gate/sentry]\ntype = sentry\n";

    #[test]
    fn a_tick_with_news_sends_the_text_and_a_refresh() {
        let mut game = Game::new(World::new(Content::parse(LAB).unwrap(), 0));
        let key = key();
        let mailbox = join(&mut game, 1, key);
        command(&mut game, 1, "go gate");
        command(&mut game, 1, "scan");
        drain(&mailbox);
        for _ in 0..3 {
            game.tick();
        }
        let messages = drain(&mailbox);
        assert_eq!(
            texts(&messages),
            ["sentry sends an ALERT. Trace: 4% -> 14%."]
        );
        let prompt = format!("[trace 14% integ 100] {}@gate $ ", name(key));
        assert_eq!(messages.last(), Some(&ServerMessage::Refresh(prompt)));
    }

    #[test]
    fn a_quiet_tick_sends_nothing() {
        let mut game = game();
        let mailbox = join(&mut game, 1, key());
        drain(&mailbox);
        game.tick();
        assert_eq!(drain(&mailbox), Vec::<ServerMessage>::new());
    }
    use crate::port_ostra;

    fn game() -> Game {
        Game::new(port_ostra(0).unwrap())
    }

    fn key() -> PublicKey {
        Keys::generate().unwrap().public()
    }

    fn name(key: PublicKey) -> String {
        format!("anon-{}", &key.to_hex()[..8])
    }

    fn join(game: &mut Game, connection: u64, key: PublicKey) -> mpsc::Receiver<ServerMessage> {
        let (outbox, mailbox) = mpsc::channel();
        game.handle(Event::Join {
            connection,
            key,
            outbox,
        });
        mailbox
    }

    fn command(game: &mut Game, connection: u64, line: &str) {
        game.handle(Event::Command {
            connection,
            line: line.to_owned(),
        });
    }

    fn drain(mailbox: &mpsc::Receiver<ServerMessage>) -> Vec<ServerMessage> {
        mailbox.try_iter().collect()
    }

    fn texts(messages: &[ServerMessage]) -> Vec<String> {
        messages
            .iter()
            .filter_map(|message| match message {
                ServerMessage::Text(text) => Some(text.plain()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn a_new_player_gets_a_welcome_the_scene_and_a_prompt() {
        let mut game = game();
        let key = key();
        let messages = drain(&join(&mut game, 1, key));
        let welcome = ServerMessage::Welcome {
            city: "Port Ostra".to_owned(),
            name: name(key),
        };
        assert_eq!(messages[0], welcome);
        assert_eq!(texts(&messages)[0], "The Relay");
        let prompt = ServerMessage::Prompt(format!("{}@relay $ ", name(key)));
        assert_eq!(messages.last(), Some(&prompt));
    }

    #[test]
    fn a_command_gets_its_output_and_a_new_prompt() {
        let mut game = game();
        let key = key();
        let mailbox = join(&mut game, 1, key);
        drain(&mailbox);
        command(&mut game, 1, "go night-market");
        let messages = drain(&mailbox);
        assert_eq!(texts(&messages)[0], "The Night Market");
        let prompt = ServerMessage::Prompt(format!("{}@night-market $ ", name(key)));
        assert_eq!(messages.last(), Some(&prompt));
    }

    #[test]
    fn players_hear_each_other() {
        let mut game = game();
        let (ghost, nyx) = (key(), key());
        let first = join(&mut game, 1, ghost);
        let second = join(&mut game, 2, nyx);
        assert_eq!(
            texts(&drain(&first)).last(),
            Some(&format!("{} connects.", name(nyx)))
        );
        drain(&second);
        command(&mut game, 2, "say hi");
        let heard = drain(&first);
        assert_eq!(texts(&heard), [format!("<{}> hi", name(nyx))]);
        let prompt = format!("{}@relay $ ", name(ghost));
        assert_eq!(heard.last(), Some(&ServerMessage::Refresh(prompt)));
    }

    #[test]
    fn leaving_tells_the_others() {
        let mut game = game();
        let (ghost, nyx) = (key(), key());
        let first = join(&mut game, 1, ghost);
        let _second = join(&mut game, 2, nyx);
        drain(&first);
        game.handle(Event::Leave { connection: 2 });
        assert_eq!(
            texts(&drain(&first)),
            [format!("{} disconnects.", name(nyx))]
        );
    }

    #[test]
    fn a_second_login_with_the_same_key_takes_over() {
        let mut game = game();
        let key = key();
        let first = join(&mut game, 1, key);
        drain(&first);
        let second = join(&mut game, 2, key);
        let closed = ServerMessage::Close("You connected from another place.".to_owned());
        assert_eq!(drain(&first).last(), Some(&closed));
        assert!(matches!(drain(&second)[0], ServerMessage::Welcome { .. }));
        command(&mut game, 1, "look");
        assert_eq!(drain(&first), Vec::<ServerMessage>::new());
    }

    #[test]
    fn events_from_unknown_connections_are_ignored() {
        let mut game = game();
        command(&mut game, 99, "look");
        game.handle(Event::Leave { connection: 99 });
    }
}
