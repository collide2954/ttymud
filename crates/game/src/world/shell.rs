use ttymud_text::{Kind, Message, Style};

use super::scripts::MAX_LINES;
use super::{
    Output, PlayerId, Step, Thing, World, address, choose, lines, parse_error, unknown, usage,
};
use crate::commands::{self, Verb};
use crate::parse::{self, Link, Op, Stage};

pub(super) const PROCESS_RAM: u32 = 1;
const MAX_WAIT: u32 = 30;

#[derive(Debug, Clone, PartialEq, Eq)]
enum Line {
    Wait { number: usize, ticks: u32 },
    Run { number: usize, links: Vec<Link> },
}

impl Line {
    fn number(&self) -> usize {
        match self {
            Self::Wait { number, .. } | Self::Run { number, .. } => *number,
        }
    }
}

#[derive(Debug)]
pub(super) struct Process {
    name: String,
    words: Vec<String>,
    total: usize,
    lines: Vec<Line>,
    line: usize,
    link: usize,
    failed: bool,
    wait: u32,
}

enum Next<'a> {
    Run(&'a [Stage]),
    Wait(u32),
    Stop(usize),
    End,
}

impl Process {
    fn next(&self) -> Next<'_> {
        match self.lines.get(self.line) {
            None => Next::End,
            Some(Line::Wait { ticks, .. }) => Next::Wait(*ticks),
            Some(Line::Run { number, links }) => match links.get(self.link) {
                Some(link) => Next::Run(&link.stages),
                None => Next::Stop(*number),
            },
        }
    }

    fn ran(&mut self, worked: bool) {
        self.failed = !worked;
        self.link += 1;
        self.settle();
    }

    fn waited(&mut self, ticks: u32) {
        self.wait = ticks - 1;
        self.next_line();
    }

    fn next_line(&mut self) {
        self.line += 1;
        self.link = 0;
        self.failed = false;
    }

    fn settle(&mut self) {
        loop {
            let runs = match self.lines.get(self.line) {
                Some(Line::Run { links, .. }) => links.get(self.link).map(|link| match link.op {
                    None => true,
                    Some(Op::And) => !self.failed,
                    Some(Op::Or) => self.failed,
                }),
                _ => return,
            };
            match runs {
                Some(true) => return,
                Some(false) => self.link += 1,
                None if self.failed => return,
                None => self.next_line(),
            }
        }
    }
}

impl World {
    pub(super) fn sh(&mut self, player: PlayerId, args: &[String]) -> Result<Step, Message> {
        let scripts = self.scripts(player);
        let names: Vec<&str> = scripts.iter().map(String::as_str).collect();
        let Some((word, words)) = args.split_first() else {
            return Err(usage("sh <script> [words]", Thing::Script, &names));
        };
        let name = choose(Thing::Script, word, &names)?.to_owned();
        let Some(rig) = self.players.get_mut(&player).map(|state| &mut state.rig) else {
            return Ok(Step::default());
        };
        if let Some(process) = &rig.process {
            return Err(Message::error("")
                .span(Style::File, &process.name)
                .text(" runs. Type ")
                .span(Style::Command, "kill")
                .text(" to stop it first."));
        }
        if rig.used() + PROCESS_RAM > rig.ram {
            return Err(Message::error(format!(
                "A script needs {PROCESS_RAM} GB of free RAM. {}. Unload a program at home, or buy RAM at a shop.",
                rig.ram_line()
            )));
        }
        let text = rig
            .files
            .iter()
            .find(|file| file.name == name)
            .map_or_default(|file| file.lines.clone());
        let lines = read_script(&name, &text, words)?;
        if lines.is_empty() {
            return Err(Message::error("")
                .span(Style::File, &name)
                .text(" has no commands."));
        }
        let started = Message::new(Kind::Game)
            .span(Style::File, &name)
            .text(" starts. It runs one command on each tick.");
        rig.process = Some(Process {
            name,
            words: words.to_vec(),
            total: text.len(),
            lines,
            line: 0,
            link: 0,
            failed: false,
            wait: 0,
        });
        Ok(Step::show(vec![started]))
    }

