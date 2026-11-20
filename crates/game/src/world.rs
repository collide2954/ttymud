mod access;
mod account;
mod alert;
mod bans;
mod builds;
mod burns;
mod channels;
mod chat;
mod complete;
mod contract;
mod crews;
mod files;
mod grep;
mod heat;
mod heists;
mod help;
mod ice;
mod ignore;
mod instance;
mod jobs;
mod market;
mod moderation;
mod movement;
mod news;
mod presence;
mod programs;
mod ram;
mod reports;
mod rig;
mod roles;
mod rumors;
mod run;
mod save;
mod shop;
mod soc;
mod software;
mod stash;
mod tell;
mod topics;
mod vault;

use std::collections::{BTreeMap, BTreeSet};
use std::net::IpAddr;

use ttymud_text::{Kind, Message, Style, sanitize};

use crate::commands::{self, Command, Item, Verb};
use crate::content::{Content, CorpDef, File, IceDef, NetworkDef, TemplateDef};
use crate::contract::Event;
use crate::heat::Security;
use crate::kits::Kit;
use crate::matching::{self, Found};
use crate::parse::{self, ParseError, Stage};
use crate::random::Random;
use crate::target::{Level, Vault};

use account::{Account, Phase, Social};
use alert::CorpState;
pub use bans::BANNED;
use burns::Burn;
use channels::Channel;
pub use chat::is_chat;
pub use complete::Completion;
use contract::{Contract, Job};
use crews::Crews;
use instance::{Instance, InstanceId};
use jobs::Fixer;
use market::Market;
use moderation::Moderation;
use news::News;
use rig::Rig;
use run::Run;
use shop::Shop;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PlayerId(u64);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Output {
    pub player: PlayerId,
    pub message: Message,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct HostId(usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct NetworkId(usize);

#[derive(Debug)]
struct Network {
    name: String,
    security: Security,
    entries: Vec<HostId>,
    keys: Vec<(String, Level)>,
    corp: Option<usize>,
    vault: Option<Vault>,
}

impl From<NetworkDef> for Network {
    fn from(def: NetworkDef) -> Self {
        Self {
            name: def.name,
            security: def.security,
            entries: Vec::new(),
            keys: Vec::new(),
            corp: None,
            vault: None,
        }
    }
}

#[derive(Debug)]
struct Host {
    id: String,
    name: String,
    description: String,
    network: Option<NetworkId>,
    routes: Vec<HostId>,
    files: Vec<File>,
    ice: Vec<IceDef>,
    security: Option<Security>,
    software: Option<String>,
}

#[derive(Debug)]
struct Player {
    name: String,
    chain: Vec<HostId>,
    rig: Rig,
    know: Vec<Verb>,
    blueprints: Vec<(Verb, u32)>,
    run: Option<Run>,
    contract: Option<Job>,
    account: Option<String>,
    phase: Phase,
    kit: Option<&'static Kit>,
    heat: u32,
    cooled: u64,
    credits: u32,
    sold: Vec<String>,
    rep: BTreeMap<String, u32>,
    done: Vec<(u64, String, u64)>,
    channels: Vec<String>,
    reply_to: Option<String>,
    last_chat: Option<(String, u64)>,
    social: Social,
    address: Option<IpAddr>,
    stash: Vec<File>,
}

impl Player {
    fn new(name: &str, at: HostId) -> Self {
        Self {
            name: name.to_owned(),
            chain: vec![at],
            heat: 0,
            cooled: 0,
            credits: 0,
            sold: Vec::new(),
            rep: BTreeMap::new(),
            done: Vec::new(),
            channels: Vec::new(),
            reply_to: None,
            last_chat: None,
            social: Social::default(),
            address: None,
            stash: Vec::new(),
            rig: Rig::new(),
            know: commands::programs()
                .map(|(command, _)| command.verb)
                .collect(),
            blueprints: Vec::new(),
            run: None,
            contract: None,
            account: None,
            phase: Phase::Playing,
            kit: None,
        }
    }

    fn end_contract(&mut self) {
        self.contract = None;
        self.sold.clear();
    }

    fn key(&self) -> String {
        self.account
            .clone()
            .unwrap_or_else(|| format!("@{}", self.name))
    }
}

#[derive(Debug)]
pub struct World {
    city: String,
    city_id: String,
    seed: u64,
    state_dirty: bool,
    networks: Vec<Network>,
    hosts: Vec<Host>,
    start: HostId,
    home: Option<HostId>,
    burns_host: Option<HostId>,
    contracts: Vec<Contract>,
    offer: Option<usize>,
    shops: Vec<Shop>,
    market: Market,
    corps: Vec<CorpDef>,
    corp_states: Vec<CorpState>,
    templates: Vec<TemplateDef>,
    fixers: Vec<Fixer>,
    channels: Vec<Channel>,
    postings: BTreeMap<(u64, usize, u64), usize>,
    players: BTreeMap<PlayerId, Player>,
    accounts: BTreeMap<String, Account>,
    moderation: Moderation,
    dirty: BTreeSet<String>,
    burns: Vec<Burn>,
    burns_dirty: bool,
    news: News,
    crews: Crews,
    instances: BTreeMap<InstanceId, Instance>,
    next_instance: u64,
    now: u64,
    next_player: u64,
    outbox: Vec<Output>,
    random: Random,
}

#[derive(Default)]
struct Step {
    shown: Vec<Message>,
    items: Vec<Item>,
}

impl Step {
    fn show(shown: Vec<Message>) -> Self {
        Self {
            shown,
            items: Vec::new(),
        }
    }
}

impl World {
    #[must_use]
    pub fn new(content: Content, seed: u64) -> Self {
        let ids: Vec<String> = content.hosts.iter().map(|host| host.id.clone()).collect();
        let find = |id: &str| HostId(ids.iter().position(|known| known == id).unwrap_or(0));
        let network_ids: Vec<&str> = content
            .networks
            .iter()
            .map(|network| network.id.as_str())
            .collect();
        let network = |id: &str| {
            network_ids
                .iter()
                .position(|&known| known == id)
                .map(NetworkId)
        };
        let mut hosts = Vec::with_capacity(content.hosts.len());
        let mut links = Vec::new();
        for (index, def) in content.hosts.into_iter().enumerate() {
            links.extend(
                def.routes
                    .iter()
                    .map(|target| (HostId(index), find(target))),
            );
            hosts.push(Host {
                id: def.id,
                name: def.name,
                description: def.description,
                network: def.network.as_deref().and_then(network),
                routes: Vec::new(),
                files: def.files,
                ice: def.ice,
                security: None,
                software: None,
            });
        }
        for (from, to) in links {
            link(&mut hosts, from, to);
            link(&mut hosts, to, from);
        }
        let offer = content
            .contract
            .as_deref()
            .and_then(|id| content.contracts.iter().position(|def| def.id == id));
        let fixers = jobs::fixers(content.fixers, &content.corps, find);
        let contracts = content
            .contracts
            .into_iter()
            .map(|def| Contract::hand_made(def, &fixers, network))
            .collect();
        let networks = content.networks.into_iter().map(Network::from).collect();
        let shops = shop::shops(content.shops, find);
        let corp_states = vec![CorpState::default(); content.corps.len()];
        let mut world = Self {
            city: content.city,
            city_id: content.id,
            seed,
            state_dirty: true,
            start: find(&content.start),
            home: content.home.as_deref().map(find),
            burns_host: content.burns.as_deref().map(find),
            contracts,
            offer,
            shops,
            market: Market::default(),
            corps: content.corps,
            corp_states,
            templates: content.templates,
            fixers,
            channels: content.channels.into_iter().map(Channel::from).collect(),
            postings: BTreeMap::new(),
            networks,
            hosts,
            players: BTreeMap::new(),
            accounts: BTreeMap::new(),
            moderation: Moderation::default(),
            dirty: BTreeSet::new(),
            burns: Vec::new(),
            burns_dirty: false,
            news: News {
                host: content.news.as_deref().map(find),
                ..News::default()
            },
            crews: Crews::default(),
            instances: BTreeMap::new(),
            next_instance: 0,
            now: 0,
            next_player: 0,
            outbox: Vec::new(),
            random: Random::new(seed),
        };
        world.post_burns();
        world.post_news();
        world
    }

    pub fn join(&mut self, name: &str) -> (PlayerId, Vec<Output>) {
        let mut state = Player::new(name, self.start);
        state.contract = self.offer.map(Job::new);
        state.channels = self.auto_channels();
        let player = self.add(state);
        self.notify(self.start, player, &presence::named(name, " connects."));
        let mut shown = self.scene(self.start, player);
        shown.extend(self.offer(player));
        (player, self.deliver(player, shown))
    }

    fn add(&mut self, state: Player) -> PlayerId {
        let player = PlayerId(self.next_player);
        self.next_player += 1;
        self.players.insert(player, state);
        player
    }

    pub fn command(&mut self, player: PlayerId, line: &str) -> Vec<Output> {
        self.touch(player);
        let Some(state) = self.players.get(&player) else {
            return Vec::new();
        };
        let line = sanitize(line);
        if state.phase != Phase::Playing {
            let shown = self.create(player, line.trim());
            return self.deliver(player, shown);
        }
        if let Some(chat) = chat::parse(&line) {
            return self.chat(player, chat);
        }
        let messages = match parse::pipeline(&line) {
            Ok(stages) => self.run(player, &stages),
            Err(error) => vec![parse_error(error)],
        };
        self.deliver(player, messages)
    }

    pub fn tick(&mut self) -> Vec<Output> {
        self.watch_corps();
        let players: Vec<PlayerId> = self.players.keys().copied().collect();
        let mut outputs = Vec::new();
        for &player in &players {
            if let Some(state) = self.players.get_mut(&player) {
                state.rig.tick();
            }
            outputs.extend(address(player, self.tick_run(player)));
        }
        let copies: Vec<InstanceId> = self.instances.keys().copied().collect();
        for id in copies {
            outputs.extend(self.tick_ice(id));
            outputs.extend(self.tick_soc(id));
            outputs.extend(self.tick_vault(id));
        }
        for &player in &players {
            let mut shown: Vec<Message> = self.hint(player).into_iter().collect();
            if let Some((before, after)) = self.cool(player) {
                shown.push(Message::game(format!(
                    "Your heat cools down: {before} -> {after}."
                )));
            }
            outputs.extend(address(player, shown));
        }
        let told: BTreeSet<PlayerId> = outputs.iter().map(|output| output.player).collect();
        for player in told {
            self.touch(player);
        }
        outputs.append(&mut self.outbox);
        outputs
    }

    pub fn leave(&mut self, player: PlayerId) -> Vec<Output> {
        if self.running(player) {
            let _ = self.jackout(player);
            self.outbox.clear();
        }
        self.keep(player);
        self.touch(player);
        let Some(state) = self.players.remove(&player) else {
            return Vec::new();
        };
        if let (Phase::Playing, Some(&host)) = (&state.phase, state.chain.last()) {
            self.notify(host, player, &presence::named(&state.name, " disconnects."));
        }
        std::mem::take(&mut self.outbox)
    }

    fn deliver(&mut self, player: PlayerId, messages: Vec<Message>) -> Vec<Output> {
        let mut outputs = address(player, messages);
        outputs.append(&mut self.outbox);
        outputs
    }

    #[must_use]
    pub fn city(&self) -> &str {
        &self.city
    }

    #[must_use]
    pub fn prompt(&self, player: PlayerId) -> Option<String> {
        let state = self.players.get(&player)?;
        match state.phase {
            Phase::Naming => return Some("handle: ".to_owned()),
            Phase::Kit(_) => return Some("kit: ".to_owned()),
            Phase::Playing => {}
        }
        let here = state.chain.last()?;
        let place = format!("{}@{} $ ", state.name, self.hosts[here.0].id);
        Some(match &state.run {
            Some(run) => {
                let integrity = state.rig.integrity;
                format!("[trace {}% integ {integrity}] {place}", run.trace)
            }
            None => place,
        })
    }

    fn run(&mut self, player: PlayerId, stages: &[Stage]) -> Vec<Message> {
        let mut found = Vec::with_capacity(stages.len());
        for stage in stages {
            let Some(command) = commands::find(&stage.name) else {
                return vec![unknown(&stage.name)];
            };
            found.push(command);
        }
        if let Err(problem) = commands::check(&found) {
            return vec![Message::error(problem)];
        }
        if let Err(message) = self.ready(player, &found) {
            return vec![message];
        }
        let mut ran = Vec::new();
        let mut input = None;
        let mut shown = Vec::new();
        for (command, stage) in found.iter().zip(stages) {
            match self.execute(player, command, &stage.args, input.take()) {
                Ok(step) => {
                    let rig = self.players.get(&player).map(|state| &state.rig);
                    ran.extend(rig.and_then(|rig| rig.program(command.verb)));
                    if let Some(event) = Event::of(command.verb) {
                        self.advance(player, event);
                    }
                    shown = step.shown;
                    input = Some(step.items);
                }
                Err(message) => {
                    shown = vec![message];
                    break;
                }
            }
        }
        shown.extend(self.charge(player, ran));
        shown
    }

    fn execute(
        &mut self,
        player: PlayerId,
        command: &Command,
        args: &[String],
        input: Option<Vec<Item>>,
    ) -> Result<Step, Message> {
        if !command.verb.takes_args() {
            no_args(command, args)?;
        }
        let here = self.here(player);
        match command.verb {
            Verb::Look => Ok(Step {
                shown: self.scene(here, player),
                items: self.items(here, player),
            }),
            Verb::List => Ok(self.list(player)),
            Verb::Go => self.go(player, args, input),
            Verb::Back => self.back(player),
            Verb::Read => self.read(player, args, input),
            Verb::Download => self.download(player, args, input),
            Verb::Inventory => Ok(self.inventory(player)),
            Verb::Status => Ok(self.status(player)),
            Verb::Jackout => self.jackout(player),
            Verb::Repair => self.repair(player),
            Verb::Contract => self.contract(player, args),
            Verb::Channels => Ok(self.list_channels(player)),
            Verb::Join => self.join_channel(player, args),
            Verb::Leave => self.leave_channel(player, args),
            Verb::Jobs => self.jobs(player),
            Verb::Accept => self.accept(player, args),
            Verb::Rumors => self.rumors(player),
            Verb::Shop => self.shop(player),
            Verb::Buy => self.buy(player, args),
            Verb::Sell => self.sell(player, args, input),
            Verb::Load => self.load(player, args),
            Verb::Unload => self.unload(player, args),
            Verb::Compile => self.compile(player, args),
            Verb::Stash => self.stash(player, args),
            Verb::Unstash => self.unstash(player, args),
            Verb::Scan => self.scan(player),
            Verb::Probe => self.probe(player, args, input),
            Verb::Crack => self.crack(player, args, input),
            Verb::Scrub => self.scrub(player, args),
            Verb::Patch => self.patch(player),
            Verb::Firewall => self.firewall(player),
            Verb::Decoy => Ok(self.decoy(player)),
            Verb::Mask => Ok(self.mask(player)),
            Verb::Proxy => self.proxy(player),
            Verb::Pretext => self.pretext(player, args, input),
            Verb::Exploit => self.exploit(player),
            Verb::Grep => grep::grep(args, input.unwrap_or_default()),
            Verb::Say => Err(Message::error("Usage: say <text>.")),
            Verb::Tell => Err(Message::error("Usage: tell <handle> <text>.")),
            Verb::Reply => Err(Message::error("Usage: reply <text>.")),
            Verb::Who => Ok(self.who()),
            Verb::News => Ok(self.read_news()),
            Verb::Crew => self.crew(player, args),
            Verb::Ignore => self.ignore(player, args),
            Verb::Unignore => self.unignore(player, args),
            Verb::Block => self.block(player, args),
            Verb::Unblock => self.unblock(player, args),
            Verb::Report => self.report(player, args),
            Verb::Recovery | Verb::Link | Verb::Keys | Verb::Revoke => {
                Err(Message::error("This command works only on a server."))
            }
            Verb::Mute => self.mute(player, args),
            Verb::Unmute => self.unmute(player, args),
            Verb::Kick => self.kick(player, args),
            Verb::Ban => self.ban(player, args),
            Verb::Unban => self.unban(player, args),
            Verb::Bans => self.bans(player),
            Verb::Reports => self.reports(player, args),
            Verb::Resolve => self.resolve(player, args),
            Verb::Help => help::help(args, self.is_moderator(player)),
            Verb::Available => Ok(self.available(player)),
        }
    }

    fn list(&self, player: PlayerId) -> Step {
        let items = self.items(self.here(player), player);
        let shown = listing(&items)
            .into_iter()
            .zip(&items)
            .map(|(line, item)| match item {
                Item::File(file) => match self.needs(player, file) {
                    Some(label) => line.text(label),
                    None => line,
                },
                Item::Route(_) | Item::Ice { .. } | Item::Line(_) => line,
            })
            .collect();
        Step { shown, items }
    }

    fn here(&self, player: PlayerId) -> HostId {
        self.players
            .get(&player)
            .and_then(|player| player.chain.last().copied())
            .unwrap_or(self.start)
    }

    fn routes(&self, id: HostId, player: PlayerId) -> Vec<HostId> {
        let mut routes = self.hosts[id.0].routes.clone();
        if self.home == Some(id) {
            routes.extend(self.job_entries(player));
        }
        let bridges = self
            .instance(player)
            .into_iter()
            .flat_map(|instance| &instance.bridges);
        for &(from, to) in bridges {
            if from == id {
                routes.push(to);
            } else if to == id {
                routes.push(from);
            }
        }
        routes
    }

    fn items(&self, id: HostId, player: PlayerId) -> Vec<Item> {
        let routes = self
            .routes(id, player)
            .into_iter()
            .map(|route| Item::Route(self.hosts[route.0].id.clone()));
        let files = self.hosts[id.0].files.iter().cloned().map(Item::File);
        routes.chain(files).collect()
    }

    fn route_names(&self, id: HostId, player: PlayerId) -> Vec<&str> {
        self.routes(id, player)
            .into_iter()
            .map(|route| self.hosts[route.0].id.as_str())
            .collect()
    }

    fn scene(&self, id: HostId, viewer: PlayerId) -> Vec<Message> {
        let host = &self.hosts[id.0];
        let routes = self.route_names(id, viewer);
        let mut scene = vec![
            Message::new(Kind::Game).span(Style::Title, &host.name),
            Message::game(&host.description),
            listed(Message::game("Routes: "), Style::Host, routes),
        ];
        if !host.files.is_empty() {
            let mut line = Message::game("Files: ");
            for (index, file) in host.files.iter().enumerate() {
                if index > 0 {
                    line = line.text(", ");
                }
                line = line.span(Style::File, &file.name);
                if let Some(label) = self.needs(viewer, file) {
                    line = line.text(label);
                }
            }
            scene.push(line);
        }
        if self.is_shop(id) {
            let shop = Message::game("There is a shop here. Type ")
                .span(Style::Command, "shop")
                .text(" to see what it sells.");
            scene.push(shop);
        }
        scene.extend(self.fixer_line(id));
        scene.extend(self.ice_line(id, viewer));
        let present: Vec<&str> = self.others(id, viewer).map(|(_, name)| name).collect();
        if !present.is_empty() {
            scene.push(listed(Message::game("Here: "), Style::Player, present));
        }
        scene
    }
}

fn no_args(command: &Command, args: &[String]) -> Result<(), Message> {
    if args.is_empty() {
        Ok(())
    } else {
        let name = command.name();
        Err(Message::error(format!("{name} takes no arguments.")))
    }
}

fn minute_word(count: u64) -> &'static str {
    if count == 1 { "minute" } else { "minutes" }
}

fn lines(shown: Vec<Message>) -> Step {
    let items = shown
        .iter()
        .map(|message| Item::Line(message.plain()))
        .collect();
    Step { shown, items }
}

fn listing(items: &[Item]) -> Vec<Message> {
    items
        .iter()
        .map(|item| match item {
            Item::Route(id) => Message::new(Kind::Game).span(Style::Host, format!("{id}/")),
            Item::File(file) => Message::new(Kind::Game).span(Style::File, &file.name),
            Item::Ice { name, .. } => Message::new(Kind::Game).span(Style::Ice, name),
            Item::Line(text) => Message::game(text),
        })
        .collect()
}

fn listed<'a>(
    mut message: Message,
    style: Style,
    names: impl IntoIterator<Item = &'a str>,
) -> Message {
    let mut empty = true;
    for name in names {
        if !empty {
            message = message.text(", ");
        }
        message = message.span(style, name);
        empty = false;
    }
    if empty { message.text("none") } else { message }
}

