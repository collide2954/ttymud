use ttymud_text::{Kind, Message, Style};

use super::{PlayerId, Step, World, hint, lines, listed};
use crate::commands::{self, COMMANDS, Command, Verb};

pub(super) const TOPICS: &[&str] = &["pipes", "pipelines"];
const COLUMN: usize = 18;

pub(super) fn help(args: &[String]) -> Result<Step, Message> {
    let shown = match args {
        [] => overview(),
        [topic] if TOPICS.contains(&topic.to_lowercase().as_str()) => pipes(),
        [name] => {
            let Some(command) = commands::find(name) else {
                let message = Message::error(format!("No help for {name}."));
                let known = commands::names().chain(TOPICS.iter().copied());
                return Err(hint(message, name, known));
            };
            details(command)
        }
        _ => return Err(Message::error("Usage: help [command].")),
    };
    Ok(lines(shown))
}

impl World {
    pub(super) fn available(&self, player: PlayerId) -> Step {
        let host = &self.hosts[self.here(player).0];
        let hops = self
            .players
            .get(&player)
            .map_or(0, |state| state.chain.len());
        let readable = !self.readable(player).is_empty();
        let names = COMMANDS
            .iter()
            .filter(|command| match command.verb {
                Verb::Go => !host.routes.is_empty(),
                Verb::Back => hops > 1,
                Verb::Read => readable,
                Verb::Download => !host.files.is_empty(),
                Verb::Available => false,
                _ => true,
            })
            .map(Command::name);
        let message = Message::game("Here you can use: ");
        lines(vec![listed(message, Style::Command, names)])
    }
}

fn overview() -> Vec<Message> {
    let mut shown = vec![Message::new(Kind::Game).span(Style::Title, "Commands")];
    shown.extend(COMMANDS.iter().map(|command| {
        let gap = COLUMN.saturating_sub(command.usage.len()).max(1);
        Message::new(Kind::Game)
            .span(Style::Command, command.usage)
            .text(format!("{}{}", " ".repeat(gap), command.summary))
    }));
    shown.push(
        Message::game("Type ")
            .span(Style::Command, "help <command>")
            .text(" for more. Join commands with ")
            .span(Style::Command, "|")
            .text(" to make a pipeline."),
    );
    shown
}

fn details(command: &Command) -> Vec<Message> {
    let mut shown = vec![
        Message::new(Kind::Game).span(Style::Command, command.usage),
        Message::game(command.summary),
    ];
    if let [_, others @ ..] = command.names
        && !others.is_empty()
    {
        let message = Message::game("Other names: ");
        shown.push(listed(message, Style::Command, others.iter().copied()));
    }
    shown.push(Message::game("Example: ").span(Style::Command, command.example));
    shown
}

fn pipes() -> Vec<Message> {
    vec![
        Message::new(Kind::Game).span(Style::Title, "Pipelines"),
        Message::game("Join commands with ")
            .span(Style::Command, "|")
            .text(" to send the items of one command to the next."),
        Message::game("Only the last command shows its output."),
        Message::game("Example: ").span(Style::Command, "ls | grep txt | read"),
    ]
}

#[cfg(test)]
mod tests {
    use crate::commands::COMMANDS;
    use crate::testing::{run, walk};

    #[test]
    fn help_lists_every_command() {
        let shown = run("help");
        assert_eq!(shown[0], "Commands");
        assert_eq!(shown.len(), COMMANDS.len() + 2);
        assert!(shown[1].starts_with("look "));
        assert!(shown.iter().any(|line| line.starts_with("go <place> ")
            && line.ends_with("Move to a host through a route.")));
    }

    #[test]
    fn help_for_a_command_shows_its_names_and_an_example() {
        assert_eq!(
            run("help go"),
            [
                "go <place>",
                "Move to a host through a route.",
                "Other names: cd, connect, ssh",
                "Example: go market",
            ]
        );
    }

    #[test]
    fn help_finds_a_command_by_any_name() {
        assert_eq!(run("help cd"), run("help go"));
    }

    #[test]
    fn man_is_another_name_for_help() {
        assert_eq!(run("man go"), run("help go"));
    }

    #[test]
    fn help_explains_pipelines() {
        assert_eq!(run("help pipes")[0], "Pipelines");
    }

    #[test]
    fn help_suggests_a_near_command() {
        assert_eq!(run("help lok"), ["No help for lok. Did you mean look?"]);
    }

    #[test]
    fn help_for_an_unknown_word_points_to_help() {
        assert_eq!(
            run("help xyzzy"),
            ["No help for xyzzy. Type help to see the commands."]
        );
    }

    #[test]
    fn help_lines_can_be_filtered() {
        let shown = run("help | grep rig");
        assert_eq!(shown.len(), 3);
        assert!(shown.iter().all(|line| line.contains("rig")));
    }

    #[test]
    fn question_mark_lists_what_works_here() {
        assert_eq!(
            run("?"),
            ["Here you can use: look, ls, go, read, download, inventory, status, grep, say, help"]
        );
    }

    #[test]
    fn question_mark_adds_back_after_a_move() {
        assert_eq!(
            walk(&["go back-room", "?"]).0,
            ["Here you can use: look, ls, go, back, inventory, status, grep, say, help"]
        );
    }
}
