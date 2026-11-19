use crate::content::File;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Verb {
    Look,
    List,
    Go,
    Back,
    Read,
    Download,
    Inventory,
    Status,
    Jackout,
    Repair,
    Scan,
    Probe,
    Crack,
    Scrub,
    Patch,
    Grep,
    Say,
    Help,
    Available,
}

const SETUP: u32 = 1;
const CONNECTION: u32 = 1;

#[derive(Debug, Clone, Copy)]
pub(crate) struct Program {
    pub(crate) ram: u32,
    pub(crate) cycles: u32,
    pub(crate) noise: u32,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Cost {
    pub(crate) cycles: u32,
    pub(crate) noise: u32,
}

impl Verb {
    pub(crate) fn program(self) -> Option<Program> {
        let (ram, cycles, noise) = match self {
            Self::Scan | Self::Probe => (1, 1, 1),
            Self::Crack => (2, 2, 2),
            Self::Scrub | Self::Patch => (2, 2, 0),
            _ => return None,
        };
        Some(Program { ram, cycles, noise })
    }
}

pub(crate) fn cost(programs: impl IntoIterator<Item = Program>) -> Cost {
    let mut programs = programs.into_iter().peekable();
    if programs.peek().is_none() {
        return Cost::default();
    }
    let start = Cost {
        cycles: SETUP,
        noise: 0,
    };
    let mut cost = programs.fold(start, |cost, program| Cost {
        cycles: cost.cycles + program.cycles,
        noise: cost.noise + program.noise,
    });
    if cost.noise > 0 {
        cost.noise += CONNECTION;
    }
    cost
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Item {
    Route(String),
    File(File),
    Ice { name: String, weak: bool },
    Line(String),
}

impl Item {
    pub(crate) fn name(&self) -> &str {
        match self {
            Self::Route(id) => id,
            Self::File(file) => &file.name,
            Self::Ice { name, .. } | Self::Line(name) => name,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum Takes {
    Nothing,
    Optional,
    Required,
}

#[derive(Debug)]
pub(crate) struct Command {
    pub(crate) verb: Verb,
    pub(crate) names: &'static [&'static str],
    pub(crate) usage: &'static str,
    pub(crate) summary: &'static str,
    pub(crate) example: &'static str,
    pub(crate) takes: Takes,
    pub(crate) output: bool,
}

impl Command {
    pub(crate) fn name(&self) -> &'static str {
        self.names[0]
    }
}

pub(crate) const COMMANDS: &[Command] = &[
    Command {
        verb: Verb::Look,
        names: &["look", "l"],
        usage: "look",
        summary: "Show the host where you are.",
        example: "look",
        takes: Takes::Nothing,
        output: true,
    },
    Command {
        verb: Verb::List,
        names: &["ls"],
        usage: "ls",
        summary: "List the routes and files here.",
        example: "ls",
        takes: Takes::Nothing,
        output: true,
    },
    Command {
        verb: Verb::Go,
        names: &["go", "cd", "connect", "ssh"],
        usage: "go <place>",
        summary: "Move to a host through a route.",
        example: "go market",
        takes: Takes::Optional,
        output: false,
    },
    Command {
        verb: Verb::Back,
        names: &["back", "exit"],
        usage: "back",
        summary: "Move back one hop. cd .. does the same.",
        example: "back",
        takes: Takes::Nothing,
        output: false,
    },
    Command {
        verb: Verb::Read,
        names: &["read", "cat"],
        usage: "read <file>",
        summary: "Show the text of a file here or on your rig.",
        example: "read motd.txt",
        takes: Takes::Optional,
        output: true,
    },
    Command {
        verb: Verb::Download,
        names: &["download", "get"],
        usage: "download <file>",
        summary: "Copy a file from this host to your rig.",
        example: "download motd.txt",
        takes: Takes::Optional,
        output: false,
    },
    Command {
        verb: Verb::Inventory,
        names: &["inventory", "inv", "i"],
        usage: "inventory",
        summary: "Show the files on your rig.",
        example: "inventory",
        takes: Takes::Nothing,
        output: true,
    },
    Command {
        verb: Verb::Status,
        names: &["status", "st"],
        usage: "status",
        summary: "Show your trace, integrity, cycles, and RAM.",
        example: "status",
        takes: Takes::Nothing,
        output: true,
    },
    Command {
        verb: Verb::Jackout,
        names: &["jackout"],
        usage: "jackout",
        summary: "Drop the link to the target at once. It costs integrity.",
        example: "jackout",
        takes: Takes::Nothing,
        output: false,
    },
    Command {
        verb: Verb::Repair,
        names: &["repair"],
        usage: "repair",
        summary: "Restore your integrity. This works only at home.",
        example: "repair",
        takes: Takes::Nothing,
        output: false,
    },
    Command {
        verb: Verb::Scan,
        names: &["scan"],
        usage: "scan",
        summary: "Find the ICE on this host.",
        example: "scan",
        takes: Takes::Nothing,
        output: true,
    },
    Command {
        verb: Verb::Probe,
        names: &["probe"],
        usage: "probe <ice>",
        summary: "Find the weak point of an ICE.",
        example: "probe sentry",
        takes: Takes::Optional,
        output: true,
    },
    Command {
        verb: Verb::Crack,
        names: &["crack"],
        usage: "crack <ice>",
        summary: "Attack an ICE. At zero integrity, the ICE breaks.",
        example: "crack sentry",
        takes: Takes::Optional,
        output: false,
    },
    Command {
        verb: Verb::Scrub,
        names: &["scrub"],
        usage: "scrub",
        summary: "Delete your log lines to lower the trace.",
        example: "scrub",
        takes: Takes::Nothing,
        output: false,
    },
    Command {
        verb: Verb::Patch,
        names: &["patch"],
        usage: "patch",
        summary: "Repair some integrity during a run.",
        example: "patch",
        takes: Takes::Nothing,
        output: false,
    },
    Command {
        verb: Verb::Grep,
        names: &["grep"],
        usage: "grep <pattern>",
        summary: "Keep only the items that contain the pattern.",
        example: "ls | grep txt",
        takes: Takes::Required,
        output: true,
    },
    Command {
        verb: Verb::Say,
        names: &["say", "'"],
        usage: "say <text>",
        summary: "Talk to everyone on this host.",
        example: "say hello",
        takes: Takes::Nothing,
        output: false,
    },
    Command {
        verb: Verb::Help,
        names: &["help", "man"],
        usage: "help [command]",
        summary: "Show help for all commands or for one command.",
        example: "help go",
        takes: Takes::Nothing,
        output: true,
    },
    Command {
        verb: Verb::Available,
        names: &["?"],
        usage: "?",
        summary: "Show the commands that you can use here.",
        example: "?",
        takes: Takes::Nothing,
        output: true,
    },
];

pub(crate) fn names() -> impl Iterator<Item = &'static str> {
    COMMANDS
        .iter()
        .flat_map(|command| command.names.iter().copied())
}

pub(crate) fn programs() -> impl Iterator<Item = (&'static Command, Program)> {
    COMMANDS
        .iter()
        .filter_map(|command| command.verb.program().map(|program| (command, program)))
}

pub(crate) fn find(name: &str) -> Option<&'static Command> {
    let name = name.to_lowercase();
    COMMANDS
        .iter()
        .find(|command| command.names.contains(&name.as_str()))
}

pub(crate) fn check(commands: &[&Command]) -> Result<(), String> {
    let mut piped = false;
    for (index, command) in commands.iter().enumerate() {
        let name = command.name();
        if !command.output && index + 1 < commands.len() {
            return Err(format!("{name} gives no output to a pipe."));
        }
        match (command.takes, piped) {
            (Takes::Nothing, true) => return Err(format!("{name} takes no input from a pipe.")),
            (Takes::Required, false) => {
                let example = command.example;
                return Err(format!(
                    "{name} needs input from a pipe. Example: {example}"
                ));
            }
            _ => {}
        }
        piped = true;
    }
    Ok(())
}