#[derive(Debug, Clone, Copy)]
enum Thing {
    Route,
    File,
    Ice,
    Program,
    Ware,
    Channel,
    Handle,
    Member,
}

impl Thing {
    fn style(self) -> Style {
        match self {
            Self::Route => Style::Host,
            Self::File => Style::File,
            Self::Ice => Style::Ice,
            Self::Program | Self::Ware | Self::Channel => Style::Command,
            Self::Handle | Self::Member => Style::Player,
        }
    }

    fn missing(self, word: &str) -> String {
        match self {
            Self::Route => format!("No route to {word}"),
            Self::File => format!("No file named {word}"),
            Self::Ice => format!("No ICE named {word}"),
            Self::Program => format!("No program named {word}"),
            Self::Ware => format!("The shop does not sell {word}"),
            Self::Channel => format!("No channel named {word}"),
            Self::Handle => format!("No handle online named {word}"),
            Self::Member => format!("No crewmate named {word}"),
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Route => "Routes",
            Self::File => "Files",
            Self::Ice => "ICE",
            Self::Program => "Programs",
            Self::Ware => "For sale",
            Self::Channel => "Channels",
            Self::Handle => "Online",
            Self::Member => "Crew",
        }
    }
}

fn choose<'a>(thing: Thing, word: &str, names: &[&'a str]) -> Result<&'a str, Message> {
    match matching::resolve(word, names) {
        Found::One(name) => Ok(name),
        Found::Many(matches) => {
            let options = either(&matches);
            Err(Message::error(format!("{word} could mean {options}.")))
        }
        Found::Nothing(Some(near)) => {
            let missing = thing.missing(word);
            Err(Message::error(format!("{missing}. Did you mean "))
                .span(thing.style(), near)
                .text("?"))
        }
        Found::Nothing(None) => {
            let (missing, label) = (thing.missing(word), thing.label());
            let message = Message::error(format!("{missing}. {label}: "));
            Err(listed(message, thing.style(), names.iter().copied()))
        }
    }
}

