use ttymud_text::{Kind, Message, Style};

use super::run::Run;
use super::{PlayerId, Step, Thing, World, choose, usage};
use crate::commands::{self, Command, Item};

const CRACK: u32 = 15;

impl World {
    pub(super) fn ready(&self, player: PlayerId, found: &[&Command]) -> Result<(), Message> {
        let programs: Vec<&str> = found
            .iter()
            .filter(|command| command.verb.program().is_some())
            .map(|command| command.name())
            .collect();
        let (Some(first), Some(state)) = (programs.first(), self.players.get(&player)) else {
            return Ok(());
        };
        if state.run.is_none() {
            return Err(Message::error(format!(
                "{first} works only in a target network."
            )));
        }
        let (cost, have) = (commands::cost(found), state.rig.cycles);
        if have >= cost {
            return Ok(());
        }
        let what = if programs.len() == 1 {
            first
        } else {
            "This pipeline"
        };
        Err(Message::error(format!(
            "{what} needs {cost} cycles, but you have {have}. You get 1 cycle on each tick."
        )))
    }

    pub(super) fn scan(&mut self, player: PlayerId) -> Result<Step, Message> {
        let here = self.here(player);
        let run = self
            .players
            .get_mut(&player)
            .and_then(|state| state.run.as_mut())
            .ok_or_else(|| Message::error("scan works only in a target network."))?;
        let mut step = Step::default();
        for ice in run.ice.iter_mut().filter(|ice| ice.host == here) {
            ice.known = true;
            let mut line = ice.describe();
            if ice.broken() {
                step.shown.push(line);
                continue;
            }
            if let Some(target) = ice.guards {
                line = line
                    .text(", guards ")
                    .span(Style::Host, &self.hosts[target.0].id);
            }
            step.shown.push(line);
            step.items.push(Item::Ice(ice.name.clone()));
        }
        if step.shown.is_empty() {
            step.shown.push(Message::game("scan finds no ICE here."));
        }
        Ok(step)
    }

    pub(super) fn crack(
        &mut self,
        player: PlayerId,
        args: &[String],
        input: Option<Vec<Item>>,
    ) -> Result<Step, Message> {
        let names = self.targets(player, "crack", args, input)?;
        let [name] = names.as_slice() else {
            return Err(Message::error("crack takes one ICE at a time."));
        };
        let here = self.here(player);
        let ice = self
            .players
            .get_mut(&player)
            .and_then(|state| state.run.as_mut())
            .and_then(|run| {
                run.ice
                    .iter_mut()
                    .find(|ice| ice.host == here && &ice.name == name)
            });
        let Some(ice) = ice else {
            return Ok(Step::default());
        };
        if ice.broken() {
            return Err(Message::new(Kind::Error)
                .span(Style::Ice, name)
                .text(" is already broken."));
        }
        let before = ice.integrity;
        ice.integrity = before.saturating_sub(CRACK);
        if !ice.broken() {
            let after = ice.integrity;
            return Ok(Step::show(vec![
                Message::game("crack hits ")
                    .span(Style::Ice, name)
                    .text(format!(". Integrity: {before} -> {after}.")),
            ]));
        }
        let mut message = Message::game("crack breaks ")
            .span(Style::Ice, name)
            .text(".");
        if let Some(target) = ice.guards {
            message = message
                .text(" The route to ")
                .span(Style::Host, &self.hosts[target.0].id)
                .text(" is open.");
        }
        Ok(Step::show(vec![message]))
    }

    pub(super) fn probe(
        &self,
        player: PlayerId,
        args: &[String],
        input: Option<Vec<Item>>,
    ) -> Result<Step, Message> {
        let names = self.targets(player, "probe", args, input)?;
        let here = self.here(player);
        let mut step = Step::default();
        let Some(run) = self.run_of(player) else {
            return Ok(step);
        };
        for ice in run
            .ice
            .iter()
            .filter(|ice| ice.host == here && names.contains(&ice.name))
        {
            if ice.broken() {
                step.shown.push(ice.describe().text("."));
                continue;
            }
            let weakness = format!(". Weak point: {}.", ice.kind.weakness());
            step.shown.push(ice.describe().text(weakness));
            step.items.push(Item::Ice(ice.name.clone()));
        }
        Ok(step)
    }

    fn run_of(&self, player: PlayerId) -> Option<&Run> {
        self.players.get(&player)?.run.as_ref()
    }

