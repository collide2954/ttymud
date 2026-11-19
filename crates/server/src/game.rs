use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::mpsc::{self, RecvTimeoutError};
use std::time::Instant;

use ttymud_game::{BANNED, Completion, Output, PlayerId, World};
use ttymud_net::PublicKey;
use ttymud_wire::ServerMessage;

use crate::city::now;
use crate::ticker::{TICK, Ticker};

const TAKEN_OVER: &str = "You connected from another place.";

pub(crate) enum Event {
    Join {
        connection: u64,
        key: PublicKey,
        address: IpAddr,
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

pub(crate) type Saves = mpsc::Sender<(String, String)>;

pub(crate) struct Game {
    world: World,
    connections: HashMap<u64, PlayerId>,
    links: HashMap<PlayerId, Link>,
    store: Option<Saves>,
}

struct Link {
    connection: u64,
    key: PublicKey,
    outbox: mpsc::Sender<ServerMessage>,
}

pub(crate) fn run(world: World, inbox: &mpsc::Receiver<Event>, store: Option<Saves>) {
    let mut game = Game::new(world, store);
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
    pub(crate) fn new(world: World, store: Option<Saves>) -> Self {
        Self {
            world,
            connections: HashMap::new(),
            links: HashMap::new(),
            store,
        }
    }

    pub(crate) fn handle(&mut self, event: Event) {
        self.world.set_time(now());
        match event {
            Event::Join {
                connection,
                key,
                address,
                outbox,
            } => self.join(connection, key, address, outbox),
            Event::Command { connection, line } => self.command(connection, &line),
            Event::Complete { connection, text } => self.complete(connection, &text),
            Event::Leave { connection } => self.leave(connection),
        }
        for (player, reason) in self.world.drops() {
            self.close(player, reason);
        }
        self.save();
    }

    fn close(&mut self, player: PlayerId, reason: String) {
        let Some(connection) = self.links.get(&player).map(|link| link.connection) else {
            return;
        };
        self.send(player, ServerMessage::Close(reason));
        self.leave(connection);
    }

    fn save(&mut self) {
        let saves = self.world.saves();
        if let Some(store) = &self.store {
            for save in saves {
                let _ = store.send(save);
            }
        }
    }

    fn join(
        &mut self,
        connection: u64,
        key: PublicKey,
        address: IpAddr,
        outbox: mpsc::Sender<ServerMessage>,
    ) {
        if self.world.banned(&key.to_hex(), Some(address)) {
            let _ = outbox.send(ServerMessage::Close(BANNED.to_owned()));
            return;
        }
        let earlier = self.links.iter().find(|(_, link)| link.key == key);
        if let Some(player) = earlier.map(|(&player, _)| player) {
            self.close(player, TAKEN_OVER.to_owned());
        }
        let (player, outputs) = self.world.connect(&key.to_hex());
        self.world.set_address(player, address);
        let name = self
            .world
            .handle(player)
            .map_or_else(|| name_for(key), str::to_owned);
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
        self.world.set_time(now());
        let outputs = self.world.tick();
        self.deliver(outputs, None);
        self.save();
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
    use std::net::IpAddr;
    use std::sync::mpsc;

    use ttymud_game::{Content, World};
    use ttymud_net::{Keys, PublicKey};
    use ttymud_wire::ServerMessage;

    use super::{Event, Game};
    use crate::port_ostra;

    const LAB: &str = "[city lab]\nname = Lab\nstart = hub\n[host hub]\nname = Hub\ndesc = A hub.\n\
                       [network net]\nname = Net\n[host gate]\nname = Gate\ndesc = A gate.\n\
                       network = net\nroute = hub\n[ice gate/sentry]\ntype = sentry\n";

    fn game() -> Game {
        Game::new(port_ostra(0).unwrap(), None)
    }

    fn key() -> PublicKey {
        Keys::generate().unwrap().public()
    }

    fn anon(key: PublicKey) -> String {
        format!("anon-{}", &key.to_hex()[..8])
    }

    fn join(game: &mut Game, connection: u64, key: PublicKey) -> mpsc::Receiver<ServerMessage> {
        join_from(game, connection, key, IpAddr::from([127, 0, 0, 1]))
    }

    fn join_from(
        game: &mut Game,
        connection: u64,
        key: PublicKey,
        address: IpAddr,
    ) -> mpsc::Receiver<ServerMessage> {
        let (outbox, mailbox) = mpsc::channel();
        game.handle(Event::Join {
            connection,
            key,
            address,
            outbox,
        });
        mailbox
    }

    fn hacker(game: &mut Game, connection: u64, name: &str) -> mpsc::Receiver<ServerMessage> {
        let mailbox = join(game, connection, key());
        command(game, connection, name);
        command(game, connection, "breaker");
        drain(&mailbox);
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
    fn a_new_account_gets_a_welcome_and_chooses_a_handle() {
        let mut game = game();
        let key = key();
        let messages = drain(&join(&mut game, 1, key));
        let welcome = ServerMessage::Welcome {
            city: "Port Ostra".to_owned(),
            name: anon(key),
        };
        assert_eq!(messages[0], welcome);
        assert!(texts(&messages)[0].starts_with("You are new in Port Ostra."));
        let prompt = ServerMessage::Prompt("handle: ".to_owned());
        assert_eq!(messages.last(), Some(&prompt));
    }

    #[test]
    fn a_returning_account_gets_a_welcome_with_its_handle() {
        let mut game = game();
        let key = key();
        let first = join(&mut game, 1, key);
        command(&mut game, 1, "ghost");
        command(&mut game, 1, "ghost");
        drain(&first);
        game.handle(Event::Leave { connection: 1 });
        let messages = drain(&join(&mut game, 2, key));
        let welcome = ServerMessage::Welcome {
            city: "Port Ostra".to_owned(),
            name: "ghost".to_owned(),
        };
        assert_eq!(messages[0], welcome);
        let prompt = ServerMessage::Prompt("ghost@safehouse $ ".to_owned());
        assert_eq!(messages.last(), Some(&prompt));
    }

    #[test]
    fn a_command_gets_its_output_and_a_new_prompt() {
        let mut game = game();
        let mailbox = hacker(&mut game, 1, "ghost");
        command(&mut game, 1, "go night-market");
        let messages = drain(&mailbox);
        assert_eq!(texts(&messages)[0], "The Night Market");
        let prompt = ServerMessage::Prompt("ghost@night-market $ ".to_owned());
        assert_eq!(messages.last(), Some(&prompt));
    }

    #[test]
    fn players_hear_each_other() {
        let mut game = game();
        let first = hacker(&mut game, 1, "ghost");
        let second = hacker(&mut game, 2, "nyx");
        assert_eq!(
            texts(&drain(&first)).last().map(String::as_str),
            Some("nyx connects.")
        );
        drain(&second);
        command(&mut game, 2, "say hi");
        let heard = drain(&first);
        assert_eq!(texts(&heard), ["<nyx> hi"]);
        let prompt = "ghost@relay $ ".to_owned();
        assert_eq!(heard.last(), Some(&ServerMessage::Refresh(prompt)));
    }

    #[test]
    fn leaving_tells_the_others() {
        let mut game = game();
        let first = hacker(&mut game, 1, "ghost");
        let _second = hacker(&mut game, 2, "nyx");
        drain(&first);
        game.handle(Event::Leave { connection: 2 });
        assert_eq!(texts(&drain(&first)), ["nyx disconnects."]);
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

    fn moderated() -> (Game, mpsc::Receiver<ServerMessage>) {
        let moderator = key();
        let mut world = port_ostra(0).unwrap();
        let file = format!("[moderator {}]\n", moderator.to_hex());
        world.restore("moderators.ini", &file).unwrap();
        let mut game = Game::new(world, None);
        let first = join(&mut game, 1, moderator);
        command(&mut game, 1, "ghost");
        command(&mut game, 1, "breaker");
        (game, first)
    }

    #[test]
    fn a_banned_account_or_address_cannot_join() {
        let (mut game, first) = moderated();
        let (banned, address) = (key(), IpAddr::from([203, 0, 113, 5]));
        let second = join_from(&mut game, 2, banned, address);
        command(&mut game, 2, "nyx");
        command(&mut game, 2, "breaker");
        drain(&first);
        drain(&second);
        command(&mut game, 1, "ban nyx ip");
        let closed = ServerMessage::Close("You are banned from this server.".to_owned());
        assert_eq!(drain(&second).last(), Some(&closed));
        let local = IpAddr::from([127, 0, 0, 1]);
        let refused = [closed];
        assert_eq!(drain(&join_from(&mut game, 3, banned, local)), refused);
        assert_eq!(drain(&join_from(&mut game, 4, key(), address)), refused);
        command(&mut game, 1, "unban nyx");
        let again = drain(&join_from(&mut game, 5, banned, address));
        assert!(matches!(again[0], ServerMessage::Welcome { .. }));
    }

    #[test]
    fn a_kick_closes_the_connection_of_the_handle() {
        let (mut game, first) = moderated();
        let second = hacker(&mut game, 2, "nyx");
        drain(&first);
        command(&mut game, 1, "kick nyx");
        let closed = ServerMessage::Close("A moderator kicked you from the server.".to_owned());
        assert_eq!(drain(&second).last(), Some(&closed));
        assert_eq!(texts(&drain(&first)), ["You kick nyx.", "nyx disconnects."]);
        command(&mut game, 2, "look");
        assert_eq!(drain(&second), Vec::<ServerMessage>::new());
    }

    #[test]
    fn a_tick_with_news_sends_the_text_and_a_refresh() {
        let mut game = Game::new(World::new(Content::parse(LAB).unwrap(), 0), None);
        let mailbox = hacker(&mut game, 1, "ghost");
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
        let prompt = "[trace 14% integ 100] ghost@gate $ ".to_owned();
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

    #[test]
    fn changes_go_to_the_store() {
        let (saves, stored) = mpsc::channel();
        let mut game = Game::new(port_ostra(0).unwrap(), Some(saves));
        let key = key();
        let _mailbox = join(&mut game, 1, key);
        command(&mut game, 1, "ghost");
        command(&mut game, 1, "breaker");
        let paths: Vec<String> = stored.try_iter().map(|(path, _)| path).collect();
        assert!(
            paths.contains(&format!("accounts/{}.ini", key.to_hex())),
            "{paths:?}"
        );
    }

    #[test]
    fn events_from_unknown_connections_are_ignored() {
        let mut game = game();
        command(&mut game, 99, "look");
        game.handle(Event::Leave { connection: 99 });
    }
}
