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
    Grep,
    Say,
    Help,
    Available,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Item {
    Route(String),
    File(File),
    Line(String),
}

impl Item {
    pub(crate) fn name(&self) -> &str {
        match self {
            Self::Route(id) => id,
            Self::File(file) => &file.name,
            Self::Line(text) => text,
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
        summary: "Show your integrity and cycles.",
        example: "status",
        takes: Takes::Nothing,
        output: true,
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