    fn targets(
        &self,
        player: PlayerId,
        command: &str,
        args: &[String],
        input: Option<Vec<Item>>,
    ) -> Result<Vec<String>, Message> {
        let here = self.here(player);
        let known: Vec<&str> = self
            .run_of(player)
            .into_iter()
            .flat_map(|run| &run.ice)
            .filter(|ice| ice.host == here && ice.known)
            .map(|ice| ice.name.as_str())
            .collect();
        match (args, input) {
            ([], Some(items)) => {
                let names: Vec<String> = items
                    .into_iter()
                    .filter_map(|item| match item {
                        Item::Ice(name) => Some(name),
                        Item::Route(_) | Item::File(_) | Item::Line(_) => None,
                    })
                    .collect();
                if names.is_empty() {
                    return Err(Message::error(format!(
                        "{command} needs ICE, but the input has none."
                    )));
                }
                Ok(names)
            }
            (_, Some(_)) => Err(Message::error(format!(
                "{command} takes an ICE or a pipe, not both."
            ))),
            ([], None) => Err(usage(&format!("{command} <ice>"), Thing::Ice, &known)),
            ([word, ..], None) if known.is_empty() => {
                Err(Message::error(format!("No ICE named {word}. Type "))
                    .span(Style::Command, "scan")
                    .text(" to find ICE."))
            }
            (words, None) => words
                .iter()
                .map(|word| choose(Thing::Ice, word, &known).map(str::to_owned))
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::testing::{run, walk};

    const GATE: [&str; 2] = ["go back-room", "go gate"];
    const UNKNOWN: &str = "ICE: 2 unknown. Type scan to see them.";

    fn at_gate(lines: &[&str]) -> Vec<String> {
        walk(&[&GATE[..], lines].concat()).0
    }

    fn last(lines: &[String]) -> Option<&str> {
        lines.last().map(String::as_str)
    }

    #[test]
    fn look_counts_the_ice_that_you_do_not_know() {
        assert_eq!(last(&at_gate(&["look"])), Some(UNKNOWN));
    }

    #[test]
    fn scan_shows_the_ice_on_this_host() {
        assert_eq!(
            at_gate(&["scan"]),
            [
                "sentry: Sentry v1, integrity 20/20",
                "warden: Warden v1, integrity 40/40, guards vault",
            ]
        );
    }

    #[test]
    fn look_names_the_ice_after_a_scan() {
        assert_eq!(
            last(&at_gate(&["scan", "look"])),
            Some("ICE: sentry, warden")
        );
    }

    #[test]
    fn scan_pipes_the_ice() {
        assert_eq!(at_gate(&["scan | grep war"]), ["warden"]);
    }

    #[test]
    fn scan_without_ice_says_so() {
        assert_eq!(at_gate(&["go core", "scan"]), ["scan finds no ICE here."]);
    }

    #[test]
    fn a_program_costs_its_cycles_and_one_for_the_setup() {
        assert_eq!(at_gate(&["scan", "status | grep cycles"]), ["Cycles: 2/4"]);
    }

    #[test]
    fn a_program_needs_enough_cycles() {
        assert_eq!(
            at_gate(&["scan", "scan", "scan"]),
            ["scan needs 2 cycles, but you have 0. You get 1 cycle on each tick."]
        );
    }

    #[test]
    fn probe_finds_the_weak_point_of_an_ice() {
        assert_eq!(
            at_gate(&["scan", "probe sentry"]),
            ["sentry: Sentry v1, integrity 20/20. Weak point: it trusts old signatures."]
        );
    }

    #[test]
    fn probe_takes_the_ice_from_a_scan() {
        let shown = at_gate(&["scan | probe"]);
        assert_eq!(shown.len(), 2, "{shown:?}");
        assert!(shown[1].starts_with("warden: Warden v1"), "{shown:?}");
    }

    #[test]
    fn probe_needs_ice_that_you_know() {
        assert_eq!(
            at_gate(&["probe sentry"]),
            ["No ICE named sentry. Type scan to find ICE."]
        );
        assert_eq!(
            at_gate(&["scan", "probe"]),
            ["Usage: probe <ice>. ICE: sentry, warden"]
        );
    }

    #[test]
    fn a_program_that_fails_costs_nothing() {
        let shown = at_gate(&["probe sentry", "status | grep cycles"]);
        assert_eq!(shown, ["Cycles: 4/4"]);
    }

    #[test]
    fn crack_hits_an_ice() {
        assert_eq!(
            at_gate(&["scan", "~", "crack warden"]),
            ["crack hits warden. Integrity: 40 -> 25."]
        );
    }

    #[test]
    fn crack_breaks_an_ice_at_zero_integrity() {
        let lines = ["scan", "~", "crack sentry", "~3", "crack sentry"];
        assert_eq!(at_gate(&lines), ["crack breaks sentry."]);
        let lines = [&lines[..], &["look"]].concat();
        assert_eq!(last(&at_gate(&lines)), Some("ICE: sentry (broken), warden"));
    }

    #[test]
    fn a_broken_warden_opens_its_route() {
        let lines = [
            "scan",
            "~",
            "crack warden",
            "~3",
            "crack warden",
            "~3",
            "crack warden",
        ];
        assert_eq!(
            at_gate(&lines),
            ["crack breaks warden. The route to vault is open."]
        );
        let lines = [&lines[..], &["go vault"]].concat();
        assert_eq!(at_gate(&lines)[0], "Lab Vault");
    }

    #[test]
    fn a_broken_ice_cannot_be_cracked_again() {
        let lines = [
            "scan",
            "~",
            "crack sentry",
            "~3",
            "crack sentry",
            "~3",
            "crack sentry",
        ];
        assert_eq!(at_gate(&lines), ["sentry is already broken."]);
    }

    #[test]
    fn crack_takes_one_ice_at_a_time() {
        assert_eq!(
            at_gate(&["scan | crack"]),
            ["crack takes one ICE at a time."]
        );
        assert_eq!(at_gate(&["status | grep cycles"]), ["Cycles: 4/4"]);
        assert_eq!(
            at_gate(&["scan | crack", "status | grep cycles"]),
            ["Cycles: 2/4"]
        );
    }

    #[test]
    fn scan_works_only_in_a_target_network() {
        assert_eq!(run("scan"), ["scan works only in a target network."]);
    }

    #[test]
    fn a_new_run_starts_with_unknown_ice() {
        assert_eq!(last(&at_gate(&["scan", "back", "go gate"])), Some(UNKNOWN));
    }
}
