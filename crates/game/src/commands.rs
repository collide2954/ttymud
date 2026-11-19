use crate::content::File;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Verb {
    Look,
    List,
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
    Required,
}

#[derive(Debug)]
pub(crate) struct Command {
    pub(crate) verb: Verb,
    pub(crate) names: &'static [&'static str],
    pub(crate) example: &'static str,
    pub(crate) takes: Takes,
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
    },
    Command {
        verb: Verb::List,
        names: &["ls"],
        example: "ls",
        takes: Takes::Nothing,
    },
    Command {
        verb: Verb::Grep,
        names: &["grep"],
        example: "ls | grep txt",
        takes: Takes::Required,
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
    for command in commands {
        let name = command.name();
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
