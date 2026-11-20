use ttymud_text::{Message, RunStatus, Status, Style};

use super::builds::Build;
use super::shell::{PROCESS_RAM, Process};
use super::{PlayerId, Step, World, lines, listed};
use crate::commands::{self, Verb};
use crate::content::File;
use crate::heat::MAX_HEAT;
use crate::kits::{CYCLES, RAM};

pub(super) const MAX_INTEGRITY: u32 = 100;
const FREE_REPAIR: u32 = 50;
const REPAIR_PRICE: u32 = 2;
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
    pub(super) ram: u32,
    pub(super) cpu: u32,
    pub(super) builds: Vec<(Verb, Build)>,
    pub(super) process: Option<Process>,
}

impl Rig {
    pub(super) fn new() -> Self {
        Self {
            integrity: MAX_INTEGRITY,
            cycles: CYCLES,
            files: Vec::new(),
            loads: LOADS.to_vec(),
            ram: RAM,
            cpu: CYCLES,
            builds: Vec::new(),
            process: None,
        }
    }

    pub(super) fn ram_line(&self) -> String {
        format!("RAM: {}/{} GB", self.used(), self.ram)
    }

    pub(super) fn used(&self) -> u32 {
        let programs: u32 = commands::programs()
            .filter(|(command, _)| self.loads.contains(&command.verb))
            .map(|(_, program)| program.ram)
            .sum();
        programs + self.process.as_ref().map_or(0, |_| PROCESS_RAM)
    }

    pub(super) fn loaded(&self) -> Vec<String> {
        commands::programs()
            .filter(|(command, _)| self.loads.contains(&command.verb))
            .map(|(command, _)| self.label(command.verb))
            .collect()
    }

    pub(super) fn tick(&mut self) {
        self.cycles = (self.cycles + 1).min(self.cpu);
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
        rig.integrity = (before + rig.power(Verb::Patch)).min(MAX_INTEGRITY);
        let after = rig.integrity;
        Ok(Step::show(vec![Message::game(format!(
            "patch repairs your rig. Integrity: {before} -> {after}."
        ))]))
    }

    pub(super) fn repair(&mut self, player: PlayerId) -> Result<Step, Message> {
        self.at_home(player, "repair your rig")?;
        let Some(state) = self.players.get_mut(&player) else {
            return Ok(Step::default());
        };
        let before = state.rig.integrity;
        if before == MAX_INTEGRITY {
            return Ok(Step::show(vec![Message::game("Your rig needs no repair.")]));
        }
        let price = format!("Above {FREE_REPAIR} integrity, each point costs {REPAIR_PRICE} cr.");
        let free = before.max(FREE_REPAIR);
        let after = (free + state.credits / REPAIR_PRICE).min(MAX_INTEGRITY);
        if after == before {
            let credits = state.credits;
            return Err(Message::error(format!("{price} You have {credits} cr.")));
        }
        let cost = (after - free) * REPAIR_PRICE;
        state.credits -= cost;
        state.rig.integrity = after;
        let repaired = format!("You repair your rig. Integrity: {before} -> {after}.");
        let mut shown = vec![Message::game(match cost {
            0 => repaired,
            cost => format!("{repaired} It costs {cost} cr."),
        })];
        if after < MAX_INTEGRITY {
            let more = (MAX_INTEGRITY - after) * REPAIR_PRICE;
            shown.push(Message::game(format!(
                "A full repair needs {more} cr more. {price}"
            )));
        }
        Ok(Step::show(shown))
    }

    pub(super) fn status(&self, player: PlayerId) -> Step {
        let Some(state) = self.players.get(&player) else {
            return Step::default();
        };
        let mut shown = Vec::new();
        if let Some(run) = &state.run {
            let network = &self.networks[run.network.0].name;
            shown.push(Message::game(format!("Trace: {}% in {network}", run.trace)));
            if let Some(alert) = self.network_alert(run.network) {
                shown.push(Message::game(format!("Alert: {}", alert.word())));
            }
            let access = self.access(player).word();
            shown.push(Message::game(format!("Access here: {access}")));
        }
        shown.push(Message::game(format!("Heat: {}/{MAX_HEAT}", state.heat)));
        shown.push(Message::game(format!("Credits: {} cr", state.credits)));
        let rig = &state.rig;
        shown.push(Message::game(format!(
            "Integrity: {}/{MAX_INTEGRITY}",
            rig.integrity
        )));
        shown.push(Message::game(format!("Cycles: {}/{}", rig.cycles, rig.cpu)));
        shown.push(Message::game(rig.ram_line()));
        let loaded = rig.loaded();
        shown.push(listed(
            Message::game("Programs: "),
            Style::Command,
            loaded.iter().map(String::as_str),
        ));
        lines(shown)
    }

