use ttymud_text::{Message, Style};

use super::alert::{Alert, TRACE_ALERT};
use super::heat::FAST_HEAT;
use super::ice::{Ice, deploy};
use super::instance::{Instance, InstanceId, decoy_on, name_of, witnesses};
use super::matches::End;
use super::{HostId, NetworkId, Output, PlayerId, Step, World};
use crate::commands::Verb;
use crate::content::Zone;
use crate::contract::Event;
use crate::heat::MAX_HEAT;
use crate::ice::{Action, IceKind};
use crate::target::Level;

const SHOCK: u32 = 20;
const LATE_TRACE: u32 = 75;
const ENTRY_ALERT: u32 = 5;
pub(super) const MAX_TRACE: u32 = 100;
const CRASH: &str = "Your rig crashes, and you lose the loot of this run.";

#[derive(Debug)]
pub(super) struct Run {
    pub(super) instance: InstanceId,
    pub(super) network: NetworkId,
    pub(super) start: usize,
    pub(super) trace: u32,
    pub(super) loot: Vec<String>,
    pub(super) firewall: u32,
    pub(super) mask: u32,
    pub(super) decoy: u32,
    pub(super) proxy: bool,
    pub(super) access: Level,
    pub(super) shells: Vec<(HostId, Level)>,
    pub(super) last: Option<Verb>,
    pub(super) frozen: Option<(Verb, u32)>,
    pub(super) passwords: Vec<String>,
    pub(super) done: bool,
}

impl Run {
    pub(super) fn new(instance: InstanceId, network: NetworkId, start: usize) -> Self {
        Self {
            instance,
            network,
            start,
            trace: 0,
            loot: Vec::new(),
            firewall: 0,
            mask: 0,
            decoy: 0,
            proxy: false,
            access: Level::Guest,
            shells: Vec::new(),
            last: None,
            frozen: None,
            passwords: Vec::new(),
            done: false,
        }
    }
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
        if let Some(shown) = self.quit_match(player, End::GiveUp) {
            return Ok(Step::show(shown));
        }
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
        if late && self.first_job(player).is_none() {
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
        let stopped = self.stop_script(player);
        let mut shown = if let Some(shown) = self.match_crash(player) {
            shown
        } else if let Some(shown) = self.darknet_crash(player) {
            shown
        } else {
            let mut shown = vec![Message::game(CRASH)];
            shown.extend(self.drop_link(player, true));
            shown
        };
        if let Some(stopped) = stopped {
            shown.insert(shown.len().min(1), stopped);
        }
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
        let corp = self.run_corp(player);
        let steps = corp.map_or(0, |corp| self.alert(corp).steps());
        if let Some(corp) = corp {
            self.raise_alert(corp, noise);
        }
        let here = self.here(player);
        let alert = self.alert_ticks(player, Action::Alert.ticks());
        let Some(id) = self.instance_id(player) else {
            return Vec::new();
        };
        let masked = self.masked(player);
        let (target, name, present) = self.audience(id, here, player);
        let Some(instance) = self.instances.get_mut(&id) else {
            return Vec::new();
        };
        if !masked {
            instance.log(here, player, noise);
        }
        let mut told = Vec::new();
        let mut watched = false;
        for ice in instance
            .ice
            .iter_mut()
            .filter(|ice| ice.host == here && !ice.broken() && !masked)
        {
            match ice.kind {
                IceKind::Sentry => {
                    watched = true;
                    if ice.intent.is_none() {
                        alert_on(ice, target, &name, &present, alert, &mut told);
                    }
                }
                kind if kind.local() && !ice.awake => {
                    ice.target = Some(target);
                    let woke = ice.wake();
                    told.extend(present.iter().map(|&other| (other, woke.clone())));
                }
                _ => {}
            }
        }
        let mut shown = self.spread(player, told);
        if !masked {
            self.warn_analysts(player, here);
        }
        let gain = match (masked, watched) {
            (true, _) => noise / 2,
            (false, true) => noise * 2,
            (false, false) => noise,
        };
        shown.extend(self.raise_trace(player, gain * (4 + steps) / 4));
        shown
    }

