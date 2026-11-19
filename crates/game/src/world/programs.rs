use ttymud_text::{Kind, Message, Style};

use super::run::Run;
use super::{PlayerId, Step, Thing, World, choose, usage};
use crate::commands::{self, Command, Item, Program};

const CRACK: u32 = 15;
const SCRUB: u32 = 15;

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
        let cost = commands::cost(found.iter().filter_map(|command| command.verb.program()));
        let (cost, have) = (cost.cycles, state.rig.cycles);
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

    pub(super) fn charge(&mut self, player: PlayerId, ran: Vec<Program>) -> Vec<Message> {
        let cost = commands::cost(ran);
        self.spend(player, cost.cycles);
        self.noise(player, cost.noise)
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
            step.items.push(Item::Ice {
                name: ice.name.clone(),
                weak: false,
            });
        }
        if step.shown.is_empty() {
            step.shown.push(Message::game("scan finds no ICE here."));
        }
        Ok(step)
    }

    pub(super) fn scrub(&mut self, player: PlayerId) -> Result<Step, Message> {
        let Some(run) = self
            .players
            .get_mut(&player)
            .and_then(|state| state.run.as_mut())
        else {
            return Ok(Step::default());
        };
        if run.trace == 0 {
            return Err(Message::error("There is no trace to scrub."));
        }
        let before = run.trace;
        run.trace = before.saturating_sub(SCRUB);
        let after = run.trace;
        Ok(Step::show(vec![Message::game(format!(
            "scrub deletes your log lines. Trace: {before}% -> {after}%."
        ))]))
    }

    pub(super) fn crack(
        &mut self,
        player: PlayerId,
        args: &[String],
        input: Option<Vec<Item>>,
    ) -> Result<Step, Message> {
        let targets = self.targets(player, "crack", args, input)?;
        let [(name, weak)] = targets.as_slice() else {
            return Err(Message::error("crack takes one ICE at a time."));
        };
        let (strength, point) = if *weak {
            (CRACK * 2, (" in its weak point", " through its weak point"))
        } else {
            (CRACK, ("", ""))
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
        ice.integrity = before.saturating_sub(strength);
        if !ice.broken() {
            let after = ice.integrity;
            let (hit, _) = point;
            return Ok(Step::show(vec![
                Message::game("crack hits ")
                    .span(Style::Ice, name)
                    .text(format!("{hit}. Integrity: {before} -> {after}.")),
            ]));
        }
        let (_, through) = point;
        let mut message = Message::game("crack breaks ")
            .span(Style::Ice, name)
            .text(format!("{through}."));
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
        let targets = self.targets(player, "probe", args, input)?;
        let names: Vec<String> = targets.into_iter().map(|(name, _)| name).collect();
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
            step.items.push(Item::Ice {
                name: ice.name.clone(),
                weak: true,
            });
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
    ) -> Result<Vec<(String, bool)>, Message> {
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
                let targets: Vec<(String, bool)> = items
                    .into_iter()
                    .filter_map(|item| match item {
                        Item::Ice { name, weak } => Some((name, weak)),
                        Item::Route(_) | Item::File(_) | Item::Line(_) => None,
                    })
                    .collect();
                if targets.is_empty() {
                    return Err(Message::error(format!(
                        "{command} needs ICE, but the input has none."
                    )));
                }
                Ok(targets)
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
                .map(|word| choose(Thing::Ice, word, &known).map(|name| (name.to_owned(), false)))
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
            at_gate(&["scan"])[..2],
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
        assert_eq!(at_gate(&["scan | grep war"])[0], "warden");
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
        assert_eq!(shown.len(), 3, "{shown:?}");
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
    fn crack_after_probe_hits_the_weak_point() {
        assert_eq!(
            at_gate(&["scan", "~2", "probe warden | crack"])[0],
            "crack hits warden in its weak point. Integrity: 40 -> 10."
        );
        assert_eq!(
            at_gate(&["scan", "~2", "probe sentry | crack"])[0],
            "crack breaks sentry through its weak point."
        );
    }

    #[test]
    fn a_fast_combo_stops_an_alert() {
        let shown = at_gate(&["scan", "~2", "probe sentry | crack", "~5"]);
        assert!(
            !shown.iter().any(|line| line.contains("ALERT")),
            "{shown:?}"
        );
    }

    #[test]
    fn the_weak_point_works_only_in_the_same_pipeline() {
        assert_eq!(
            at_gate(&["scan", "~2", "probe warden", "~3", "crack warden"])[0],
            "crack hits warden. Integrity: 40 -> 25."
        );
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
        assert_eq!(
            last(&at_gate(&lines)),
            Some("ICE: sentry (broken), warden, 1 unknown. Type scan to see them.")
        );
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
            at_gate(&lines)[0],
            "crack breaks warden. The route to vault is open."
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
            at_gate(&["scan | crack"])[0],
            "crack takes one ICE at a time."
        );
        assert_eq!(at_gate(&["status | grep cycles"]), ["Cycles: 4/4"]);
        assert_eq!(
            at_gate(&["scan | crack", "status | grep cycles"]),
            ["Cycles: 2/4"]
        );
    }

    #[test]
    fn scrub_lowers_the_trace_without_noise() {
        let lines = ["scan", "~", "crack sentry", "~3", "scrub"];
        assert_eq!(
            at_gate(&lines),
            ["scrub deletes your log lines. Trace: 10% -> 0%."]
        );
        let prompt = walk(&[&GATE[..], &lines].concat()).1;
        assert_eq!(
            prompt.as_deref(),
            Some("[trace 0% integ 100] ghost@gate $ ")
        );
    }

    #[test]
    fn scrub_needs_a_trace() {
        assert_eq!(at_gate(&["scrub"]), ["There is no trace to scrub."]);
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