    pub(super) fn ps(&self, player: PlayerId) -> Step {
        let process = self
            .players
            .get(&player)
            .and_then(|state| state.rig.process.as_ref());
        let Some(process) = process else {
            return lines(vec![Message::game("No script runs.")]);
        };
        let at = process
            .lines
            .get(process.line)
            .map_or(process.total, Line::number);
        let words = match process.words.as_slice() {
            [] => String::new(),
            words => format!(" {}", words.join(" ")),
        };
        let mut shown = vec![
            Message::new(Kind::Game)
                .span(Style::File, &process.name)
                .text(format!(
                    "{words} is at line {at} of {}. It uses {PROCESS_RAM} GB of RAM.",
                    process.total
                )),
        ];
        if process.wait > 0 {
            let ticks = process.wait;
            let unit = if ticks == 1 { "tick" } else { "ticks" };
            shown.push(Message::game(format!("It waits for {ticks} {unit}.")));
        }
        lines(shown)
    }

    pub(super) fn kill(&mut self, player: PlayerId) -> Result<Step, Message> {
        let process = self
            .players
            .get_mut(&player)
            .and_then(|state| state.rig.process.take())
            .ok_or_else(|| Message::error("No script runs."))?;
        Ok(Step::show(vec![
            Message::game("You stop ")
                .span(Style::File, process.name)
                .text("."),
        ]))
    }

    pub(super) fn stop_script(&mut self, player: PlayerId) -> Option<Message> {
        let process = self.players.get_mut(&player)?.rig.process.take()?;
        Some(
            Message::new(Kind::Game)
                .span(Style::File, process.name)
                .text(" stops."),
        )
    }

    pub(super) fn tick_scripts(&mut self) -> Vec<Output> {
        let running: Vec<PlayerId> = self
            .players
            .iter()
            .filter(|(_, state)| state.rig.process.is_some())
            .map(|(&player, _)| player)
            .collect();
        let mut outputs = Vec::new();
        for player in running {
            let shown = self.step_script(player);
            outputs.extend(address(player, shown));
        }
        outputs
    }

    fn step_script(&mut self, player: PlayerId) -> Vec<Message> {
        let mut shown = Vec::new();
        if let Some((name, stages)) = self.next_command(player)
            && !self.short_of_cycles(player, &stages)
        {
            shown.push(
                Message::new(Kind::Game)
                    .span(Style::File, name)
                    .text(": ")
                    .span(Style::Command, command_text(&stages)),
            );
            let (output, worked) = self.pipeline(player, &stages);
            shown.extend(output);
            if let Some(process) = self.process_mut(player) {
                process.ran(worked);
            }
        }
        shown.extend(self.end_script(player));
        shown
    }

    fn next_command(&mut self, player: PlayerId) -> Option<(String, Vec<Stage>)> {
        let process = self.process_mut(player)?;
        if process.wait > 0 {
            process.wait -= 1;
            return None;
        }
        let ticks = match process.next() {
            Next::Run(stages) => return Some((process.name.clone(), stages.to_vec())),
            Next::Wait(ticks) => ticks,
            Next::Stop(_) | Next::End => return None,
        };
        process.waited(ticks);
        None
    }

    fn short_of_cycles(&self, player: PlayerId, stages: &[Stage]) -> bool {
        let found: Option<Vec<_>> = stages
            .iter()
            .map(|stage| commands::find(&stage.name))
            .collect();
        let (Some(found), Some(state)) = (found, self.players.get(&player)) else {
            return false;
        };
        let cost = self.cycles_for(player, &found);
        self.allowed(player, &found).is_ok() && cost > state.rig.cycles && cost <= state.rig.cpu
    }

    fn end_script(&mut self, player: PlayerId) -> Option<Message> {
        let rig = &mut self.players.get_mut(&player)?.rig;
        let process = rig.process.as_ref()?;
        let end = match (process.wait, process.next()) {
            (0, Next::End) => " ends.".to_owned(),
            (_, Next::Stop(number)) => format!(" stops at line {number}."),
            _ => return None,
        };
        let process = rig.process.take()?;
        Some(
            Message::new(Kind::Game)
                .span(Style::File, process.name)
                .text(end),
        )
    }

    fn process_mut(&mut self, player: PlayerId) -> Option<&mut Process> {
        self.players.get_mut(&player)?.rig.process.as_mut()
    }
}

