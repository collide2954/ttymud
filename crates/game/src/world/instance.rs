use super::ice::Ice;
use super::soc::Analyst;
use super::{HostId, NetworkId, Output, PlayerId, World};
use crate::content::File;
use ttymud_text::{Mate, Message};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct InstanceId(u64);

#[derive(Debug)]
pub(super) struct Instance {
    pub(super) network: NetworkId,
    pub(super) ice: Vec<Ice>,
    pub(super) analysts: Vec<Analyst>,
    pub(super) logs: Vec<(HostId, PlayerId, u32)>,
    pub(super) bridges: Vec<(HostId, HostId)>,
    pub(super) files: Vec<(HostId, File)>,
    pub(super) deleted: Vec<(HostId, String)>,
    pub(super) clock: u32,
    pub(super) cuts: Vec<(HostId, HostId)>,
    pub(super) cutting: Vec<Cut>,
}

impl Instance {
    pub(super) fn log(&mut self, host: HostId, player: PlayerId, amount: u32) {
        let mine = self
            .logs
            .iter_mut()
            .find(|(at, owner, _)| *at == host && *owner == player);
        match mine {
            Some((_, _, logged)) => *logged += amount,
            None => self.logs.push((host, player, amount)),
        }
    }
}

#[derive(Debug, Clone)]
pub(super) struct Cut {
    pub(super) analyst: String,
    pub(super) route: (HostId, HostId),
    pub(super) ticks: u32,
}

#[derive(Debug, Clone)]
pub(super) struct Member {
    pub(super) player: PlayerId,
    pub(super) host: HostId,
    pub(super) name: String,
    pub(super) trace: u32,
    pub(super) decoy: bool,
}

pub(super) fn decoy_on(members: &[Member], host: HostId) -> Option<PlayerId> {
    let found = members
        .iter()
        .find(|member| member.host == host && member.decoy);
    found.map(|member| member.player)
}

pub(super) fn host_of(members: &[Member], player: PlayerId) -> Option<HostId> {
    let member = members.iter().find(|member| member.player == player);
    member.map(|member| member.host)
}

pub(super) fn name_of(members: &[Member], player: PlayerId) -> &str {
    let member = members.iter().find(|member| member.player == player);
    member.map_or("", |member| member.name.as_str())
}

pub(super) fn witnesses(members: &[Member], host: HostId, except: PlayerId) -> Vec<PlayerId> {
    members
        .iter()
        .filter(|member| member.host == host && member.player != except)
        .map(|member| member.player)
        .collect()
}

pub(super) fn tell(players: &[PlayerId], message: &Message) -> Vec<Output> {
    players
        .iter()
        .map(|&player| Output {
            player,
            message: message.clone(),
        })
        .collect()
}

impl World {
    pub(super) fn members(&self, id: InstanceId) -> Vec<Member> {
        self.players
            .iter()
            .filter_map(|(&player, state)| {
                let run = state.run.as_ref().filter(|run| run.instance == id)?;
                Some(Member {
                    player,
                    host: *state.chain.last()?,
                    name: state.name.clone(),
                    trace: run.trace,
                    decoy: run.decoy > 0,
                })
            })
            .collect()
    }

    pub(super) fn mates(&self, player: PlayerId) -> Vec<Mate> {
        let Some(id) = self.instance_id(player) else {
            return Vec::new();
        };
        let crew = self.handle(player).and_then(|name| self.crews.of(name));
        self.members(id)
            .into_iter()
            .filter(|member| member.player != player)
            .filter(|member| crew.is_some_and(|crew| crew.has(&member.name)))
            .map(|member| Mate {
                name: member.name,
                trace: member.trace,
            })
            .collect()
    }

    pub(super) fn mates_here(&self, player: PlayerId) -> Vec<&str> {
        let here = self.here(player);
        let Some(copy) = self.instance_id(player) else {
            return Vec::new();
        };
        let crew = self.handle(player).and_then(|name| self.crews.of(name));
        self.players
            .iter()
            .filter(|&(&other, state)| {
                other != player
                    && state.chain.last() == Some(&here)
                    && state.run.as_ref().is_some_and(|run| run.instance == copy)
                    && crew.is_some_and(|crew| crew.has(&state.name))
            })
            .map(|(_, state)| state.name.as_str())
            .collect()
    }

