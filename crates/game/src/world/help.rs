use ttymud_text::{Kind, Message, Style};

use super::topics::{self, TOPICS};
use super::{PlayerId, Step, World, hint, lines, listed};
use crate::commands::{self, COMMANDS, Command, Verb};

const COLUMN: usize = 18;

pub(super) fn help(args: &[String]) -> Result<Step, Message> {
    let shown = match args {
        [] => overview(),
        [name] => match (topics::topic(&name.to_lowercase()), commands::find(name)) {
            (Some(topic), _) => topic,
            (None, Some(command)) => details(command),
            (None, None) => {
                let message = Message::error(format!("No help for {name}."));
                let known = commands::names().chain(TOPICS.iter().copied());
                return Err(hint(message, name, known));
            }
        },
        _ => return Err(Message::error("Usage: help [command].")),
    };
    Ok(lines(shown))
}

impl World {
    pub(super) fn available(&self, player: PlayerId) -> Step {
        let here = self.here(player);
        let host = &self.hosts[here.0];
        let state = self.players.get(&player);
        let hops = state.map_or(0, |state| state.chain.len());
        let running = state.is_some_and(|state| state.run.is_some());
        let hired = state.is_some_and(|state| state.contract.is_some());
        let readable = !self.readable(player).is_empty();
        let names = COMMANDS
            .iter()
            .filter(|command| match command.verb {
                Verb::Go => !host.routes.is_empty(),
                Verb::Back => hops > 1,
                Verb::Jackout => running,
                verb if verb.program().is_some() => {
                    running && state.is_some_and(|state| state.rig.loads.contains(&verb))
                }
                Verb::Repair | Verb::Load | Verb::Unload | Verb::Unstash => self.home == Some(here),
                Verb::Contract => hired,
                Verb::Shop => self.is_shop(here),
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
    shown.push(topics::index());
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

#[cfg(test)]
mod tests {
    use crate::commands::COMMANDS;
    use crate::testing::{run, walk};

    #[test]
    fn help_lists_every_command() {
        let shown = run("help");
        assert_eq!(shown[0], "Commands");
        assert_eq!(shown.len(), COMMANDS.len() + 3);
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
    fn help_explains_ice_and_runs() {
        assert_eq!(run("help ice")[0], "ICE");
        assert_eq!(run("help runs")[0], "Runs");
        assert_eq!(run("help trace"), run("help runs"));
        assert_eq!(run("HELP ICE"), run("help ice"));
    }

    #[test]
    fn help_explains_heat_and_burns() {
        let heat = run("help heat");
        assert_eq!(heat[0], "Heat");
        assert!(
            heat.contains(
                &"At heat 5, your handle burns. Your account keeps its know-how and its stash."
                    .to_owned()
            ),
            "{heat:?}"
        );
        assert_eq!(run("help burn"), heat);
    }

    #[test]
    fn help_for_repair_gives_the_price() {
        assert_eq!(
            run("help repair")[1],
            "Restore your integrity at home. Up to 50 is free. Above 50, each point costs 2 cr."
        );
    }

    #[test]
    fn help_for_status_names_the_heat() {
        assert_eq!(
            run("help status")[1],
            "Show your heat, trace, integrity, cycles, and RAM."
        );
    }

    #[test]
    fn help_names_the_topics() {
        assert_eq!(
            run("help").last().map(String::as_str),
            Some("Topics: heat, ice, pipes, runs. Example: help ice")
        );
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

    fn available(lines: &[&str]) -> Vec<String> {
        let shown = walk(&[lines, &["?"]].concat()).0;
        shown[0]
            .trim_start_matches("Here you can use: ")
            .split(", ")
            .map(str::to_owned)
            .collect()
    }

    fn check(names: &[String], present: &[&str], absent: &[&str]) {
        for name in present {
            assert!(names.iter().any(|known| known == name), "{name}: {names:?}");
        }
        for name in absent {
            assert!(
                !names.iter().any(|known| known == name),
                "{name}: {names:?}"
            );
        }
    }

    #[test]
    fn question_mark_lists_what_works_here() {
        assert_eq!(
            run("?")[0],
            "Here you can use: look, ls, go, read, download, inventory, status, stash, grep, say, help"
        );
    }

    #[test]
    fn question_mark_adds_the_run_commands_during_a_run() {
        let outside = available(&[]);
        let inside = available(&["go back-room", "go gate"]);
        check(&outside, &[], &["jackout", "scan"]);
        check(&inside, &["jackout", "scan", "crack"], &["firewall"]);
    }

    #[test]
    fn question_mark_adds_the_shop_commands_at_a_shop() {
        check(&available(&[]), &[], &["shop"]);
        check(&available(&["go market"]), &["shop"], &[]);
    }

    #[test]
    fn question_mark_adds_the_home_commands_at_home() {
        let home = available(&["go back-room"]);
        check(
            &home,
            &["back", "repair", "load", "unload", "unstash"],
            &["download"],
        );
    }
}