    pub(super) fn watch(&mut self, player: PlayerId) -> Vec<Message> {
        let here = self.here(player);
        let alert = self.alert_ticks(player, ENTRY_ALERT);
        let Some(id) = self.instance_id(player).filter(|_| !self.masked(player)) else {
            return Vec::new();
        };
        let (target, name, present) = self.audience(id, here, player);
        let Some(instance) = self.instances.get_mut(&id) else {
            return Vec::new();
        };
        let mut told = Vec::new();
        let seeing = instance.ice.iter_mut().filter(|ice| {
            ice.host == here && ice.kind == IceKind::Sentry && !ice.broken() && ice.intent.is_none()
        });
        for ice in seeing {
            alert_on(ice, target, &name, &present, alert, &mut told);
        }
        self.spread(player, told)
    }

    fn masked(&self, player: PlayerId) -> bool {
        self.players
            .get(&player)
            .and_then(|state| state.run.as_ref())
            .is_some_and(|run| run.mask > 0)
    }

    fn alert_ticks(&self, player: PlayerId, ticks: u32) -> u32 {
        if self.heat_of(player) >= FAST_HEAT {
            ticks - 1
        } else {
            ticks
        }
    }

    fn audience(
        &self,
        id: InstanceId,
        here: HostId,
        player: PlayerId,
    ) -> (PlayerId, String, Vec<PlayerId>) {
        let members = self.members(id);
        let target = decoy_on(&members, here).unwrap_or(player);
        let name = name_of(&members, target).to_owned();
        let mut present = witnesses(&members, here, target);
        present.push(target);
        (target, name, present)
    }

    fn spread(&mut self, player: PlayerId, told: Vec<(PlayerId, Message)>) -> Vec<Message> {
        let mut shown = Vec::new();
        for (other, message) in told {
            if other == player {
                shown.push(message);
            } else {
                self.outbox.push(Output {
                    player: other,
                    message,
                });
            }
        }
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
        if self.networks[run.network.0].zone != Zone::Target {
            return Vec::new();
        }
        run.trace = (run.trace + amount).min(MAX_TRACE);
        if run.trace < MAX_TRACE {
            return Vec::new();
        }
        let network = &self.networks[run.network.0];
        let security = self.hosts[here.0].security.unwrap_or(network.security);
        let (heat, corp) = (security.heat(), network.corp);
        let name = &network.name;
        let mut shown = vec![Message::game(format!(
            "{name} traces you. The link drops, and you lose the loot of this run."
        ))];
        if let Some(corp) = corp {
            self.raise_alert(corp, TRACE_ALERT);
            let name = &self.corps[corp].name;
            self.publish(format!("{name} traces an intruder and calls the police."));
        }
        match self.first_job(player) {
            Some(fixer) => {
                let (name, now) = (self.fixer_name(fixer), self.heat_of(player));
                shown.push(Message::game(format!(
                    "{name} keeps the police away from a first job. Your heat stays at {now}."
                )));
            }
            None => shown.push(self.add_heat(player, heat)),
        }
        shown.extend(self.stop_script(player));
        let scene = self.drop_link(player, true);
        shown.extend(self.burn_if_hot(player, scene));
        shown
    }

    fn first_job(&self, player: PlayerId) -> Option<usize> {
        let state = self.players.get(&player)?;
        let (job, run) = (state.contract?, state.run.as_ref()?);
        let contract = self.contracts.get(job.contract())?;
        let first = self.offer == Some(job.contract()) && contract.network == run.network;
        first.then_some(contract.fixer)
    }

