mod chat;
mod complete;
mod contract;
mod files;
mod grep;
mod help;
mod ice;
mod movement;
mod presence;
mod programs;
mod ram;
mod rig;
mod run;
mod topics;

use std::collections::BTreeMap;

use ttymud_text::{Kind, Message, Style, sanitize};

use crate::commands::{self, Command, Item, Verb};
use crate::content::{Content, File, IceDef};
use crate::contract::Event;
use crate::matching::{self, Found};
use crate::parse::{self, ParseError, Stage};
use crate::random::Random;

pub use chat::is_chat;
pub use complete::Completion;
use contract::{Contract, Job};
use rig::Rig;
use run::Run;

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
}

#[derive(Debug)]
struct Player {
    name: String,
    chain: Vec<HostId>,
    rig: Rig,
    know: Vec<Verb>,
    run: Option<Run>,
    contract: Option<Job>,
}

#[derive(Debug)]
pub struct World {
    city: String,
    networks: Vec<Network>,
    hosts: Vec<Host>,
    start: HostId,
    home: Option<HostId>,
    contracts: Vec<Contract>,
    offer: Option<usize>,
    players: BTreeMap<PlayerId, Player>,
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
        let contracts = content
            .contracts
            .into_iter()
            .map(|def| Contract {
                fixer: def.fixer,
                network: network(&def.network).unwrap_or(NetworkId(0)),
                goal: def.goal,
                offer: def.offer,
                steps: def.steps,
            })
            .collect();
        let networks = content
            .networks
            .into_iter()
            .map(|network| Network { name: network.name })
            .collect();
        Self {
            city: content.city,
            start: find(&content.start),
            home: content.home.as_deref().map(find),
            contracts,
            offer,
            networks,
            hosts,
            players: BTreeMap::new(),
            next_player: 0,
            outbox: Vec::new(),
            random: Random::new(seed),
        }
    }

    pub fn join(&mut self, name: &str) -> (PlayerId, Vec<Output>) {
        let player = PlayerId(self.next_player);
        self.next_player += 1;
        self.players.insert(
            player,
            Player {
                name: name.to_owned(),
                chain: vec![self.start],
                rig: Rig::new(),
                know: commands::programs()
                    .map(|(command, _)| command.verb)
                    .collect(),
                run: None,
                contract: self.offer.map(Job::new),
            },
        );
        self.notify(self.start, player, &presence::named(name, " connects."));
        let mut shown = self.scene(self.start, player);
        shown.extend(self.offer(player));
        (player, self.deliver(player, shown))
    }

    pub fn command(&mut self, player: PlayerId, line: &str) -> Vec<Output> {
        if !self.players.contains_key(&player) {
            return Vec::new();
        }
        let line = sanitize(line);
        if let Some(text) = chat::chat_text(&line) {
            return self.say(player, text);
        }
        let messages = match parse::pipeline(&line) {
            Ok(stages) => self.run(player, &stages),
            Err(error) => vec![parse_error(error)],
        };
        self.deliver(player, messages)
    }

    pub fn tick(&mut self) -> Vec<Output> {
        let players: Vec<PlayerId> = self.players.keys().copied().collect();
        let mut outputs = Vec::new();
        for player in players {
            if let Some(state) = self.players.get_mut(&player) {
                state.rig.tick();
            }
            let mut shown = self.tick_ice(player);
            shown.extend(self.hint(player));
            outputs.extend(address(player, shown));
        }
        outputs.append(&mut self.outbox);
        outputs
    }

    pub fn leave(&mut self, player: PlayerId) -> Vec<Output> {
        let Some(state) = self.players.remove(&player) else {
            return Vec::new();
        };
        if let Some(&host) = state.chain.last() {
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
                    ran.extend(command.verb.program());
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
        let here = self.here(player);
        match command.verb {
            Verb::Look => {
                no_args(command, args)?;
                Ok(Step {
                    shown: self.scene(here, player),
                    items: self.items(here),
                })
            }
            Verb::List => {
                no_args(command, args)?;
                let items = self.items(here);
                Ok(Step {
                    shown: listing(&items),
                    items,
                })
            }
            Verb::Go => self.go(player, args, input),
            Verb::Back => {
                no_args(command, args)?;
                self.back(player)
            }
            Verb::Read => self.read(player, args, input),
            Verb::Download => self.download(player, args, input),
            Verb::Inventory => {
                no_args(command, args)?;
                Ok(self.inventory(player))
            }
            Verb::Status => {
                no_args(command, args)?;
                Ok(self.status(player))
            }
            Verb::Jackout => {
                no_args(command, args)?;
                self.jackout(player)
            }
            Verb::Repair => {
                no_args(command, args)?;
                self.repair(player)
            }
            Verb::Contract => self.contract(player, args),
            Verb::Load => self.load(player, args),
            Verb::Unload => self.unload(player, args),
            Verb::Scan => {
                no_args(command, args)?;
                self.scan(player)
            }
            Verb::Probe => self.probe(player, args, input),
            Verb::Crack => self.crack(player, args, input),
            Verb::Scrub => {
                no_args(command, args)?;
                self.scrub(player)
            }
            Verb::Patch => {
                no_args(command, args)?;
                self.patch(player)
            }
            Verb::Grep => grep::grep(args, input.unwrap_or_default()),
            Verb::Say => Err(Message::error("Usage: say <text>.")),
            Verb::Help => help::help(args),
            Verb::Available => {
                no_args(command, args)?;
                Ok(self.available(player))
            }
        }
    }

    fn here(&self, player: PlayerId) -> HostId {
        self.players
            .get(&player)
            .and_then(|player| player.chain.last().copied())
            .unwrap_or(self.start)
    }

    fn items(&self, id: HostId) -> Vec<Item> {
        let host = &self.hosts[id.0];
        let routes = host
            .routes
            .iter()
            .map(|route| Item::Route(self.hosts[route.0].id.clone()));
        let files = host.files.iter().cloned().map(Item::File);
        routes.chain(files).collect()
    }

    fn route_names(&self, id: HostId) -> Vec<&str> {
        self.hosts[id.0]
            .routes
            .iter()
            .map(|route| self.hosts[route.0].id.as_str())
            .collect()
    }

    fn scene(&self, id: HostId, viewer: PlayerId) -> Vec<Message> {
        let host = &self.hosts[id.0];
        let mut scene = vec![
            Message::new(Kind::Game).span(Style::Title, &host.name),
            Message::game(&host.description),
            listed(Message::game("Routes: "), Style::Host, self.route_names(id)),
        ];
        if !host.files.is_empty() {
            let files = host.files.iter().map(|file| file.name.as_str());
            scene.push(listed(Message::game("Files: "), Style::File, files));
        }
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
}

impl Thing {
    fn style(self) -> Style {
        match self {
            Self::Route => Style::Host,
            Self::File => Style::File,
            Self::Ice => Style::Ice,
            Self::Program => Style::Command,
        }
    }

    fn missing(self, word: &str) -> String {
        match self {
            Self::Route => format!("No route to {word}"),
            Self::File => format!("No file named {word}"),
            Self::Ice => format!("No ICE named {word}"),
            Self::Program => format!("No program named {word}"),
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Route => "Routes",
            Self::File => "Files",
            Self::Ice => "ICE",
            Self::Program => "Programs",
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
