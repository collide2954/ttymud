use ttymud_text::{Message, Style};

use super::heat::FAST_HEAT;
use super::ice::{Ice, deploy};
use super::{HostId, NetworkId, PlayerId, Step, World};
use crate::contract::Event;
use crate::heat::MAX_HEAT;
use crate::ice::{Action, IceKind};
use crate::target::Level;

const SHOCK: u32 = 20;
const LATE_TRACE: u32 = 75;
pub(super) const MAX_TRACE: u32 = 100;
const CRASH: &str = "Your rig crashes, and you lose the loot of this run.";

#[derive(Debug)]
pub(super) struct Run {
    pub(super) network: NetworkId,
    pub(super) start: usize,
    pub(super) trace: u32,
    pub(super) loot: Vec<String>,
    pub(super) ice: Vec<Ice>,
    pub(super) firewall: bool,
    pub(super) mask: u32,
    pub(super) proxy: bool,
    pub(super) access: Level,
    pub(super) shells: Vec<(HostId, Level)>,
}

impl World {
    pub(super) fn may_enter(&self, player: PlayerId, target: HostId) -> Result<(), Message> {
        let Some(state) = self.players.get(&player) else {
            return Ok(());
        };
        let entering = state.run.is_none() && self.hosts[target.0].network.is_some();
        if !entering || state.rig.integrity > 0 {
            return Ok(());
        }
        let message = Message::error("Your rig has no integrity left.");
        Err(match self.home {
            Some(home) => message
                .text(" Type ")
                .span(Style::Command, "repair")
                .text(" at ")
                .span(Style::Host, &self.hosts[home.0].id)
                .text("."),
            None => message,
        })
    }

    pub(super) fn jackout(&mut self, player: PlayerId) -> Result<Step, Message> {
        let Some(state) = self.players.get_mut(&player) else {
            return Ok(Step::default());
        };
        let Some(run) = &state.run else {
            return Err(Message::error("You are not in a target network."));
        };
        let name = &self.networks[run.network.0].name;
        let late = run.trace >= LATE_TRACE;
        let mut shown = vec![Message::game(format!(
            "You jack out of {name}. The shock costs your rig {SHOCK} integrity."
        ))];
        if late {
            let heat = self.add_heat(player, 1);
            shown.push(Message::game("The trace was 75% or more. ").text(heat.plain()));
        }
        let mut after = self.damage(player, SHOCK);
        if self.running(player) {
            after.extend(self.drop_link(player, false));
        }
        shown.extend(self.burn_if_hot(player, after));
        Ok(Step::show(shown))
    }

    fn burn_if_hot(&mut self, player: PlayerId, otherwise: Vec<Message>) -> Vec<Message> {
        if self.heat_of(player) >= MAX_HEAT {
            self.burn(player)
        } else {
            otherwise
        }
    }

    pub(super) fn damage(&mut self, player: PlayerId, amount: u32) -> Vec<Message> {
        let Some(state) = self.players.get_mut(&player) else {
            return Vec::new();
        };
        state.rig.integrity = state.rig.integrity.saturating_sub(amount);
        if state.rig.integrity > 0 {
            return Vec::new();
        }
        let mut shown = vec![Message::game(CRASH)];
        shown.extend(self.drop_link(player, true));
        shown
    }

    pub(super) fn running(&self, player: PlayerId) -> bool {
        self.players
            .get(&player)
            .is_some_and(|state| state.run.is_some())
    }

    pub(super) fn noise(&mut self, player: PlayerId, noise: u32) -> Vec<Message> {
        if noise == 0 {
            return Vec::new();
        }
        let here = self.here(player);
        let alert = if self.heat_of(player) >= FAST_HEAT {
            Action::Alert.ticks() - 1
        } else {
            Action::Alert.ticks()
        };
        let Some(run) = self
            .players
            .get_mut(&player)
            .and_then(|state| state.run.as_mut())
        else {
            return Vec::new();
        };
        let masked = run.mask > 0;
        let mut shown = Vec::new();
        let mut watched = false;
        for ice in run
            .ice
            .iter_mut()
            .filter(|ice| ice.host == here && !ice.broken() && !masked)
        {
            match ice.kind {
                IceKind::Sentry => {
                    watched = true;
                    if ice.intent.is_none() {
                        shown.push(ice.compile_in(Action::Alert, alert));
                    }
                }
                IceKind::Scorch if !ice.awake => shown.push(ice.wake()),
                _ => {}
            }
        }
        let gain = match (masked, watched) {
            (true, _) => noise / 2,
            (false, true) => noise * 2,
            (false, false) => noise,
        };
        shown.extend(self.raise_trace(player, gain));
        shown
    }