fn read_script(name: &str, text: &[String], words: &[String]) -> Result<Vec<Line>, Message> {
    if text.len() > MAX_LINES {
        return Err(Message::error("").span(Style::File, name).text(format!(
            " has {} lines. A script has {MAX_LINES} lines at most.",
            text.len()
        )));
    }
    let mut lines = Vec::new();
    for (index, raw) in text.iter().enumerate() {
        let number = index + 1;
        let expanded = expand(raw, words);
        let line = expanded.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let read = parse::list(line)
            .map_err(parse_error)
            .and_then(|links| check(number, links));
        lines.push(read.map_err(|message| located(name, number, message))?);
    }
    Ok(lines)
}

fn check(number: usize, links: Vec<Link>) -> Result<Line, Message> {
    if let [Link { stages, .. }] = links.as_slice()
        && let [stage] = stages.as_slice()
        && commands::find(&stage.name).is_some_and(|command| command.verb == Verb::Wait)
    {
        let ticks = wait_ticks(&stage.args)?;
        return Ok(Line::Wait { number, ticks });
    }
    for link in &links {
        let mut found = Vec::with_capacity(link.stages.len());
        for stage in &link.stages {
            let command = commands::find(&stage.name).ok_or_else(|| unknown(&stage.name))?;
            if command.verb == Verb::Wait {
                return Err(Message::error("wait must be alone on its line."));
            }
            if !command.verb.scripted() {
                let name = command.name();
                return Err(Message::error(format!("A script cannot use {name}.")));
            }
            found.push(command);
        }
        commands::check(&found).map_err(Message::error)?;
    }
    Ok(Line::Run { number, links })
}

fn wait_ticks(args: &[String]) -> Result<u32, Message> {
    let ticks = match args {
        [] => Some(1),
        [word] => word
            .parse()
            .ok()
            .filter(|ticks| (1..=MAX_WAIT).contains(ticks)),
        _ => None,
    };
    ticks.ok_or_else(|| Message::error(format!("Usage: wait [ticks]. Use 1 to {MAX_WAIT} ticks.")))
}

fn expand(line: &str, words: &[String]) -> String {
    let mut text = String::with_capacity(line.len());
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        let index = chars
            .peek()
            .and_then(|next| next.to_digit(10))
            .and_then(|digit| usize::try_from(digit).ok())
            .filter(|&digit| digit > 0);
        match index {
            Some(digit) if c == '$' => {
                chars.next();
                text.push_str(words.get(digit - 1).map_or("", String::as_str));
            }
            _ => text.push(c),
        }
    }
    text
}

fn located(name: &str, number: usize, inner: Message) -> Message {
    let mut message = Message::error("")
        .span(Style::File, name)
        .text(format!(" line {number}: "));
    message.spans.extend(inner.spans);
    message
}

fn command_text(stages: &[Stage]) -> String {
    let stages: Vec<String> = stages
        .iter()
        .map(|stage| {
            let args = stage.args.iter().map(|arg| quoted(arg));
            std::iter::once(stage.name.clone())
                .chain(args)
                .collect::<Vec<_>>()
                .join(" ")
        })
        .collect();
    stages.join(" | ")
}

fn quoted(arg: &str) -> String {
    let plain = !arg.is_empty()
        && !arg
            .chars()
            .any(|c| c.is_whitespace() || matches!(c, '|' | '&' | '\'' | '"'));
    if plain {
        arg.to_owned()
    } else if arg.contains('"') {
        format!("'{arg}'")
    } else {
        format!("\"{arg}\"")
    }
}

#[cfg(test)]
mod tests {
    use super::{Line, read_script};
    use crate::content::File;
    use crate::parse::{Link, Op, Stage};
    use crate::testing::{heard, joined, plain};
    use crate::{PlayerId, World};

    fn read(text: &[&str], words: &[&str]) -> Result<Vec<Line>, String> {
        let text: Vec<String> = text.iter().map(|&line| line.to_owned()).collect();
        let words: Vec<String> = words.iter().map(|&word| word.to_owned()).collect();
        read_script("x.sh", &text, &words).map_err(|message| message.plain())
    }

    fn stage(name: &str, args: &[&str]) -> Stage {
        Stage {
            name: name.to_owned(),
            args: args.iter().map(|&arg| arg.to_owned()).collect(),
        }
    }

    fn run_line(number: usize, links: Vec<(Option<Op>, Vec<Stage>)>) -> Line {
        let links = links
            .into_iter()
            .map(|(op, stages)| Link { op, stages })
            .collect();
        Line::Run { number, links }
    }