fn usage(text: &str, thing: Thing, names: &[&str]) -> Message {
    let label = thing.label();
    let message = Message::error(format!("Usage: {text}. {label}: "));
    listed(message, thing.style(), names.iter().copied())
}

fn either(names: &[&str]) -> String {
    match names {
        [rest @ .., last] if !rest.is_empty() => format!("{} or {last}", rest.join(", ")),
        _ => names.join(""),
    }
}

fn unknown(name: &str) -> Message {
    let message = Message::error(format!("Unknown command: {name}."));
    hint(message, name, commands::names())
}

fn hint<'a>(
    message: Message,
    word: &str,
    candidates: impl IntoIterator<Item = &'a str>,
) -> Message {
    match matching::closest(&word.to_lowercase(), candidates) {
        Some(near) => message
            .text(" Did you mean ")
            .span(Style::Command, near)
            .text("?"),
        None => message
            .text(" Type ")
            .span(Style::Command, "help")
            .text(" to see the commands."),
    }
}

fn parse_error(error: ParseError) -> Message {
    Message::error(match error {
        ParseError::OpenQuote => "A quote is not closed.",
        ParseError::EmptyStage => "Each side of | needs a command.",
    })
}

fn link(hosts: &mut [Host], from: HostId, to: HostId) {
    let routes = &mut hosts[from.0].routes;
    if !routes.contains(&to) {
        routes.push(to);
    }
}

