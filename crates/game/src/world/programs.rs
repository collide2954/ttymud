use ttymud_text::{Kind, Message, Style};

use super::instance::InstanceId;
use super::presence::named;
use super::{HostId, Output, PlayerId, Step, Thing, World, choose, usage};
use crate::commands::{self, Command, Item, Program, Verb};
use crate::content::File;
use crate::contract::Event;

#[derive(Debug)]
pub(super) enum Target {
    Ice { name: String, weak: bool },
    Rig { name: String, weak: bool },
}

impl World {
    pub(super) fn ready(&self, player: PlayerId, found: &[&Command]) -> Result<(), Message> {
        self.allowed(player, found)?;
        let programs: Vec<&str> = found
            .iter()
            .filter(|command| command.verb.program().is_some())
            .map(|command| command.name())
            .collect();
        let (Some(first), Some(state)) = (programs.first(), self.players.get(&player)) else {
            return Ok(());
        };
        let (cost, have) = (self.cycles_for(player, found), state.rig.cycles);
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

    pub(super) fn allowed(&self, player: PlayerId, found: &[&Command]) -> Result<(), Message> {
        let first = found
            .iter()
            .find(|command| command.verb.program().is_some());
        let (Some(first), Some(state)) = (first, self.players.get(&player)) else {
            return Ok(());
        };
        let missing = found.iter().find(|command| {
            command.verb.program().is_some() && !state.rig.loads.contains(&command.verb)
        });
        if let Some(command) = missing {
            return Err(self.not_loaded(command.name()));
        }
        let Some(run) = &state.run else {
            let first = first.name();
            return Err(Message::error(format!(
                "{first} works only in a target network."
            )));
        };
        if let Some((verb, left)) = run.frozen
            && found.iter().any(|command| command.verb == verb)
        {
            let (name, unit) = (
                commands::name_of(verb),
                if left == 1 { "tick" } else { "ticks" },
            );
            return Err(Message::error(format!(
                "Your {name} is frozen for {left} more {unit}."
            )));
        }
        Ok(())
    }

    pub(super) fn cycles_for(&self, player: PlayerId, found: &[&Command]) -> u32 {
        let Some(state) = self.players.get(&player) else {
            return 0;
        };
        let programs = found
            .iter()
            .filter_map(|command| state.rig.program(command.verb));
        commands::cost(programs).cycles
    }

    pub(super) fn charge(&mut self, player: PlayerId, ran: Vec<Program>) -> Vec<Message> {
        let cost = commands::cost(ran);
        self.spend(player, cost.cycles);
        self.noise(player, cost.noise)
    }

    pub(super) fn scan(&mut self, player: PlayerId) -> Result<Step, Message> {
        let here = self.here(player);
        let id = self
            .instance_id(player)
            .ok_or_else(|| Message::error("scan works only in a target network."))?;
        let mut step = Step::default();
        let ice = self
            .instances
            .get_mut(&id)
            .into_iter()
            .flat_map(|instance| instance.ice.iter_mut());
        for ice in ice.filter(|ice| ice.host == here) {
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
        step.shown.extend(self.software_line(player));
        step.shown.extend(self.soc_line(player));
        step.shown.extend(self.vault_lines(player));
        Ok(step)
    }

    pub(super) fn scrub(&mut self, player: PlayerId, args: &[String]) -> Result<Step, Message> {
        let here = self.here(player);
        let Some(id) = self.instance_id(player) else {
            return Ok(Step::default());
        };
        let mate = match args {
            [] => None,
            [word] => {
                let names = self.mates_here(player);
                let found = choose(Thing::Member, word, &names)?;
                let members = self.members(id);
                let mate = members.iter().find(|member| member.name == found);
                mate.map(|member| (member.player, member.name.clone()))
            }
            _ => return Err(Message::error("Usage: scrub [handle].")),
        };
        let whom = mate.as_ref().map_or(player, |&(mate, _)| mate);
        let scrub = self.power(player, Verb::Scrub);
        let Some((before, after)) = self.wipe(id, (here, whom), scrub) else {
            return Err(Message::error(match &mate {
                None => "There is no trace to scrub.".to_owned(),
                Some((_, name)) => format!("{name} has no trace to scrub."),
            }));
        };
        let Some((mate, name)) = mate else {
            return Ok(Step::show(vec![Message::game(format!(
                "scrub deletes your log lines. Trace: {before}% -> {after}%."
            ))]));
        };
        let me = self.handle(player).unwrap_or_default().to_owned();
        let told = format!(" scrubs your log lines. Trace: {before}% -> {after}%.");
        self.outbox.push(Output {
            player: mate,
            message: named(&me, &told),
        });
        Ok(Step::show(vec![
            Message::game("scrub deletes the log lines of ")
                .span(Style::Player, &name)
                .text(format!(". Trace of {name}: {before}% -> {after}%.")),
        ]))
    }

    fn wipe(
        &mut self,
        id: InstanceId,
        (here, whom): (HostId, PlayerId),
        scrub: u32,
    ) -> Option<(u32, u32)> {
        let run = self.players.get_mut(&whom)?.run.as_mut()?;
        if run.trace == 0 {
            return None;
        }
        let before = run.trace;
        run.trace = before.saturating_sub(scrub);
        let after = run.trace;
        if let Some(instance) = self.instances.get_mut(&id) {
            instance
                .logs
                .retain(|&(host, owner, _)| host != here || owner != whom);
        }
        Some((before, after))
    }

    pub(super) fn crack(
        &mut self,
        player: PlayerId,
        args: &[String],
        input: Option<Vec<Item>>,
    ) -> Result<Step, Message> {
        let targets = self.targets(player, "crack", args, input)?;
        let (name, weak) = match targets.as_slice() {
            [Target::Ice { name, weak }] => (name, *weak),
            [Target::Rig { name, weak }] => return Ok(self.crack_rig(player, name, *weak)),
            _ => return Err(Message::error("crack takes one target at a time.")),
        };
        let crack = self.power(player, Verb::Crack);
        let (strength, point) = if weak {
            (crack * 2, (" in its weak point", " through its weak point"))
        } else {
            (crack, ("", ""))
        };
        let here = self.here(player);
        let ice = self.instance_id(player).and_then(|id| {
            self.instances.get_mut(&id).and_then(|instance| {
                instance
                    .ice
                    .iter_mut()
                    .find(|ice| ice.host == here && &ice.name == name)
            })
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
        self.advance(player, Event::Break);
        Ok(Step::show(vec![message]))
    }

    pub(super) fn probe(
        &mut self,
        player: PlayerId,
        args: &[String],
        input: Option<Vec<Item>>,
    ) -> Result<Step, Message> {
        if let Some(files) = self.probed_files(player, args, input.as_deref()) {
            return Ok(probe_files(&files));
        }
        let targets = self.targets(player, "probe", args, input)?;
        let mut step = Step::default();
        for target in targets {
            match target {
                Target::Ice { name, .. } => self.probe_ice(player, name, &mut step),
                Target::Rig { name, .. } => self.probe_rig(player, name, &mut step),
            }
        }
        Ok(step)
    }

    fn probed_files(
        &self,
        player: PlayerId,
        args: &[String],
        input: Option<&[Item]>,
    ) -> Option<Vec<File>> {
        let files: Vec<File> = if let Some(items) = input {
            let piped = items.iter().filter_map(|item| match item {
                Item::File(file) => Some(file.clone()),
                Item::Route(_) | Item::Ice { .. } | Item::Rig { .. } | Item::Line(_) => None,
            });
            piped.collect()
        } else {
            let here = self.files_on(self.here(player), player);
            let found: Vec<File> = args
                .iter()
                .filter_map(|arg| here.iter().find(|file| file.name == *arg))
                .map(|&file| file.clone())
                .collect();
            if found.len() < args.len() {
                return None;
            }
            found
        };
        (!files.is_empty()).then_some(files)
    }

    fn probe_ice(&self, player: PlayerId, name: String, step: &mut Step) {
        let here = self.here(player);
        let Some(ice) = self.instance(player).and_then(|instance| {
            instance
                .ice
                .iter()
                .find(|ice| ice.host == here && ice.name == name)
        }) else {
            return;
        };
        if ice.broken() {
            step.shown.push(ice.describe().text("."));
            return;
        }
        let weakness = format!(". Weak point: {}.", ice.kind.weakness());
        step.shown.push(ice.describe().text(weakness));
        step.items.push(Item::Ice { name, weak: true });
    }

    pub(super) fn targets(
        &self,
        player: PlayerId,
        command: &str,
        args: &[String],
        input: Option<Vec<Item>>,
    ) -> Result<Vec<Target>, Message> {
        let here = self.here(player);
        let known: Vec<&str> = self
            .instance(player)
            .into_iter()
            .flat_map(|instance| &instance.ice)
            .filter(|ice| ice.host == here && ice.known)
            .map(|ice| ice.name.as_str())
            .collect();
        match (args, input) {
            ([], Some(items)) => {
                let targets: Vec<Target> = items
                    .into_iter()
                    .filter_map(|item| match item {
                        Item::Ice { name, weak } => Some(Target::Ice { name, weak }),
                        Item::Rig { name, weak } => Some(Target::Rig { name, weak }),
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
            (words, None) if self.pvp(player) => self.aim(player, command, words, &known),
            ([], None) => Err(usage(&format!("{command} <ice>"), Thing::Ice, &known)),
            ([word, ..], None) if known.is_empty() => {
                Err(Message::error(format!("No ICE named {word}. Type "))
                    .span(Style::Command, "scan")
                    .text(" to find ICE."))
            }
            (words, None) => words
                .iter()
                .map(|word| {
                    let name = choose(Thing::Ice, word, &known)?.to_owned();
                    Ok(Target::Ice { name, weak: false })
                })
                .collect(),
        }
    }
}

fn probe_files(files: &[File]) -> Step {
    let shown = files
        .iter()
        .map(|file| {
            let verdict = if file.bait {
                " is a honeypot. Do not touch it."
            } else {
                " is a real file."
            };
            Message::game("")
                .span(Style::File, &file.name)
                .text(verdict)
        })
        .collect();
    Step::show(shown)
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
            "crack takes one target at a time."
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
        let shown = at_gate(&["scan", "back", "go gate"]);
        assert!(shown.iter().any(|line| line == UNKNOWN), "{shown:?}");
    }
}
