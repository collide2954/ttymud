use ttymud_text::sanitize;

use super::{PlayerId, World, help};
use crate::commands::{self, COMMANDS, Command, Verb};
use crate::parse::{self, Tail};

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Completion {
    pub word: String,
    pub names: Vec<String>,
}

impl World {
    #[must_use]
    pub fn complete(&self, player: PlayerId, text: &str) -> Completion {
        if !self.players.contains_key(&player) {
            return Completion::default();
        }
        let Some(Tail { words, word }) = parse::tail(&sanitize(text)) else {
            return Completion::default();
        };
        let start = word.to_lowercase();
        let names = self
            .choices(player, &words, !word.is_empty())
            .into_iter()
            .filter(|name| name.starts_with(&start))
            .map(str::to_owned)
            .collect();
        Completion { word, names }
    }

    fn choices(&self, player: PlayerId, words: &[String], every: bool) -> Vec<&str> {
        let Some(first) = words.first() else {
            return command_names(every);
        };
        let Some(command) = commands::find(first) else {
            return Vec::new();
        };
        let here = self.here(player);
        match (command.verb, words.len()) {
            (Verb::Go, 1) => self.route_names(here),
            (Verb::Help, 1) => {
                let mut names = command_names(every);
                names.extend(help::TOPICS);
                names
            }
            (Verb::Read, _) => self
                .readable(player)
                .into_iter()
                .map(|file| file.name.as_str())
                .collect(),
            (Verb::Download, _) => self.hosts[here.0]
                .files
                .iter()
                .map(|file| file.name.as_str())
                .collect(),
            (Verb::Probe | Verb::Crack, _) => self
                .players
                .get(&player)
                .and_then(|state| state.run.as_ref())
                .into_iter()
                .flat_map(|run| &run.ice)
                .filter(|ice| ice.host == here && ice.known && !ice.broken())
                .map(|ice| ice.name.as_str())
                .collect(),
            (Verb::Contract, 1) => vec!["drop"],
            _ => Vec::new(),
        }
    }
}

fn command_names(every: bool) -> Vec<&'static str> {
    if every {
        commands::names().collect()
    } else {
        COMMANDS.iter().map(Command::name).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::Completion;
    use crate::commands::COMMANDS;
    use crate::testing::joined;

    fn complete(lines: &[&str], text: &str) -> Completion {
        let (mut world, player) = joined();
        for line in lines {
            world.command(player, line);
        }
        world.complete(player, text)
    }

    fn names(text: &str) -> Vec<String> {
        complete(&[], text).names
    }

    fn main_names() -> Vec<String> {
        COMMANDS
            .iter()
            .map(|command| command.name().to_owned())
            .collect()
    }

    #[test]
    fn the_first_word_completes_to_command_names() {
        assert_eq!(names("lo"), ["look"]);
        assert_eq!(names("c"), ["cd", "connect", "cat", "contract", "crack"]);
    }

    #[test]
    fn an_empty_line_offers_the_main_name_of_each_command() {
        assert_eq!(names(""), main_names());
    }

    #[test]
    fn go_completes_one_route_here() {
        assert_eq!(names("go ba"), ["back-room", "bazaar"]);
        assert_eq!(names("cd m"), ["market"]);
        assert_eq!(names("go market "), Vec::<String>::new());
    }

    #[test]
    fn read_completes_files_here_and_on_the_rig() {
        let completion = complete(&["get motd.txt", "go market"], "read ");
        assert_eq!(completion.names, ["prices.db", "motd.txt"]);
    }

    #[test]
    fn download_completes_files_on_this_host() {
        assert_eq!(names("get r"), ["rules.txt"]);
        let completion = complete(&["get motd.txt", "go market"], "get ");
        assert_eq!(completion.names, ["prices.db"]);
    }

    #[test]
    fn help_completes_commands_and_topics() {
        assert_eq!(names("help pi"), ["pipes", "pipelines"]);
        assert_eq!(names("man gr"), ["grep"]);
    }

    #[test]
    fn probe_and_crack_complete_the_known_ice_here() {
        let gate = ["go back-room", "go gate"];
        assert_eq!(complete(&gate, "probe ").names, Vec::<String>::new());
        let scanned = [&gate[..], &["scan"]].concat();
        assert_eq!(complete(&scanned, "crack w").names, ["warden"]);
        assert_eq!(
            complete(&scanned, "scan | probe ").names,
            ["sentry", "warden"]
        );
    }

    #[test]
    fn contract_completes_drop() {
        assert_eq!(names("contract d"), ["drop"]);
        assert_eq!(names("job "), ["drop"]);
    }

    #[test]
    fn a_pipe_starts_a_new_command() {
        assert_eq!(names("ls | gr"), ["grep"]);
        assert_eq!(names("ls|"), main_names());
    }

    #[test]
    fn the_word_comes_back_as_typed() {
        assert_eq!(
            complete(&[], "GO MA"),
            Completion {
                word: "MA".to_owned(),
                names: vec!["market".to_owned()],
            }
        );
    }

    #[test]
    fn chat_quotes_and_unknown_commands_complete_nothing() {
        for text in ["say he", "'he", "read \"mo", "xyzzy a", "ls || gr"] {
            assert_eq!(names(text), Vec::<String>::new(), "{text}");
        }
    }
}