    pub(super) fn crew_copy(&self, player: PlayerId, network: NetworkId) -> Option<InstanceId> {
        let crew = self.crews.of(self.handle(player)?)?;
        self.players
            .iter()
            .filter(|&(&other, state)| other != player && crew.has(&state.name))
            .filter_map(|(_, state)| state.run.as_ref())
            .find(|run| run.network == network)
            .map(|run| run.instance)
    }

    pub(super) fn open_instance(&mut self, instance: Instance) -> InstanceId {
        let id = InstanceId(self.next_instance);
        self.next_instance += 1;
        self.instances.insert(id, instance);
        id
    }

    pub(super) fn close_instance(&mut self, id: InstanceId) {
        let used = self
            .players
            .values()
            .filter_map(|state| state.run.as_ref())
            .any(|run| run.instance == id);
        if !used {
            self.instances.remove(&id);
        }
    }

    pub(super) fn instance_id(&self, player: PlayerId) -> Option<InstanceId> {
        Some(self.players.get(&player)?.run.as_ref()?.instance)
    }

    pub(super) fn instance(&self, player: PlayerId) -> Option<&Instance> {
        self.instances.get(&self.instance_id(player)?)
    }

    pub(super) fn instance_mut(&mut self, player: PlayerId) -> Option<&mut Instance> {
        let id = self.instance_id(player)?;
        self.instances.get_mut(&id)
    }
}

#[cfg(test)]
mod tests {
    use crate::ice::IceKind;
    use crate::testing::{heard, plain, world};
    use crate::{PlayerId, World};

    const NOON: u64 = 20_000 * 86_400 + 12 * 3_600;

    fn crew_of_two(world: &mut World) -> (PlayerId, PlayerId) {
        let (nyx, _) = world.join("nyx");
        let (zer0, _) = world.join("zer0");
        world.command(nyx, "crew create Moths");
        world.command(nyx, "crew invite zer0");
        world.command(zer0, "crew join moths");
        for player in [nyx, zer0] {
            if let Some(state) = world.players.get_mut(&player) {
                state.rig.cycles = 20;
            }
        }
        (nyx, zer0)
    }

    fn lab_crew() -> (World, PlayerId, PlayerId) {
        let mut world = world();
        let (nyx, zer0) = crew_of_two(&mut world);
        for player in [nyx, zer0] {
            world.command(player, "go back-room");
        }
        world.command(nyx, "go gate");
        (world, nyx, zer0)
    }

    fn trace(world: &World, player: PlayerId) -> u32 {
        let run = world
            .players
            .get(&player)
            .and_then(|state| state.run.as_ref());
        run.map_or(0, |run| run.trace)
    }

    fn integrity(world: &World, player: PlayerId) -> u32 {
        world
            .players
            .get(&player)
            .map_or(0, |state| state.rig.integrity)
    }

    #[test]
    fn crewmates_in_one_network_share_a_copy_and_see_and_hear_each_other() {
        let (mut world, nyx, zer0) = lab_crew();
        let outputs = world.command(zer0, "go gate");
        assert_eq!(heard(&outputs, nyx), ["zer0 arrives."]);
        let shown = heard(&outputs, zer0);
        assert_eq!(shown[0], "You join your crew in the network of Test Lab.");
        assert!(shown.contains(&"Here: nyx".to_owned()), "{shown:?}");
        assert_eq!(world.instance_id(nyx), world.instance_id(zer0));
        assert_eq!(world.instances.len(), 1);
        let outputs = world.command(zer0, "say on the gate");
        assert_eq!(heard(&outputs, nyx), ["<zer0> on the gate"]);
        let outputs = world.command(zer0, "go core");
        assert_eq!(heard(&outputs, nyx), ["zer0 leaves for core."]);
    }