    #[must_use]
    pub fn status_of(&self, player: PlayerId) -> Option<Status> {
        self.handle(player)?;
        let state = self.players.get(&player)?;
        let run = state.run.as_ref().map(|run| RunStatus {
            network: self.networks[run.network.0].name.clone(),
            access: self.access(player).word().to_owned(),
            trace: run.trace,
            alert: self
                .network_alert(run.network)
                .map_or_default(|alert| alert.word().to_owned()),
        });
        let rig = &state.rig;
        Some(Status {
            host: self.hosts[self.here(player).0].id.clone(),
            run,
            integrity: rig.integrity,
            heat: state.heat,
            heat_max: MAX_HEAT,
            ram_used: rig.used(),
            ram_total: rig.ram,
            cycles: rig.cycles,
            cycles_max: rig.cpu,
            credits: state.credits,
            headline: self.headline(),
            crew: self.mates(player),
        })
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
    fn status_of_gives_the_state_for_a_status_bar() {
        use ttymud_text::Status;
        let (mut world, player) = joined();
        let expected = Status {
            host: "relay".to_owned(),
            run: None,
            integrity: 100,
            heat: 0,
            heat_max: 5,
            ram_used: 8,
            ram_total: 8,
            cycles: 4,
            cycles_max: 4,
            credits: 0,
            headline: String::new(),
            crew: Vec::new(),
        };
        assert_eq!(world.status_of(player), Some(expected));
        for line in ["go back-room", "go gate"] {
            world.command(player, line);
        }
        let run = world.status_of(player).and_then(|status| status.run);
        assert_eq!(
            run.map(|run| (run.network, run.access, run.alert)),
            Some(("Test Lab".to_owned(), "guest".to_owned(), String::new()))
        );
        let (naming, _) = world.connect("a");
        assert_eq!(world.status_of(naming), None);
    }

    #[test]
    fn status_of_gives_the_alert_of_the_corp_and_the_latest_headline() {
        let (mut world, player) = crate::testing::hired_at(20_000 * 86_400 + 12 * 3_600);
        crate::testing::enter_job(&mut world, player);
        let corp = world.run_corp(player).unwrap_or_default();
        world.raise_alert(corp, 60);
        world.publish("Old news.".to_owned());
        world.publish("New news.".to_owned());
        let status = world.status_of(player).unwrap_or_default();
        assert_eq!(status.run.map(|run| run.alert).as_deref(), Some("orange"));
        assert_eq!(status.headline, "New news.");
    }

    #[test]
    fn status_of_lists_the_crewmates_in_your_copy_and_their_traces() {
        use ttymud_text::Mate;
        let mut world = crate::testing::world();
        let (nyx, _) = world.join("nyx");
        let (zer0, _) = world.join("zer0");
        world.command(nyx, "crew create Moths");
        world.command(nyx, "crew invite zer0");
        world.command(zer0, "crew join moths");
        for player in [nyx, zer0] {
            for line in ["go back-room", "go gate"] {
                world.command(player, line);
            }
        }
        world.command(zer0, "scan");
        let crew = world.status_of(nyx).map(|status| status.crew);
        let mate = Mate {
            name: "zer0".to_owned(),
            trace: 4,
        };
        assert_eq!(crew, Some(vec![mate]));
        let lone = world.status_of(zer0).map(|status| status.crew.len());
        assert_eq!(lone, Some(1));
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

    fn damaged(integrity: u32, credits: u32) -> (World, PlayerId) {
        let (mut world, player) = joined();
        world.command(player, "go back-room");
        if let Some(state) = world.players.get_mut(&player) {
            state.rig.integrity = integrity;
            state.credits = credits;
        }
        (world, player)
    }

    #[test]
    fn a_repair_to_50_integrity_is_free() {
        let (mut world, player) = damaged(20, 0);
        assert_eq!(
            plain(&world.command(player, "repair")),
            [
                "You repair your rig. Integrity: 20 -> 50.",
                "A full repair needs 100 cr more. Above 50 integrity, each point costs 2 cr.",
            ]
        );
    }

    #[test]
    fn each_point_above_50_integrity_costs_2_credits() {
        let (mut world, player) = damaged(80, 100);
        assert_eq!(
            plain(&world.command(player, "repair")),
            ["You repair your rig. Integrity: 80 -> 100. It costs 40 cr."]
        );
        assert_eq!(
            plain(&world.command(player, "status | grep credits")),
            ["Credits: 60 cr"]
        );
    }

    #[test]
    fn a_repair_uses_the_credits_that_you_have() {
        let (mut world, player) = damaged(40, 31);
        assert_eq!(
            plain(&world.command(player, "repair")),
            [
                "You repair your rig. Integrity: 40 -> 65. It costs 30 cr.",
                "A full repair needs 70 cr more. Above 50 integrity, each point costs 2 cr.",
            ]
        );
    }

    #[test]
    fn without_credits_a_rig_above_50_integrity_stays_as_it_is() {
        let (mut world, player) = damaged(80, 1);
        assert_eq!(
            plain(&world.command(player, "repair")),
            ["Above 50 integrity, each point costs 2 cr. You have 1 cr."]
        );
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
                "Credits: 0 cr",
                "Integrity: 100/100",
                "Cycles: 4/4",
                "RAM: 8/8 GB",
                "Programs: scan, probe, crack, scrub, patch"
            ]
        );
    }

    #[test]
    fn a_faster_cpu_holds_more_cycles() {
        let (mut world, player) = joined();
        if let Some(state) = world.players.get_mut(&player) {
            state.rig.cpu = 6;
            state.rig.cycles = 0;
        }
        for _ in 0..7 {
            world.tick();
        }
        assert_eq!(cycles(&mut world, player), ["Cycles: 6/6"]);
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