    fn drop_link(&mut self, player: PlayerId, lose_loot: bool) -> Vec<Message> {
        let from = self.here(player);
        let back = self.players.get(&player).and_then(|state| {
            let start = state.run.as_ref()?.start;
            state.chain.get(start.checked_sub(1)?).copied()
        });
        self.depart(player, from, back.unwrap_or(self.start));
        let mut closed = None;
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
            closed = Some((run.instance, run.network));
        }
        if let Some((id, network)) = closed {
            self.close_instance(id);
            self.restart_steps(player, network);
        }
        let here = self.here(player);
        self.appear(player, here);
        self.scene(here, player)
    }

    pub(super) fn cross(&mut self, player: PlayerId, from: HostId, to: HostId) -> Vec<Message> {
        match (self.hosts[from.0].network, self.hosts[to.0].network) {
            (None, Some(network)) => self.start_run(player, network),
            (Some(network), None) => self.end_run(player, network),
            _ => Vec::new(),
        }
    }

    fn start_run(&mut self, player: PlayerId, network: NetworkId) -> Vec<Message> {
        let darknet = self.zone(network) == Zone::Darknet;
        let joined = if darknet {
            self.darknet_copy(network)
        } else {
            self.crew_copy(player, network)
        };
        let id = joined.unwrap_or_else(|| {
            let instance = Instance {
                network,
                ice: deploy(&self.hosts, network, self.ice_bonus(network)),
                analysts: self.staff(network),
                logs: Vec::new(),
                bridges: Vec::new(),
                files: Vec::new(),
                deleted: Vec::new(),
                clock: 0,
                cuts: Vec::new(),
                cutting: Vec::new(),
                resets: Vec::new(),
            };
            self.open_instance(instance)
        });
        let Some(state) = self.players.get_mut(&player) else {
            return Vec::new();
        };
        state.run = Some(Run::new(id, network, state.chain.len() - 1));
        let name = &self.networks[network.0].name;
        let alert = self.network_alert(network);
        let start = if darknet {
            format!("You enter the darknet {name}. Other handles can attack you here.")
        } else if joined.is_some() {
            format!("You join your crew in the network of {name}.")
        } else {
            format!("You connect to the network of {name}.")
        };
        let connect = match alert.filter(|&alert| alert > Alert::Green) {
            Some(alert) => format!("{start} It is on {} alert.", alert.word()),
            None => start,
        };
        if self.is_job(player, network) {
            self.advance(player, Event::Enter);
        }
        self.note_entry(player, network);
        vec![Message::game(connect)]
    }

    fn end_run(&mut self, player: PlayerId, network: NetworkId) -> Vec<Message> {
        let closed = self
            .players
            .get_mut(&player)
            .and_then(|state| state.run.take());
        let done = closed.as_ref().is_some_and(|run| run.done);
        if let Some(run) = closed {
            self.close_instance(run.instance);
        }
        let name = &self.networks[network.0].name;
        let exit = if self.zone(network) == Zone::Darknet {
            Message::game(format!("You leave the darknet {name}."))
        } else {
            Message::game(format!("You leave the network of {name}. Clean exit."))
        };
        if self.is_job(player, network) {
            self.advance(player, Event::Exit);
        }
        let mut shown = vec![exit];
        shown.extend(self.finish_contract(player, network, done));
        shown.extend(self.finish_heist(player, network));
        self.restart_steps(player, network);
        shown
    }
}

fn alert_on(
    ice: &mut Ice,
    target: PlayerId,
    name: &str,
    present: &[PlayerId],
    ticks: u32,
    told: &mut Vec<(PlayerId, Message)>,
) {
    ice.target = Some(target);
    let mine = ice.compile_in(Action::Alert, ticks);
    let theirs = ice.telegraph(Some(name));
    for &other in present {
        let message = if other == target { &mine } else { &theirs };
        told.push((other, message.clone()));
    }
}

#[cfg(test)]
mod tests {
    use crate::testing::{hired, joined, plain, run, walk};
    use crate::{PlayerId, World};

    fn in_the_first_job() -> (World, PlayerId) {
        let (mut world, player) = hired();
        for line in ["go back-room", "go gate"] {
            world.command(player, line);
        }
        (world, player)
    }

    #[test]
    fn a_trace_in_the_first_job_gives_no_heat() {
        let (mut world, player) = in_the_first_job();
        let shown: Vec<String> = world
            .raise_trace(player, 100)
            .iter()
            .map(ttymud_text::Message::plain)
            .collect();
        assert_eq!(
            shown[..2],
            [
                "Test Lab traces you. The link drops, and you lose the loot of this run.",
                "Pell keeps the police away from a first job. Your heat stays at 0.",
            ]
        );
        assert_eq!(world.heat_of(player), 0);
    }

    #[test]
    fn a_late_jack_out_in_the_first_job_gives_no_heat() {
        let (mut world, player) = in_the_first_job();
        if let Some(run) = world
            .players
            .get_mut(&player)
            .and_then(|state| state.run.as_mut())
        {
            run.trace = 80;
        }
        let shown = plain(&world.command(player, "jackout"));
        assert!(!shown.iter().any(|line| line.contains("heat")), "{shown:?}");
        assert_eq!(world.heat_of(player), 0);
    }

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