    pub(super) fn raise_trace(&mut self, player: PlayerId, amount: u32) -> Vec<Message> {
        let here = self.here(player);
        let Some(run) = self
            .players
            .get_mut(&player)
            .and_then(|state| state.run.as_mut())
        else {
            return Vec::new();
        };
        run.trace = (run.trace + amount).min(MAX_TRACE);
        if run.trace < MAX_TRACE {
            return Vec::new();
        }
        let network = &self.networks[run.network.0];
        let security = self.hosts[here.0].security.unwrap_or(network.security);
        let heat = security.heat();
        let name = &network.name;
        let mut shown = vec![Message::game(format!(
            "{name} traces you. The link drops, and you lose the loot of this run."
        ))];
        shown.push(self.add_heat(player, heat));
        let scene = self.drop_link(player, true);
        shown.extend(self.burn_if_hot(player, scene));
        shown
    }

    fn drop_link(&mut self, player: PlayerId, lose_loot: bool) -> Vec<Message> {
        let from = self.here(player);
        if let Some(state) = self.players.get_mut(&player)
            && let Some(run) = state.run.take()
        {
            state.chain.truncate(run.start);
            if lose_loot {
                state
                    .rig
                    .files
                    .retain(|file| !run.loot.contains(&file.name));
            }
        }
        let here = self.here(player);
        self.relocate(player, from, here);
        self.scene(here, player)
    }