    #[test]
    fn comments_and_blank_lines_are_not_commands() {
        assert_eq!(
            read(&["# set up", "", "scan", "  # more", "wait"], &[]),
            Ok(vec![
                run_line(3, vec![(None, vec![stage("scan", &[])])]),
                Line::Wait {
                    number: 5,
                    ticks: 1
                },
            ])
        );
    }

    #[test]
    fn dollar_numbers_give_the_words_after_the_script_name() {
        assert_eq!(
            read(&["crack $1 && go $2-gw$3"], &["sentry", "mail"]),
            Ok(vec![run_line(
                1,
                vec![
                    (None, vec![stage("crack", &["sentry"])]),
                    (Some(Op::And), vec![stage("go", &["mail-gw"])]),
                ]
            )])
        );
        assert_eq!(
            read(&["grep '$1 costs $0'"], &["crack"]),
            read(&["grep 'crack costs $0'"], &[])
        );
    }

    #[test]
    fn wait_takes_1_to_30_ticks_on_its_own_line() {
        assert_eq!(
            read(&["wait 30"], &[]),
            Ok(vec![Line::Wait {
                number: 1,
                ticks: 30
            }])
        );
        for bad in ["wait 0", "wait 31", "wait x", "wait 1 2"] {
            assert_eq!(
                read(&[bad], &[]),
                Err("x.sh line 1: Usage: wait [ticks]. Use 1 to 30 ticks.".to_owned()),
                "{bad}"
            );
        }
        assert_eq!(
            read(&["scan", "scan && wait"], &[]),
            Err("x.sh line 2: wait must be alone on its line.".to_owned())
        );
    }

    #[test]
    fn a_script_is_checked_before_it_runs() {
        for (line, problem) in [
            ("crak sentry", "Unknown command: crak. Did you mean crack?"),
            ("say hi", "A script cannot use say."),
            ("tell nyx hi", "A script cannot use tell."),
            ("mute zer0 5", "A script cannot use mute."),
            ("sh x.sh", "A script cannot use sh."),
            (
                "ls | crack",
                "crack needs ICE, but ls gives routes and files.",
            ),
            ("grep 'a", "A quote is not closed."),
            ("scan &&", "Each side of && and || needs a command."),
        ] {
            assert_eq!(
                read(&["look", line], &[]),
                Err(format!("x.sh line 2: {problem}")),
                "{line}"
            );
        }
        assert_eq!(
            read(&["ls"; 21], &[]),
            Err("x.sh has 21 lines. A script has 20 lines at most.".to_owned())
        );
    }

    fn with_script(text: &[&str]) -> (World, PlayerId) {
        let (mut world, player) = joined();
        if let Some(state) = world.players.get_mut(&player) {
            state.rig.ram = 9;
            let text = text.iter().map(|&line| line.to_owned()).collect();
            state.rig.files.push(File::new("x.sh", text));
        }
        (world, player)
    }

    fn tick(world: &mut World, player: PlayerId) -> Vec<String> {
        heard(&world.tick(), player)
    }

    fn started(text: &[&str], line: &str) -> (World, PlayerId) {
        let (mut world, player) = with_script(text);
        assert_eq!(
            plain(&world.command(player, line)),
            ["x.sh starts. It runs one command on each tick."]
        );
        (world, player)
    }

    #[test]
    fn a_script_runs_one_command_on_each_tick() {
        let (mut world, player) = started(&["# look first", "look", "ls | grep txt"], "sh x.sh");
        assert_eq!(
            tick(&mut world, player),
            [
                "x.sh: look",
                "The Relay",
                "A crowded public node.",
                "Routes: market, back-room, bazaar",
                "Files: motd.txt, rules.txt",
            ]
        );
        assert_eq!(
            tick(&mut world, player),
            ["x.sh: ls | grep txt", "motd.txt", "rules.txt", "x.sh ends."]
        );
        assert_eq!(tick(&mut world, player), Vec::<String>::new());
    }

    #[test]
    fn the_words_after_the_script_name_go_into_the_script() {
        let (mut world, player) = started(&["go $1"], "sh x market");
        assert_eq!(
            tick(&mut world, player)[..2],
            ["x.sh: go market", "The Night Market"]
        );
    }

