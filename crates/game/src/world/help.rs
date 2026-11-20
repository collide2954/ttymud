use ttymud_text::{Kind, Message, Style};

use super::topics::{self, TOPICS};
use super::{PlayerId, Step, World, hint, lines, listed};
use crate::commands::{self, COMMANDS, Command, Verb};

const COLUMN: usize = 18;

pub(super) fn help(args: &[String], moderator: bool) -> Result<Step, Message> {
    let shown = match args {
        [] => overview(moderator),
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
        let state = self.players.get(&player);
        let hops = state.map_or(0, |state| state.chain.len());
        let running = state.is_some_and(|state| state.run.is_some());
        let hired = state.is_some_and(|state| state.contract.is_some());
        let readable = !self.readable(player).is_empty();
        let names = shown_to(self.is_moderator(player))
            .filter(|command| match command.verb {
                Verb::Go => !self.routes(here, player).is_empty(),
                Verb::Back => hops > 1,
                Verb::Jackout => running,
                Verb::Tip => self.in_darknet(player),
                Verb::Duel => self.arena.is_some() && !running,
                Verb::Match => {
                    let name = self.handle(player).unwrap_or_default();
                    self.arena.is_some() && !running && self.led_crew(name).is_ok()
                }
                verb if verb.program().is_some() => {
                    running && state.is_some_and(|state| state.rig.loads.contains(&verb))
                }
                Verb::Repair | Verb::Load | Verb::Unload | Verb::Compile | Verb::Unstash => {
                    self.home == Some(here)
                }
                Verb::Contract => hired,
                Verb::Reply => state.is_some_and(|state| state.reply_to.is_some()),
                Verb::Unignore => state.is_some_and(|state| !state.social.ignores.is_empty()),
                Verb::Unblock => state.is_some_and(|state| !state.social.blocks.is_empty()),
                Verb::Shop | Verb::Buy | Verb::Sell => self.is_shop(here),
                Verb::Lots | Verb::Bid => self.auction.host == Some(here),
                Verb::Jobs | Verb::Accept | Verb::Rumors => self.has_fixer(here),
                Verb::Read => readable,
                Verb::Rm | Verb::Sh => !self.scripts(player).is_empty(),
                Verb::Ps | Verb::Kill => state.is_some_and(|state| state.rig.process.is_some()),
                Verb::Download => !self.files_on(here, player).is_empty(),
                Verb::Recovery
                | Verb::Link
                | Verb::Keys
                | Verb::Revoke
                | Verb::Wait
                | Verb::Available => false,
                _ => true,
            })
            .map(Command::name);
        let message = Message::game("Here you can use: ");
        lines(vec![listed(message, Style::Command, names)])
    }
}

pub(super) fn shown_to(moderator: bool) -> impl Iterator<Item = &'static Command> {
    COMMANDS
        .iter()
        .filter(move |command| moderator || !command.verb.moderation())
}

