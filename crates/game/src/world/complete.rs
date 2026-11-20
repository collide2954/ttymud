use ttymud_text::sanitize;

use super::crews::ACTIONS;
use super::{PlayerId, World, help, topics};
use crate::commands::{self, Command, Verb};
use crate::parse::{self, Tail};

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Completion {
    pub word: String,
    pub names: Vec<String>,
}

impl World {
    #[must_use]
    pub fn complete(&self, player: PlayerId, text: &str) -> Completion {
        if self.handle(player).is_none() {
            return Completion::default();
        }
        let Some(Tail { words, word }) = parse::tail(&sanitize(text)) else {
            return Completion::default();
        };
        let start = word.to_lowercase();
        let names = self
            .choices(player, &words, !word.is_empty())
            .into_iter()
            .filter(|name| name.to_lowercase().starts_with(&start))
            .map(str::to_owned)
            .collect();
        Completion { word, names }
    }

    fn choices(&self, player: PlayerId, words: &[String], every: bool) -> Vec<&str> {
        let moderator = self.is_moderator(player);
        let Some(first) = words.first() else {
            return command_names(every, moderator);
        };
        let Some(command) = commands::find(first) else {
            return Vec::new();
        };
        let here = self.here(player);
        match (command.verb, words.len()) {
            (Verb::Go, 1) => self.route_names(here, player),
            (Verb::Help, 1) => {
                let mut names = command_names(every, moderator);
                names.extend(topics::TOPICS);
                names
            }
            (Verb::Read, _) => self
                .readable(player)
                .into_iter()
                .map(|file| file.name.as_str())
                .collect(),
            (Verb::Download, _) => self
                .files_on(here, player)
                .into_iter()
                .map(|file| file.name.as_str())
                .collect(),
            (Verb::Sh, 1) | (Verb::Rm, _) => self.script_names(player),
            (Verb::Echo, _) if matches!(words.last().map(String::as_str), Some(">" | ">>")) => {
                self.script_names(player)
            }
            (Verb::Probe | Verb::Crack | Verb::Pretext, _) => {
                self.targets_here(player, command.verb == Verb::Pretext)
            }
            (Verb::Contract, 1) => vec!["drop"],
            (Verb::Crew, 1) => ACTIONS.to_vec(),
            (Verb::Scrub, 1) => self.mates_here(player),
            (Verb::Tip, 1) => self
                .tip_targets(player)
                .into_iter()
                .map(|(_, name)| name)
                .collect(),
            (Verb::Duel, 1) => self.duel_names(player),
            (Verb::Match, 1) => self.crew_names(player),
            (Verb::Crew, 2) => self.crew_choices(player, &words[1]),
            (Verb::Tell | Verb::Ignore | Verb::Block | Verb::Report, 1) => self.online_names(),
            (Verb::Unignore | Verb::Unblock, 1) => {
                self.listed_people(player, command.verb == Verb::Unblock)
            }
            (Verb::Join | Verb::Leave, 1) => {
                self.channel_choices(player, command.verb == Verb::Leave)
            }
            (Verb::Buy, 1) => self.ware_names(player),
            (Verb::Sell, _) => self
                .players
                .get(&player)
                .into_iter()
                .flat_map(|state| &state.rig.files)
                .filter(|file| file.value > 0)
                .map(|file| file.name.as_str())
                .collect(),
            (Verb::Stash | Verb::Unstash, 1) => {
                let Some(state) = self.players.get(&player) else {
                    return Vec::new();
                };
                let files = if command.verb == Verb::Stash {
                    &state.rig.files
                } else {
                    &state.stash
                };
                files.iter().map(|file| file.name.as_str()).collect()
            }
            (Verb::Compile, 1) => self
                .known_programs(player)
                .into_iter()
                .map(|(_, name)| name)
                .collect(),
            (Verb::Compile, 2) => vec!["quiet", "plain"],
            (Verb::Load | Verb::Unload, 1) => {
                let loads = self
                    .players
                    .get(&player)
                    .map_or(&[][..], |state| state.rig.loads.as_slice());
                let unload = command.verb == Verb::Unload;
                self.known_programs(player)
                    .into_iter()
                    .filter(|(verb, _)| loads.contains(verb) == unload)
                    .map(|(_, name)| name)
                    .collect()
            }
            _ => Vec::new(),
        }
    }
}

impl World {
    fn targets_here(&self, player: PlayerId, analysts: bool) -> Vec<&str> {
        let here = self.here(player);
        let mut names: Vec<&str> = self
            .instance(player)
            .into_iter()
            .flat_map(|instance| &instance.ice)
            .filter(|ice| ice.host == here && ice.known && !ice.broken())
            .map(|ice| ice.name.as_str())
            .collect();
        if analysts {
            names.extend(self.analyst_names(player));
            names.extend(self.employee_here(player));
        } else {
            names.extend(self.rivals(player).into_iter().map(|(_, name)| name));
        }
        names
    }

    fn listed_people(&self, player: PlayerId, blocked: bool) -> Vec<&str> {
        let Some(state) = self.players.get(&player) else {
            return Vec::new();
        };
        let keys = if blocked {
            &state.social.blocks
        } else {
            &state.social.ignores
        };
        keys.iter()
            .filter_map(|key| self.person_label(key))
            .collect()
    }