    #[test]
    fn a_failed_command_stops_the_script() {
        let (mut world, player) = started(&["read zzz", "look"], "sh x.sh");
        assert_eq!(
            tick(&mut world, player),
            [
                "x.sh: read zzz",
                "No file named zzz. Files: motd.txt, rules.txt, x.sh",
                "x.sh stops at line 1.",
            ]
        );
        assert_eq!(tick(&mut world, player), Vec::<String>::new());
    }

    #[test]
    fn or_runs_after_a_failure_and_and_runs_after_a_success() {
        let (mut world, player) = started(
            &[
                "read zzz && look || ls | grep motd",
                "ls | grep rules && read rules.txt",
            ],
            "sh x.sh",
        );
        assert_eq!(tick(&mut world, player)[0], "x.sh: read zzz");
        assert_eq!(
            tick(&mut world, player),
            ["x.sh: ls | grep motd", "motd.txt"]
        );
        assert_eq!(
            tick(&mut world, player),
            ["x.sh: ls | grep rules", "rules.txt"]
        );
        assert_eq!(
            tick(&mut world, player),
            ["x.sh: read rules.txt", "No spam.", "x.sh ends."]
        );
    }

    #[test]
    fn a_line_that_ends_in_a_failure_stops_the_script() {
        let (mut world, player) = started(&["ls | grep motd && read zzz", "look"], "sh x.sh");
        tick(&mut world, player);
        let shown = tick(&mut world, player);
        assert_eq!(shown[0], "x.sh: read zzz");
        assert_eq!(shown[2], "x.sh stops at line 1.");
    }

    #[test]
    fn wait_lets_ticks_go_by() {
        let (mut world, player) = started(&["wait 2", "ls | grep rules"], "sh x.sh");
        assert_eq!(tick(&mut world, player), Vec::<String>::new());
        assert_eq!(tick(&mut world, player), Vec::<String>::new());
        assert_eq!(
            tick(&mut world, player),
            ["x.sh: ls | grep rules", "rules.txt", "x.sh ends."]
        );
    }

    fn in_run(text: &[&str], cycles: u32) -> (World, PlayerId) {
        let (mut world, player) = with_script(text);
        for line in ["go back-room", "go gate"] {
            world.command(player, line);
        }
        if let Some(state) = world.players.get_mut(&player) {
            state.rig.cycles = cycles;
        }
        world.command(player, "sh x.sh");
        (world, player)
    }

    fn ran(shown: &[String]) -> bool {
        shown.iter().any(|line| line.starts_with("x.sh"))
    }

    #[test]
    fn a_script_waits_for_the_cycles_of_its_command() {
        let (mut world, player) = in_run(&["scan"], 0);
        assert!(!ran(&tick(&mut world, player)));
        let shown = tick(&mut world, player);
        assert_eq!(shown[0], "x.sh: scan", "{shown:?}");
        assert_eq!(
            plain(&world.command(player, "status | grep cycles")),
            ["Cycles: 0/4"]
        );
    }

    #[test]
    fn a_command_that_needs_more_cycles_than_the_cpu_holds_fails() {
        let (mut world, player) = in_run(&["scan | probe | crack"], 4);
        let shown = tick(&mut world, player);
        assert_eq!(
            shown[..3],
            [
                "x.sh: scan | probe | crack",
                "This pipeline needs 5 cycles, but you have 4. You get 1 cycle on each tick.",
                "x.sh stops at line 1.",
            ]
        );
    }

    #[test]
    fn one_script_runs_at_a_time_and_kill_stops_it() {
        let (mut world, player) = started(&["look", "look"], "sh x.sh");
        assert_eq!(
            plain(&world.command(player, "sh x.sh")),
            ["x.sh runs. Type kill to stop it first."]
        );
        assert_eq!(plain(&world.command(player, "kill")), ["You stop x.sh."]);
        assert_eq!(tick(&mut world, player), Vec::<String>::new());
        assert_eq!(plain(&world.command(player, "kill")), ["No script runs."]);
    }

    #[test]
    fn ps_shows_the_script_and_its_line() {
        let (mut world, player) = with_script(&["# a", "look", "wait 3", "look"]);
        assert_eq!(plain(&world.command(player, "ps")), ["No script runs."]);
        world.command(player, "sh x.sh north");
        assert_eq!(
            plain(&world.command(player, "ps")),
            ["x.sh north is at line 2 of 4. It uses 1 GB of RAM."]
        );
        tick(&mut world, player);
        tick(&mut world, player);
        assert_eq!(
            plain(&world.command(player, "ps")),
            [
                "x.sh north is at line 4 of 4. It uses 1 GB of RAM.",
                "It waits for 2 ticks."
            ]
        );
    }