fn overview(moderator: bool) -> Vec<Message> {
    let mut shown = vec![Message::new(Kind::Game).span(Style::Title, "Commands")];
    shown.extend(shown_to(moderator).map(|command| {
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
    fn help_lists_every_command_but_the_moderator_commands() {
        let shown = run("help");
        assert_eq!(shown[0], "Commands");
        assert_eq!(shown.len(), super::shown_to(false).count() + 3);
        assert!(super::shown_to(false).count() < COMMANDS.len());
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
    fn help_explains_access() {
        assert_eq!(
            run("help runs | grep access"),
            ["Some files need user or admin access. Read stolen credentials, or run exploit."]
        );
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
            Some(
                "Topics: crews, economy, heat, ice, pipes, pvp, runs, scripts, soc. Example: help ice"
            )
        );
    }

    #[test]
    fn help_scripts_explains_echo_sh_ps_kill_and_wait() {
        let scripts = run("help scripts");
        assert_eq!(scripts[0], "Scripts");
        for word in [
            ".sh",
            "echo 'scan | probe' > sweep.sh",
            ">>",
            "sh sweep.sh sentry",
            "$1 to $9",
            "1 GB of RAM",
            "ps",
            "kill",
            "&&",
            "||",
            "wait 3",
            "#",
        ] {
            assert!(
                scripts.iter().any(|line| line.contains(word)),
                "{word}: {scripts:?}"
            );
        }
        assert_eq!(run("help script"), scripts);
        assert_eq!(run("help sh")[0], "sh <script> [words]");
    }

    #[test]
    fn help_pvp_explains_the_darknet_the_tip_and_the_fights() {
        let pvp = run("help pvp");
        assert_eq!(pvp[0], "Fights between handles");
        for word in [
            "darknet",
            "probe <handle> | crack",
            "tip <handle>",
            "duel <handle>",
            "match <crew>",
        ] {
            assert!(
                pvp.iter().any(|line| line.contains(word)),
                "{word}: {pvp:?}"
            );
        }
        assert_eq!(run("help darknet"), pvp);
        assert_eq!(run("help arena"), pvp);
        assert_eq!(run("help duel")[0], "duel <handle> [credits]");
    }

    #[test]
    fn help_explains_versions_blueprints_prices_and_the_auction() {
        let economy = run("help economy");
        assert_eq!(economy[0], "The economy");
        for word in [
            "blueprint",
            "compile <program>",
            "quiet",
            "sale",
            "lots",
            "bid <lot> <credits>",
        ] {
            assert!(
                economy.iter().any(|line| line.contains(word)),
                "{word}: {economy:?}"
            );
        }
        assert_eq!(run("help auction"), economy);
        assert_eq!(run("help blueprints"), economy);
        assert_eq!(run("help compile")[0], "compile [program] [quiet|plain]");
    }

    #[test]
    fn help_explains_crews_group_runs_and_heists() {
        let crew = run("help crews");
        assert_eq!(crew[0], "Crews and heists");
        for word in [
            "crew invite",
            "#crew",
            "scrub <handle>",
            "decoy",
            "laptop",
            "leader",
        ] {
            assert!(
                crew.iter().any(|line| line.contains(word)),
                "{word}: {crew:?}"
            );
        }
        assert_eq!(run("help heist"), crew);
        assert_eq!(
            run("help crew")[0],
            "crew [create|invite|join|leave|kick|lead]"
        );
    }

    #[test]
    fn help_explains_the_soc_alerts_patches_news_and_rumors() {
        let soc = run("help soc");
        assert_eq!(soc[0], "The SOC");
        for word in [
            "alert level",
            "analyst",
            "pretext",
            "patches",
            "news",
            "rumors",
        ] {
            assert!(
                soc.iter().any(|line| line.contains(word)),
                "{word}: {soc:?}"
            );
        }
        assert_eq!(run("help alert"), soc);
        assert_eq!(run("help patches"), soc);
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
        let expected: Vec<String> = run("help")
            .into_iter()
            .filter(|line| line.to_lowercase().contains("rig"))
            .collect();
        assert!(expected.len() > 1);
        assert_eq!(run("help | grep rig"), expected);
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
            "Here you can use: look, ls, go, read, download, inventory, status, stash, grep, echo, say, channels, join, leave, who, news, crew, tell, ignore, block, report, help"
        );
        check(&available(&["echo ls > a.sh"]), &["rm"], &[]);
    }

    #[test]
    fn question_mark_adds_the_run_commands_during_a_run() {
        let outside = available(&[]);
        let inside = available(&["go back-room", "go gate"]);
        check(&outside, &[], &["jackout", "scan"]);
        check(&inside, &["jackout", "scan", "crack"], &["firewall"]);
    }

    #[test]
    fn question_mark_adds_unignore_unblock_and_reply_when_they_can_act() {
        check(
            &available(&[]),
            &["ignore", "block"],
            &["unignore", "unblock", "reply"],
        );
        let (mut world, ghost) = crate::testing::joined();
        let (nyx, _) = world.join("nyx");
        world.command(nyx, "tell ghost hi");
        for line in ["ignore nyx", "block nyx"] {
            world.command(ghost, line);
        }
        let shown = crate::testing::plain(&world.command(ghost, "?"));
        let names: Vec<String> = shown[0]
            .trim_start_matches("Here you can use: ")
            .split(", ")
            .map(str::to_owned)
            .collect();
        check(&names, &["unignore", "unblock", "reply"], &[]);
    }

    #[test]
    fn the_account_commands_work_only_on_a_server() {
        let server = ["This command works only on a server."];
        for line in ["recovery", "link", "keys", "revoke 2"] {
            assert_eq!(run(line), server, "{line}");
        }
        assert_eq!(run("link now"), ["link takes no arguments."]);
        assert_eq!(
            run("help link")[1],
            "Make a code that adds a new device to your account. On the new device, type link <code> at the handle prompt."
        );
        check(
            &available(&[]),
            &[],
            &["recovery", "link", "keys", "revoke"],
        );
    }

    #[test]
    fn question_mark_adds_the_shop_commands_at_a_shop() {
        check(&available(&[]), &[], &["shop", "buy"]);
        check(&available(&["go market"]), &["shop", "buy"], &[]);
    }

    #[test]
    fn question_mark_adds_the_home_commands_at_home() {
        let home = available(&["go back-room"]);
        check(
            &home,
            &["back", "repair", "load", "unload", "compile", "unstash"],
            &["download"],
        );
    }
}