    pub(super) fn cross(&mut self, player: PlayerId, from: HostId, to: HostId) -> Vec<Message> {
        let Some(state) = self.players.get_mut(&player) else {
            return Vec::new();
        };
        match (self.hosts[from.0].network, self.hosts[to.0].network) {
            (None, Some(network)) => {
                state.run = Some(Run {
                    network,
                    start: state.chain.len() - 1,
                    trace: 0,
                    loot: Vec::new(),
                    ice: deploy(&self.hosts, network),
                    firewall: false,
                    mask: 0,
                    proxy: false,
                    access: Level::Guest,
                    shells: Vec::new(),
                });
                let name = &self.networks[network.0].name;
                let shown = vec![Message::game(format!(
                    "You connect to the network of {name}."
                ))];
                if self.is_job(player, network) {
                    self.advance(player, Event::Enter);
                }
                shown
            }
            (Some(network), None) => {
                state.run = None;
                let name = &self.networks[network.0].name;
                let exit = Message::game(format!("You leave the network of {name}. Clean exit."));
                if self.is_job(player, network) {
                    self.advance(player, Event::Exit);
                }
                std::iter::once(exit)
                    .chain(self.finish_contract(player, network))
                    .collect()
            }
            _ => Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::testing::{joined, plain, run, walk};
    use crate::{PlayerId, World};

    const LOOT: [&str; 4] = ["go back-room", "go gate", "go core", "get data.db"];

    fn damaged(integrity: u32) -> (World, PlayerId) {
        let (mut world, player) = joined();
        if let Some(state) = world.players.get_mut(&player) {
            state.rig.integrity = integrity;
        }
        (world, player)
    }

    fn prompt(lines: &[&str]) -> Option<String> {
        walk(&[&["go back-room", "go gate"][..], lines].concat()).1
    }

    #[test]
    fn noise_raises_the_trace() {
        assert_eq!(
            prompt(&["go core", "scan"]).as_deref(),
            Some("[trace 2% integ 100] ghost@core $ ")
        );
    }

    #[test]
    fn a_working_sentry_doubles_the_trace_from_noise() {
        assert_eq!(
            prompt(&["scan"]).as_deref(),
            Some("[trace 4% integ 100] ghost@gate $ ")
        );
    }

    #[test]
    fn a_pipeline_makes_less_noise_than_its_parts() {
        assert_eq!(
            prompt(&["scan | probe"]).as_deref(),
            Some("[trace 6% integ 100] ghost@gate $ ")
        );
        assert_eq!(
            prompt(&["scan", "~3", "probe sentry warden"]).as_deref(),
            Some("[trace 8% integ 100] ghost@gate $ ")
        );
    }

    #[test]
    fn a_broken_sentry_stops_watching() {
        let lines = [
            "scan",
            "~",
            "crack sentry",
            "~3",
            "crack sentry",
            "~2",
            "scan",
        ];
        assert_eq!(
            prompt(&lines).as_deref(),
            Some("[trace 15% integ 100] ghost@gate $ ")
        );
    }

    #[test]
    fn a_full_trace_ends_the_run_and_loses_the_loot() {
        let (mut world, player) = joined();
        for line in LOOT {
            world.command(player, line);
        }
        if let Some(run) = world
            .players
            .get_mut(&player)
            .and_then(|state| state.run.as_mut())
        {
            run.trace = 98;
        }
        let shown = plain(&world.command(player, "scan"));
        assert_eq!(
            shown[..4],
            [
                "scan finds no ICE here.",
                "Test Lab traces you. The link drops, and you lose the loot of this run.",
                "Your heat: 0 -> 2.",
                "The Back Room",
            ]
        );
        assert_eq!(
            plain(&world.command(player, "inventory")),
            ["Your rig has no files."]
        );
    }

    #[test]
    fn jackout_drops_the_link_at_once() {
        let (shown, prompt) = walk(&["go back-room", "go gate", "go core", "jackout"]);
        assert_eq!(
            shown[..2],
            [
                "You jack out of Test Lab. The shock costs your rig 20 integrity.",
                "The Back Room",
            ]
        );
        assert_eq!(prompt.as_deref(), Some("ghost@back-room $ "));
    }

    #[test]
    fn jackout_keeps_the_loot_but_costs_integrity() {
        let lines = [&LOOT[..], &["jackout", "inventory"]].concat();
        assert_eq!(walk(&lines).0, ["data.db"]);
        let lines = [
            "go back-room",
            "go gate",
            "jackout",
            "status | grep integrity",
        ];
        assert_eq!(walk(&lines).0, ["Integrity: 80/100"]);
    }

    #[test]
    fn jackout_needs_a_run() {
        assert_eq!(run("jackout"), ["You are not in a target network."]);
    }

    #[test]
    fn a_jackout_without_enough_integrity_crashes_the_rig() {
        let (mut world, player) = damaged(15);
        for line in LOOT {
            world.command(player, line);
        }
        let shown = plain(&world.command(player, "jackout"));
        assert_eq!(
            shown[1..3],
            [
                "Your rig crashes, and you lose the loot of this run.",
                "The Back Room",
            ]
        );
        assert_eq!(
            plain(&world.command(player, "inventory")),
            ["Your rig has no files."]
        );
    }

    #[test]
    fn a_rig_without_integrity_cannot_connect() {
        let (mut world, player) = damaged(0);
        world.command(player, "go back-room");
        assert_eq!(
            plain(&world.command(player, "go gate")),
            ["Your rig has no integrity left. Type repair at back-room."]
        );
        assert_eq!(world.prompt(player).as_deref(), Some("ghost@back-room $ "));
    }

    #[test]
    fn entering_a_network_starts_a_run() {
        let (shown, prompt) = walk(&["go back-room", "go gate"]);
        assert_eq!(
            shown[..2],
            ["You connect to the network of Test Lab.", "Lab Gate"]
        );
        assert_eq!(
            prompt.as_deref(),
            Some("[trace 0% integ 100] ghost@gate $ ")
        );
    }

    #[test]
    fn status_shows_the_trace_during_a_run() {
        let (shown, _) = walk(&["go back-room", "go gate", "status"]);
        assert_eq!(shown[0], "Trace: 0% in Test Lab");
        assert_eq!(walk(&["status"]).0[0], "Heat: 0/5");
    }

    #[test]
    fn the_run_goes_on_inside_the_network() {
        let (shown, prompt) = walk(&["go back-room", "go gate", "go core"]);
        assert_eq!(shown[0], "Lab Core");
        assert_eq!(
            prompt.as_deref(),
            Some("[trace 0% integ 100] ghost@core $ ")
        );
    }

    #[test]
    fn leaving_through_the_entry_is_a_clean_exit() {
        let (shown, prompt) = walk(&["go back-room", "go gate", "back"]);
        assert_eq!(
            shown[..2],
            [
                "You leave the network of Test Lab. Clean exit.",
                "The Back Room"
            ]
        );
        assert_eq!(prompt.as_deref(), Some("ghost@back-room $ "));
    }

    #[test]
    fn going_to_the_hub_by_name_is_a_clean_exit_too() {
        let lines = ["go back-room", "go gate", "go core", "go gate", "go back"];
        let (shown, _) = walk(&lines);
        assert_eq!(shown[0], "You leave the network of Test Lab. Clean exit.");
    }
}