    #[test]
    fn a_running_script_uses_1_gb_of_ram() {
        let (mut world, player) = with_script(&["look"]);
        if let Some(state) = world.players.get_mut(&player) {
            state.rig.ram = 8;
        }
        assert_eq!(
            plain(&world.command(player, "sh x.sh")),
            [
                "A script needs 1 GB of free RAM. RAM: 8/8 GB. Unload a program at home, or buy RAM at a shop."
            ]
        );
        if let Some(state) = world.players.get_mut(&player) {
            state.rig.ram = 9;
        }
        world.command(player, "sh x.sh");
        let ram = |world: &mut World| plain(&world.command(player, "status | grep GB"));
        assert_eq!(ram(&mut world), ["RAM: 9/9 GB"]);
        tick(&mut world, player);
        assert_eq!(ram(&mut world), ["RAM: 8/9 GB"]);
    }

    fn stopped(world: &World, player: PlayerId) -> bool {
        world
            .players
            .get(&player)
            .is_some_and(|state| state.rig.process.is_none())
    }

    #[test]
    fn a_crash_stops_the_script() {
        let (mut world, player) = in_run(&["wait 30"], 4);
        if let Some(state) = world.players.get_mut(&player) {
            state.rig.integrity = 5;
        }
        let shown: Vec<String> = world
            .damage(player, 10)
            .iter()
            .map(ttymud_text::Message::plain)
            .collect();
        assert_eq!(
            shown[..2],
            [
                "Your rig crashes, and you lose the loot of this run.",
                "x.sh stops."
            ]
        );
        assert!(stopped(&world, player));
    }

    #[test]
    fn a_trace_stops_the_script() {
        let (mut world, player) = in_run(&["wait 30"], 4);
        let shown: Vec<String> = world
            .raise_trace(player, 100)
            .iter()
            .map(ttymud_text::Message::plain)
            .collect();
        assert_eq!(shown[2], "x.sh stops.", "{shown:?}");
        assert!(stopped(&world, player));
    }

    #[test]
    fn a_burn_stops_the_script() {
        let (mut world, player) = in_run(&["wait 30"], 4);
        if let Some(state) = world.players.get_mut(&player) {
            state.heat = 4;
            if let Some(run) = state.run.as_mut() {
                run.trace = 80;
            }
        }
        let shown = plain(&world.command(player, "jackout"));
        assert!(shown.contains(&"x.sh stops.".to_owned()), "{shown:?}");
        assert!(stopped(&world, player));
    }

    #[test]
    fn a_disconnect_stops_the_script() {
        let mut world = crate::testing::world();
        let player = crate::testing::hacker(&mut world, "a", "nyx");
        if let Some(state) = world.players.get_mut(&player) {
            state.rig.ram = 9;
            state
                .rig
                .files
                .push(File::new("x.sh", vec!["wait 30".to_owned()]));
        }
        world.command(player, "sh x.sh");
        world.leave(player);
        let (player, _) = world.connect("a");
        assert_eq!(plain(&world.command(player, "ps")), ["No script runs."]);
        assert_eq!(
            plain(&world.command(player, "status | grep GB")),
            ["RAM: 8/9 GB"]
        );
    }

    #[test]
    fn sh_checks_the_whole_script_before_it_starts() {
        let (mut world, player) = with_script(&["look", "crak"]);
        assert_eq!(
            plain(&world.command(player, "sh x.sh")),
            ["x.sh line 2: Unknown command: crak. Did you mean crack?"]
        );
        assert_eq!(plain(&world.command(player, "ps")), ["No script runs."]);
        assert_eq!(
            plain(&world.command(player, "sh")),
            ["Usage: sh <script> [words]. Scripts: x.sh"]
        );
        assert_eq!(
            plain(&world.command(player, "sh zzz")),
            ["No script named zzz. Scripts: x.sh"]
        );
        let (mut world, player) = with_script(&["# only a note"]);
        assert_eq!(
            plain(&world.command(player, "sh x.sh")),
            ["x.sh has no commands."]
        );
    }
}