    fn channel_choices(&self, player: PlayerId, leaving: bool) -> Vec<&str> {
        let joined = self
            .players
            .get(&player)
            .map_or(&[][..], |state| state.channels.as_slice());
        self.channels
            .iter()
            .filter(|channel| joined.contains(&channel.id) == leaving)
            .map(|channel| channel.id.as_str())
            .collect()
    }
}

fn command_names(every: bool, moderator: bool) -> Vec<&'static str> {
    let shown = help::shown_to(moderator);
    if every {
        shown
            .flat_map(|command| command.names.iter().copied())
            .collect()
    } else {
        shown.map(Command::name).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::Completion;
    use crate::testing::joined;
    use crate::world::help::shown_to;

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
        shown_to(false)
            .map(|command| command.name().to_owned())
            .collect()
    }

    #[test]
    fn the_first_word_completes_to_command_names() {
        assert_eq!(names("lo"), ["look", "load", "lots"]);
        assert_eq!(
            names("c"),
            [
                "cd", "connect", "cat", "compile", "contract", "crack", "channels", "crew"
            ]
        );
    }

    #[test]
    fn sh_rm_and_echo_complete_the_scripts_on_the_rig() {
        let lines = ["echo ls > sweep.sh", "echo ls > scan2.sh", "get motd.txt"];
        assert_eq!(complete(&lines, "sh sw").names, ["sweep.sh"]);
        assert_eq!(complete(&lines, "rm ").names, ["sweep.sh", "scan2.sh"]);
        assert_eq!(
            complete(&lines, "echo look >> s").names,
            ["sweep.sh", "scan2.sh"]
        );
        assert_eq!(
            complete(&lines, "sh sweep.sh s").names,
            Vec::<String>::new()
        );
        assert_eq!(complete(&lines, "echo s").names, Vec::<String>::new());
    }

    #[test]
    fn compile_completes_the_programs_and_the_mods() {
        assert_eq!(names("compile cr"), ["crack"]);
        assert_eq!(names("compile crack "), ["quiet", "plain"]);
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
    fn stash_completes_rig_files_and_unstash_completes_stash_files() {
        let lines = [
            "get motd.txt",
            "get rules.txt",
            "go back-room",
            "stash rules.txt",
        ];
        assert_eq!(complete(&lines, "stash ").names, ["motd.txt"]);
        assert_eq!(complete(&lines, "unstash ").names, ["rules.txt"]);
    }

    #[test]
    fn buy_completes_the_wares_and_sell_the_stolen_data() {
        let (mut world, player) = joined();
        world.command(player, "go market");
        let mut payroll = crate::content::File::new("payroll.db", vec!["Pay.".to_owned()]);
        payroll.value = 40;
        if let Some(state) = world.players.get_mut(&player) {
            state.rig.files.push(crate::content::File::new(
                "memo.txt",
                vec!["Hi.".to_owned()],
            ));
            state.rig.files.push(payroll);
        }
        assert_eq!(
            world.complete(player, "buy ").names,
            ["crack-v2", "ram", "cpu"]
        );
        assert_eq!(world.complete(player, "sell ").names, ["payroll.db"]);
        assert_eq!(complete(&[], "buy ").names, Vec::<String>::new());
    }

    #[test]
    fn tell_completes_the_handles_online_and_join_the_channels() {
        let (mut world, player) = joined();
        world.join("nyx");
        world.join("nora");
        assert_eq!(world.complete(player, "tell n").names, ["nyx", "nora"]);
        assert_eq!(
            world.complete(player, "tell nyx h").names,
            Vec::<String>::new()
        );
        assert_eq!(world.complete(player, "join t").names, ["trade"]);
        assert_eq!(world.complete(player, "leave ").names, ["city"]);
    }

    #[test]
    fn ignore_completes_handles_and_unignore_completes_the_list() {
        let (mut world, player) = joined();
        world.join("nyx");
        world.join("nora");
        assert_eq!(world.complete(player, "block n").names, ["nyx", "nora"]);
        world.command(player, "ignore nora");
        assert_eq!(world.complete(player, "unignore ").names, ["nora"]);
        assert_eq!(
            world.complete(player, "unblock ").names,
            Vec::<String>::new()
        );
    }

    #[test]
    fn crew_completes_its_actions_and_the_handles_for_them() {
        let (mut world, player) = joined();
        let (nyx, _) = world.join("nyx");
        assert_eq!(world.complete(player, "crew l").names, ["leave", "lead"]);
        assert_eq!(world.complete(player, "crew invite n").names, ["nyx"]);
        world.command(nyx, "crew create Moths");
        world.command(nyx, "crew invite ghost");
        assert_eq!(world.complete(player, "crew join ").names, ["Moths"]);
        assert_eq!(world.complete(player, "crew join m").names, ["Moths"]);
        world.command(player, "crew join moths");
        assert_eq!(world.complete(nyx, "crew kick ").names, ["ghost"]);
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
        for text in ["say he", "'he", "read \"mo", "xyzzy a", "ls | | gr"] {
            assert_eq!(names(text), Vec::<String>::new(), "{text}");
        }
        assert_eq!(names("ls || gr"), names("gr"));
    }
}