fn address(player: PlayerId, messages: Vec<Message>) -> Vec<Output> {
    messages
        .into_iter()
        .map(|message| Output { player, message })
        .collect()
}

#[cfg(test)]
mod tests {
    use ttymud_text::{Kind, Style};

    use super::PlayerId;
    use crate::testing::{joined, plain, run, walk, world};

    #[test]
    fn joining_shows_the_start_host() {
        let (_, outputs) = world().join("ghost");
        assert_eq!(
            plain(&outputs),
            [
                "The Relay",
                "A crowded public node.",
                "Routes: market, back-room, bazaar",
                "Files: motd.txt, rules.txt",
            ]
        );
    }

    #[test]
    fn each_player_gets_an_own_id() {
        let mut world = world();
        let (first, _) = world.join("ghost");
        let (second, outputs) = world.join("nyx");
        assert_ne!(first, second);
        assert!(outputs.iter().any(|output| output.player == second));
    }

    #[test]
    fn scene_styles_the_title_and_the_names() {
        let (_, outputs) = world().join("ghost");
        assert_eq!(outputs[0].message.spans[0].style, Style::Title);
        let route = &outputs[2].message.spans[1];
        assert_eq!((route.style, route.text.as_str()), (Style::Host, "market"));
        let file = &outputs[3].message.spans[1];
        assert_eq!((file.style, file.text.as_str()), (Style::File, "motd.txt"));
    }

