use ttymud_text::{Message, Style};

use super::{PlayerId, Step, World, lines, listed};
use crate::commands::{self, Verb};
use crate::content::File;
use crate::heat::MAX_HEAT;

pub(super) const MAX_INTEGRITY: u32 = 100;
const MAX_CYCLES: u32 = 4;
pub(super) const RAM: u32 = 8;
const PATCH: u32 = 20;
const LOADS: [Verb; 5] = [
    Verb::Scan,
    Verb::Probe,
    Verb::Crack,
    Verb::Scrub,
    Verb::Patch,
];

#[derive(Debug)]
pub(super) struct Rig {
    pub(super) integrity: u32,
    pub(super) cycles: u32,
    pub(super) files: Vec<File>,
    pub(super) loads: Vec<Verb>,
}

impl Rig {
    pub(super) fn new() -> Self {
        Self {
            integrity: MAX_INTEGRITY,
            cycles: MAX_CYCLES,
            files: Vec::new(),
            loads: LOADS.to_vec(),
        }
    }

    pub(super) fn used(&self) -> u32 {
        commands::programs()
            .filter(|(command, _)| self.loads.contains(&command.verb))
            .map(|(_, program)| program.ram)
            .sum()
    }

    pub(super) fn loaded(&self) -> Vec<&'static str> {
        commands::programs()
            .filter(|(command, _)| self.loads.contains(&command.verb))
            .map(|(command, _)| command.name())
            .collect()
    }

    pub(super) fn tick(&mut self) {
        self.cycles = (self.cycles + 1).min(MAX_CYCLES);
    }
}

impl World {
    pub(super) fn spend(&mut self, player: PlayerId, cycles: u32) {
        if let Some(state) = self.players.get_mut(&player) {
            state.rig.cycles = state.rig.cycles.saturating_sub(cycles);
        }
    }

    pub(super) fn patch(&mut self, player: PlayerId) -> Result<Step, Message> {
        let Some(rig) = self.players.get_mut(&player).map(|state| &mut state.rig) else {
            return Ok(Step::default());
        };
        let before = rig.integrity;
        if before == MAX_INTEGRITY {
            return Err(Message::error("Your rig needs no patch."));
        }
        rig.integrity = (before + PATCH).min(MAX_INTEGRITY);
        let after = rig.integrity;
        Ok(Step::show(vec![Message::game(format!(
            "patch repairs your rig. Integrity: {before} -> {after}."
        ))]))
    }

    pub(super) fn repair(&mut self, player: PlayerId) -> Result<Step, Message> {
        self.at_home(player, "repair your rig")?;
        let Some(rig) = self.players.get_mut(&player).map(|state| &mut state.rig) else {
            return Ok(Step::default());
        };
        let before = rig.integrity;
        if before == MAX_INTEGRITY {
            return Ok(Step::show(vec![Message::game("Your rig needs no repair.")]));
        }
        rig.integrity = MAX_INTEGRITY;
        Ok(Step::show(vec![Message::game(format!(
            "You repair your rig. Integrity: {before} -> {MAX_INTEGRITY}."
        ))]))
    }

    pub(super) fn status(&self, player: PlayerId) -> Step {
        let Some(state) = self.players.get(&player) else {
            return Step::default();
        };
        let mut shown = Vec::new();
        if let Some(run) = &state.run {
            let network = &self.networks[run.network.0].name;
            shown.push(Message::game(format!("Trace: {}% in {network}", run.trace)));
        }
        shown.push(Message::game(format!("Heat: {}/{MAX_HEAT}", state.heat)));
        let rig = &state.rig;
        shown.push(Message::game(format!(
            "Integrity: {}/{MAX_INTEGRITY}",
            rig.integrity
        )));
        shown.push(Message::game(format!(
            "Cycles: {}/{MAX_CYCLES}",
            rig.cycles
        )));
        shown.push(Message::game(format!("RAM: {}/{RAM} GB", rig.used())));
        shown.push(listed(
            Message::game("Programs: "),
            Style::Command,
            rig.loaded(),
        ));
        lines(shown)
    }
}

#[cfg(test)]
mod tests {
    use crate::testing::{joined, plain, run, walk};
    use crate::{PlayerId, World};

    fn cycles(world: &mut World, player: PlayerId) -> Vec<String> {
        plain(&world.command(player, "status | grep cycles"))
    }

    #[test]
    fn each_tick_gives_back_one_cycle_up_to_the_maximum() {
        let (mut world, player) = joined();
        if let Some(state) = world.players.get_mut(&player) {
            state.rig.cycles = 2;
        }
        assert_eq!(cycles(&mut world, player), ["Cycles: 2/4"]);
        world.tick();
        assert_eq!(cycles(&mut world, player), ["Cycles: 3/4"]);
        world.tick();
        world.tick();
        assert_eq!(cycles(&mut world, player), ["Cycles: 4/4"]);
    }

    #[test]
    fn repair_at_home_restores_full_integrity() {
        let lines = ["go back-room", "go gate", "jackout", "repair"];
        assert_eq!(
            walk(&lines).0,
            ["You repair your rig. Integrity: 80 -> 100."]
        );
        let lines = [&lines[..], &["status | grep integrity"]].concat();
        assert_eq!(walk(&lines).0, ["Integrity: 100/100"]);
    }

    #[test]
    fn patch_repairs_some_integrity_during_a_run() {
        let (mut world, player) = joined();
        for line in ["go back-room", "go gate"] {
            world.command(player, line);
        }
        assert_eq!(
            plain(&world.command(player, "patch")),
            ["Your rig needs no patch."]
        );
        if let Some(state) = world.players.get_mut(&player) {
            state.rig.integrity = 90;
        }
        assert_eq!(
            plain(&world.command(player, "patch")),
            ["patch repairs your rig. Integrity: 90 -> 100."]
        );
        assert_eq!(run("patch"), ["patch works only in a target network."]);
    }

    #[test]
    fn repair_works_only_at_home() {
        assert_eq!(
            run("repair"),
            ["You can repair your rig only at back-room."]
        );
    }

    #[test]
    fn a_rig_at_full_integrity_needs_no_repair() {
        assert_eq!(
            walk(&["go back-room", "repair"]).0,
            ["Your rig needs no repair."]
        );
    }

    #[test]
    fn status_shows_the_rig() {
        assert_eq!(
            run("status"),
            [
                "Heat: 0/5",
                "Integrity: 100/100",
                "Cycles: 4/4",
                "RAM: 8/8 GB",
                "Programs: scan, probe, crack, scrub, patch"
            ]
        );
    }

    #[test]
    fn st_is_short_for_status() {
        assert_eq!(run("st"), run("status"));
    }

    #[test]
    fn status_lines_can_be_filtered() {
        assert_eq!(run("status | grep cycles"), ["Cycles: 4/4"]);
    }
}
