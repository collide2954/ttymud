use crate::content::File;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Verb {
    Look,
    List,
    Go,
    Back,
    Grep,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Item {
    Route(String),
    File(File),
}

impl Item {
    pub(crate) fn name(&self) -> &str {
        match self {
            Self::Route(id) => id,
            Self::File(file) => &file.name,
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
        example: "look",
        takes: Takes::Nothing,
        output: true,
    },
    Command {
        verb: Verb::List,
        names: &["ls"],
        example: "ls",
        takes: Takes::Nothing,
        output: true,
    },
    Command {
        verb: Verb::Go,
        names: &["go", "cd", "connect", "ssh"],
        example: "go market",
        takes: Takes::Optional,
        output: false,
    },
    Command {
        verb: Verb::Back,
        names: &["back", "exit"],
        example: "back",
        takes: Takes::Nothing,
        output: false,
    },
    Command {
        verb: Verb::Grep,
        names: &["grep"],
        example: "ls | grep txt",
        takes: Takes::Required,
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
