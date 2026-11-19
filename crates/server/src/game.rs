mod account;

use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::mpsc::{self, RecvTimeoutError};
use std::time::Instant;

use ttymud_game::{BANNED, Completion, Output, PlayerId, World};
use ttymud_net::PublicKey;
use ttymud_text::{Message, Status};
use ttymud_wire::ServerMessage;

use crate::city::now;
use crate::identity::Identity;
use crate::ticker::{TICK, Ticker};

const TAKEN_OVER: &str = "You connected from another place.";
const REVOKED: &str = "Your account revoked this key.";
const LINK_HINT: &str =
    "If you have an account on a different device, type link <code> or recover <code>.";

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
    identity: Identity,
    connections: HashMap<u64, PlayerId>,
    links: HashMap<PlayerId, Link>,
    store: Option<Saves>,
}

struct Link {
    connection: u64,
    key: PublicKey,
    account: String,
    address: IpAddr,
    outbox: mpsc::Sender<ServerMessage>,
    status: Option<Status>,
}

pub(crate) fn run(
    world: World,
    identity: Identity,
    inbox: &mpsc::Receiver<Event>,
    store: Option<Saves>,
) {
    let mut game = Game::new(world, identity, store);
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
    pub(crate) fn new(world: World, identity: Identity, store: Option<Saves>) -> Self {
        Self {
            world,
            identity,
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
        let mut saves = self.world.saves();
        saves.extend(self.identity.save());
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
        let Some(account) = self.identity.account_of(&key.to_hex()) else {
            let _ = outbox.send(ServerMessage::Close(REVOKED.to_owned()));
            return;
        };
        if self.world.banned(&account, Some(address)) {
            let _ = outbox.send(ServerMessage::Close(BANNED.to_owned()));
            return;
        }
        let player = self.start(Link {
            connection,
            key,
            account,
            address,
            outbox,
            status: None,
        });
        self.prompt(player);
    }

    fn start(&mut self, link: Link) -> PlayerId {
        let earlier = self
            .links
            .iter()
            .find(|(_, known)| known.account == link.account);
        if let Some(player) = earlier.map(|(&player, _)| player) {
            self.close(player, TAKEN_OVER.to_owned());
        }
        let (player, outputs) = self.world.connect(&link.account);
        self.world.set_address(player, link.address);
        let name = self
            .world
            .handle(player)
            .map_or_else(|| name_for(link.key), str::to_owned);
        let city = self.world.city().to_owned();
        let _ = link.outbox.send(ServerMessage::Welcome { city, name });
        self.connections.insert(link.connection, player);
        self.links.insert(player, link);
        self.deliver(outputs, Some(player));
        if self.world.handle(player).is_none() {
            self.show(player, vec![Message::game(LINK_HINT)]);
        }
        player
    }

    fn command(&mut self, connection: u64, line: &str) {
        let Some(&player) = self.connections.get(&connection) else {
            return;
        };
        if let Some(player) = self.account_command(player, line) {
            self.prompt(player);
            return;
        }
        let named = self.world.handle(player).is_some();
        let outputs = self.world.command(player, line);
        self.deliver(outputs, Some(player));
        if !named && self.world.handle(player).is_some() {
            self.first_recovery(player);
        }
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
        let players: Vec<PlayerId> = self.links.keys().copied().collect();
        for player in players {
            self.sync_status(player);
        }
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

    fn deliver(&mut self, outputs: Vec<Output>, answered: Option<PlayerId>) {
        let mut told = Vec::new();
        for output in outputs {
            if Some(output.player) != answered && !told.contains(&output.player) {
                told.push(output.player);
            }
            self.send(output.player, ServerMessage::Text(output.message));
        }
        for player in told {
            if let Some(prompt) = self.world.prompt(player) {
                self.sync_status(player);
                self.send(player, ServerMessage::Refresh(prompt));
            }
        }
    }

    fn prompt(&mut self, player: PlayerId) {
        if let Some(prompt) = self.world.prompt(player) {
            self.sync_status(player);
            self.send(player, ServerMessage::Prompt(prompt));
        }
    }

    fn sync_status(&mut self, player: PlayerId) {
        let status = self.world.status_of(player);
        let Some(link) = self.links.get_mut(&player) else {
            return;
        };
        if status.is_none() || link.status == status {
            return;
        }
        link.status.clone_from(&status);
        if let Some(status) = status {
            let _ = link.outbox.send(ServerMessage::Status(status));
        }
    }

    fn show(&self, player: PlayerId, shown: Vec<Message>) {
        for message in shown {
            self.send(player, ServerMessage::Text(message));
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
    use ttymud_text::Status;
    use ttymud_wire::ServerMessage;

    use super::{Event, Game};
    use crate::identity::Identity;
    use crate::port_ostra;

    const LAB: &str = "[city lab]\nname = Lab\nstart = hub\n[host hub]\nname = Hub\ndesc = A hub.\n\
                       [network net]\nname = Net\n[host gate]\nname = Gate\ndesc = A gate.\n\
                       network = net\nroute = hub\n[ice gate/sentry]\ntype = sentry\n";

    fn game() -> Game {
        with_world(port_ostra(0).unwrap())
    }

    fn with_world(world: World) -> Game {
        Game::new(world, Identity::default(), None)
    }

    fn found(messages: &[ServerMessage], start: &str) -> Option<String> {
        let mut shown = texts(messages).into_iter();
        shown.find_map(|line| line.strip_prefix(start).map(str::to_owned))
    }

    fn statuses(messages: &[ServerMessage]) -> Vec<Status> {
        let found = messages.iter().filter_map(|message| match message {
            ServerMessage::Status(status) => Some(status.clone()),
            _ => None,
        });
        found.collect()
    }

    #[test]
    fn the_status_goes_out_before_the_prompt_when_it_changes() {
        let mut game = game();
        let mailbox = join(&mut game, 1, key());
        assert_eq!(statuses(&drain(&mailbox)), []);
        command(&mut game, 1, "ghost");
        command(&mut game, 1, "breaker");
        let made: Vec<String> = statuses(&drain(&mailbox))
            .into_iter()
            .map(|status| status.host)
            .collect();
        assert_eq!(made, ["relay"]);
        command(&mut game, 1, "look");
        assert_eq!(statuses(&drain(&mailbox)), []);
        command(&mut game, 1, "go safehouse");
        let moved = drain(&mailbox);
        let hosts: Vec<String> = statuses(&moved)
            .into_iter()
            .map(|status| status.host)
            .collect();
        assert_eq!(hosts, ["safehouse"]);
        assert!(matches!(moved.last(), Some(ServerMessage::Prompt(_))));
    }

    #[test]
    fn a_tick_sends_the_status_when_the_cycles_come_back() {
        let mut game = with_world(World::new(Content::parse(LAB).unwrap(), 0));
        let mailbox = hacker(&mut game, 1, "ghost");
        command(&mut game, 1, "go gate");
        command(&mut game, 1, "scan");
        let spent = statuses(&drain(&mailbox))
            .last()
            .map_or(0, |status| status.cycles);
        game.tick();
        let after: Vec<u32> = statuses(&drain(&mailbox))
            .iter()
            .map(|status| status.cycles)
            .collect();
        assert_eq!(after, [spent + 1]);
    }

    fn welcome(name: &str) -> ServerMessage {
        ServerMessage::Welcome {
            city: "Port Ostra".to_owned(),
            name: name.to_owned(),
        }
    }

    #[test]
    fn a_new_handle_gets_a_recovery_code_that_brings_the_account_back() {
        let mut game = game();
        let first = join(&mut game, 1, key());
        command(&mut game, 1, "ghost");
        command(&mut game, 1, "breaker");
        let code = found(&drain(&first), "Your recovery code: ").unwrap_or_default();
        assert_eq!(code.len(), 39);
        game.handle(Event::Leave { connection: 1 });
        let second = join(&mut game, 2, key());
        assert!(
            texts(&drain(&second)).contains(
                &"If you have an account on a different device, type link <code> or recover <code>."
                    .to_owned()
            )
        );
        command(&mut game, 2, "recover 0000-0000");
        assert_eq!(
            texts(&drain(&second)),
            ["That recovery code does not work."]
        );
        command(&mut game, 2, &format!("recover {}", code.to_uppercase()));
        let messages = drain(&second);
        assert!(messages.contains(&welcome("ghost")));
        let shown = texts(&messages);
        assert_eq!(shown[0], "You recovered your account.");
        assert!(shown.contains(&"Welcome back, ghost.".to_owned()));
        let fresh = found(&messages, "Your recovery code: ").unwrap_or_default();
        assert_ne!(fresh, code);
        game.handle(Event::Leave { connection: 2 });
        let third = join(&mut game, 3, key());
        drain(&third);
        command(&mut game, 3, &format!("recover {code}"));
        assert_eq!(texts(&drain(&third)), ["That recovery code does not work."]);
    }

    #[test]
    fn a_link_code_adds_a_device_and_keys_and_revoke_manage_it() {
        let mut game = game();
        let (old, new) = (key(), key());
        let first = join(&mut game, 1, old);
        command(&mut game, 1, "ghost");
        command(&mut game, 1, "breaker");
        drain(&first);
        command(&mut game, 1, "link");
        let code = found(&drain(&first), "Your link code: ").unwrap_or_default();
        let second = join(&mut game, 2, new);
        drain(&second);
        command(&mut game, 2, "link 1234-5678");
        assert_eq!(
            texts(&drain(&second)),
            ["That link code does not work. Type link on your other device to make a new code."]
        );
        command(&mut game, 2, &format!("link {code}"));
        let messages = drain(&second);
        assert_eq!(
            texts(&messages)[0],
            "This device is now linked to your account."
        );
        assert!(messages.contains(&welcome("ghost")));
        let taken = ServerMessage::Close("You connected from another place.".to_owned());
        assert_eq!(drain(&first).last(), Some(&taken));
        command(&mut game, 2, "keys");
        assert_eq!(
            texts(&drain(&second)),
            [
                "Keys of your account:".to_owned(),
                format!("1. {}", old.fingerprint()),
                format!("2. {} (this device)", new.fingerprint()),
                "Type revoke <number> to remove a key.".to_owned(),
            ]
        );
        let mut said = |line: &str| {
            command(&mut game, 2, line);
            texts(&drain(&second))
        };
        assert_eq!(
            said("revoke 2"),
            ["You cannot revoke the key of this device."]
        );
        assert_eq!(said("revoke 3"), ["No key 3. Type keys to see the keys."]);
        assert_eq!(said("keys now"), ["keys takes no arguments."]);
        assert_eq!(
            said("revoke 1"),
            [format!(
                "You revoke key 1, {}. That device cannot connect now.",
                old.fingerprint()
            )]
        );
        let revoked = ServerMessage::Close("Your account revoked this key.".to_owned());
        assert_eq!(drain(&join(&mut game, 3, old)), [revoked]);
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
        let mut game = with_world(world);
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
        let mut game = with_world(World::new(Content::parse(LAB).unwrap(), 0));
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
        let mut game = Game::new(port_ostra(0).unwrap(), Identity::default(), Some(saves));
        let key = key();
        let _mailbox = join(&mut game, 1, key);
        command(&mut game, 1, "ghost");
        command(&mut game, 1, "breaker");
        let paths: Vec<String> = stored.try_iter().map(|(path, _)| path).collect();
        assert!(
            paths.contains(&format!("accounts/{}.ini", key.to_hex())),
            "{paths:?}"
        );
        assert!(paths.contains(&"keys.ini".to_owned()), "{paths:?}");
    }

    #[test]
    fn events_from_unknown_connections_are_ignored() {
        let mut game = game();
        command(&mut game, 99, "look");
        game.handle(Event::Leave { connection: 99 });
    }
}
