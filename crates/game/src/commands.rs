#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Verb {
    Look,
    List,
}

#[derive(Debug)]
pub(crate) struct Command {
    pub(crate) verb: Verb,
    pub(crate) names: &'static [&'static str],
    pub(crate) takes_input: bool,
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
        takes_input: false,
    },
    Command {
        verb: Verb::List,
        names: &["ls"],
        takes_input: false,
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