    #[test]
    fn the_world_knows_its_city() {
        assert_eq!(world().city(), "Test City");
    }

    #[test]
    fn prompt_shows_the_player_and_the_host() {
        let (world, player) = joined();
        assert_eq!(world.prompt(player).as_deref(), Some("ghost@relay $ "));
    }

    #[test]
    fn unknown_players_have_no_prompt() {
        assert_eq!(world().prompt(PlayerId(99)), None);
    }

    #[test]
    fn look_shows_the_scene() {
        let mut world = world();
        let (player, joined) = world.join("ghost");
        assert_eq!(world.command(player, "look"), joined);
    }

    #[test]
    fn l_is_short_for_look() {
        assert_eq!(run("l"), run("look"));
    }

    #[test]
    fn command_names_ignore_case() {
        assert_eq!(run("LOOK"), run("look"));
    }

    #[test]
    fn ls_lists_routes_and_files() {
        assert_eq!(
            run("ls"),
            ["market/", "back-room/", "bazaar/", "motd.txt", "rules.txt"]
        );
    }

    #[test]
    fn blank_lines_do_nothing() {
        assert_eq!(run("   "), Vec::<String>::new());
    }

    #[test]
    fn unknown_commands_point_to_help() {
        assert_eq!(
            run("xyzzy"),
            ["Unknown command: xyzzy. Type help to see the commands."]
        );
    }

