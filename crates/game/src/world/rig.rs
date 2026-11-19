use ttymud_text::{Message, Style};

use super::{PlayerId, Step, World, lines, listed};
use crate::commands;
use crate::content::File;

const MAX_INTEGRITY: u32 = 100;
const MAX_CYCLES: u32 = 4;
const RAM: u32 = 8;

#[derive(Debug)]
pub(super) struct Rig {
    pub(super) integrity: u32,
    pub(super) cycles: u32,
    pub(super) files: Vec<File>,
}

impl Rig {
    pub(super) fn new() -> Self {
        Self {
            integrity: MAX_INTEGRITY,
            cycles: MAX_CYCLES,
            files: Vec::new(),
        }
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

    pub(super) fn repair(&mut self, player: PlayerId) -> Result<Step, Message> {
        let Some(home) = self.home else {
            return Err(Message::error("This city has no place to repair a rig."));
        };
        if self.here(player) != home {
            return Err(Message::error("You can repair your rig only at ")
                .span(Style::Host, &self.hosts[home.0].id)
                .text("."));
        }
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
        let rig = &state.rig;
        shown.push(Message::game(format!(
            "Integrity: {}/{MAX_INTEGRITY}",
            rig.integrity
        )));
        shown.push(Message::game(format!(
            "Cycles: {}/{MAX_CYCLES}",
            rig.cycles
        )));
        let used: u32 = commands::programs().map(|(_, program)| program.ram).sum();
        shown.push(Message::game(format!("RAM: {used}/{RAM} GB")));
        let loaded = commands::programs().map(|(command, _)| command.name());
        shown.push(listed(Message::game("Programs: "), Style::Command, loaded));
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
        let lines = [&lines[..], &["status"]].concat();
        assert_eq!(walk(&lines).0[0], "Integrity: 100/100");
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
                "Integrity: 100/100",
                "Cycles: 4/4",
                "RAM: 6/8 GB",
                "Programs: scan, probe, crack, scrub"
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