    #[test]
    fn a_handle_outside_the_crew_gets_its_own_copy() {
        let (mut world, nyx, _) = lab_crew();
        let (ghost, _) = world.join("ghost");
        world.command(ghost, "go back-room");
        let outputs = world.command(ghost, "go gate");
        assert_eq!(heard(&outputs, nyx), Vec::<String>::new());
        let shown = heard(&outputs, ghost);
        assert!(
            !shown.iter().any(|line| line.starts_with("Here:")),
            "{shown:?}"
        );
        assert_ne!(world.instance_id(ghost), world.instance_id(nyx));
    }

    #[test]
    fn a_crack_breaks_the_ice_for_the_crew_and_the_copy_stays_for_the_last_crewmate() {
        let (mut world, nyx, zer0) = lab_crew();
        world.command(zer0, "go gate");
        world.command(nyx, "scan");
        world.command(nyx, "probe sentry | crack");
        let broken = "sentry: Sentry v1, broken";
        assert_eq!(plain(&world.command(zer0, "scan"))[0], broken);
        world.command(nyx, "jackout");
        assert_eq!(world.instances.len(), 1);
        assert_eq!(plain(&world.command(zer0, "scan"))[0], broken);
        world.command(zer0, "back");
        assert!(world.instances.is_empty());
    }

    #[test]
    fn ice_attacks_the_crewmate_who_woke_it_and_the_others_see_the_telegraph() {
        let (mut world, nyx, zer0) = lab_crew();
        world.command(zer0, "go gate");
        for player in [nyx, zer0] {
            for line in ["go core", "go archive"] {
                world.command(player, line);
            }
        }
        let outputs = world.command(nyx, "scan");
        assert!(
            heard(&outputs, zer0).contains(&"scorch wakes up.".to_owned()),
            "{outputs:?}"
        );
        let told = heard(&world.tick(), zer0);
        assert!(
            told.iter()
                .any(|line| line.starts_with("scorch starts to compile ")
                    && line.contains(" against nyx (")),
            "{told:?}"
        );
        for _ in 0..3 {
            world.tick();
        }
        assert!(integrity(&world, nyx) < 100);
        assert_eq!(integrity(&world, zer0), 100);
    }

    #[test]
    fn a_scorch_turns_to_a_crewmate_when_its_target_leaves_the_host() {
        let (mut world, nyx, zer0) = lab_crew();
        world.command(zer0, "go gate");
        for player in [nyx, zer0] {
            for line in ["go core", "go archive"] {
                world.command(player, line);
            }
        }
        world.command(nyx, "scan");
        world.tick();
        world.command(nyx, "go core");
        assert_eq!(heard(&world.tick(), nyx), ["scorch loses your signal."]);
        let told = heard(&world.tick(), zer0);
        assert!(
            told.iter()
                .any(|line| line.starts_with("scorch starts to compile ")
                    && !line.contains(" against ")),
            "{told:?}"
        );
    }

    #[test]
    fn scrub_with_a_handle_cleans_up_after_a_crewmate_on_your_host() {
        let (mut world, nyx, zer0) = lab_crew();
        world.command(zer0, "go gate");
        world.command(nyx, "scan");
        let outputs = world.command(zer0, "scrub nyx");
        assert_eq!(
            heard(&outputs, zer0),
            ["scrub deletes the log lines of nyx. Trace of nyx: 4% -> 0%."]
        );
        assert_eq!(
            heard(&outputs, nyx),
            ["zer0 scrubs your log lines. Trace: 4% -> 0%."]
        );
        let logs = world
            .instance(nyx)
            .map_or(0, |instance| instance.logs.len());
        assert_eq!(logs, 0);
        assert_eq!(
            plain(&world.command(zer0, "scrub nyx")),
            ["nyx has no trace to scrub."]
        );
        assert_eq!(
            plain(&world.command(zer0, "scrub ghost")),
            ["No crewmate named ghost. Crew: nyx"]
        );
        assert_eq!(world.complete(zer0, "scrub ").names, ["nyx"]);
    }