    #[test]
    fn typos_get_a_suggestion() {
        assert_eq!(run("lok"), ["Unknown command: lok. Did you mean look?"]);
        assert_eq!(run("LX"), ["Unknown command: LX. Did you mean ls?"]);
    }

    #[test]
    fn extra_arguments_fail() {
        assert_eq!(run("ls -la"), ["ls takes no arguments."]);
    }

    #[test]
    fn commands_without_input_reject_pipes() {
        assert_eq!(run("look | ls"), ["ls takes no input from a pipe."]);
    }

    #[test]
    fn a_pipe_must_carry_what_the_next_command_needs() {
        assert_eq!(
            run("ls | crack"),
            ["crack needs ICE, but ls gives routes and files."]
        );
        assert_eq!(run("help | go"), ["go needs routes, but help gives lines."]);
        assert_eq!(
            run("ls | grep x | probe"),
            ["probe needs ICE, but ls gives routes and files."]
        );
    }

    #[test]
    fn a_pipe_that_does_not_fit_costs_nothing() {
        let lines = ["go back-room", "go gate", "scan | read"];
        assert_eq!(walk(&lines).0, ["read needs files, but scan gives ICE."]);
        let lines = [&lines[..], &["status | grep cycles"]].concat();
        assert_eq!(walk(&lines).0, ["Cycles: 4/4"]);
    }

    #[test]
    fn a_bad_stage_stops_the_pipeline_before_it_runs() {
        assert_eq!(run("look | xyzzy"), run("xyzzy"));
    }

    #[test]
    fn parse_errors_explain_the_problem() {
        assert_eq!(run("look \"x"), ["A quote is not closed."]);
        assert_eq!(run("ls |"), ["Each side of | needs a command."]);
    }

    #[test]
    fn command_lines_are_sanitized() {
        assert_eq!(run("\u{feff}l\u{200b}s\u{1b}"), run("ls"));
    }

    #[test]
    fn errors_have_the_error_kind() {
        let (mut world, player) = joined();
        let outputs = world.command(player, "xyzzy");
        assert_eq!(outputs[0].message.kind, Kind::Error);
    }

    #[test]
    fn unknown_players_get_no_output() {
        assert_eq!(world().command(PlayerId(9), "look"), Vec::new());
    }
}