    #[test]
    fn a_decoy_draws_the_ice_on_its_host_away_from_the_crew() {
        let (mut world, nyx, zer0) = lab_crew();
        world.command(zer0, "go gate");
        for player in [nyx, zer0] {
            for line in ["go core", "go archive"] {
                world.command(player, line);
            }
        }
        if let Some(state) = world.players.get_mut(&zer0) {
            state.rig.loads.push(crate::commands::Verb::Decoy);
        }
        world.command(nyx, "scan");
        world.tick();
        let outputs = world.command(zer0, "decoy");
        assert_eq!(
            heard(&outputs, zer0)[0],
            "decoy draws the ICE on this host to you for 10 ticks."
        );
        assert_eq!(
            heard(&outputs, nyx),
            ["zer0 runs a decoy. The ICE on this host turns to zer0."]
        );
        let mut faded = false;
        for _ in 0..10 {
            let outputs = world.tick();
            faded |= heard(&outputs, zer0).contains(&"Your decoy fades.".to_owned());
        }
        assert!(faded);
        assert_eq!(integrity(&world, nyx), 100);
        assert!(integrity(&world, zer0) < 100);
    }

    #[test]
    fn a_sentry_alert_sends_the_hound_after_the_noisy_crewmate() {
        let (mut world, nyx, zer0) = lab_crew();
        world.command(zer0, "go gate");
        world.command(nyx, "scan");
        for _ in 0..3 {
            world.tick();
        }
        let hound = world
            .instance(nyx)
            .and_then(|instance| instance.ice.iter().find(|ice| ice.kind == IceKind::Hound))
            .and_then(|ice| ice.target);
        assert_eq!(hound, Some(nyx));
        assert!(trace(&world, nyx) > 0);
        assert_eq!(trace(&world, zer0), 0);
    }

    #[test]
    fn an_analyst_finds_the_trail_of_the_crewmate_who_made_the_noise() {
        let mut world = world();
        world.set_time(NOON);
        let (nyx, zer0) = crew_of_two(&mut world);
        for player in [nyx, zer0] {
            for line in ["go bazaar", "accept 1", "go relay", "go back-room"] {
                world.command(player, line);
            }
            crate::testing::enter_job(&mut world, player);
        }
        let here = world.hosts[world.here(nyx).0].id.clone();
        let Some(instance) = world.instance_mut(nyx) else {
            return;
        };
        instance.ice.clear();
        for (index, analyst) in instance.analysts.iter_mut().enumerate() {
            analyst.habit = super::super::soc::Habit::Strict;
            analyst.wait = if index == 0 { 1 } else { 100 };
        }
        let name = instance.analysts[0].name.clone();
        world.command(nyx, "scan");
        let turns = format!("[SOC] Analyst {name} turns to the logs of {here}.");
        let outputs = world.tick();
        assert_eq!(heard(&outputs, nyx), std::slice::from_ref(&turns));
        assert_eq!(heard(&outputs, zer0), [turns]);
        world.tick();
        let outputs = world.tick();
        assert_eq!(
            heard(&outputs, nyx)[0],
            format!(
                "[SOC] Analyst {name} reads the logs of {here} and finds your trail. Trace: 2% -> 9%."
            )
        );
        assert_eq!(
            heard(&outputs, zer0)[0],
            format!("[SOC] Analyst {name} reads the logs of {here} and finds the trail of nyx.")
        );
        assert_eq!(trace(&world, zer0), 0);
    }

    #[test]
    fn a_run_opens_its_own_copy_and_the_end_of_the_run_closes_it() {
        let mut world = world();
        let (ghost, _) = world.join("ghost");
        let (nyx, _) = world.join("nyx");
        for player in [ghost, nyx] {
            for line in ["go back-room", "go gate"] {
                world.command(player, line);
            }
        }
        assert_eq!(world.instances.len(), 2);
        assert_ne!(world.instance_id(ghost), world.instance_id(nyx));
        world.command(ghost, "back");
        assert_eq!(world.instances.len(), 1);
        world.command(nyx, "jackout");
        assert!(world.instances.is_empty());
    }
}
