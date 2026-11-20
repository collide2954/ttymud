use ttymud_text::{Kind, Message, Style};

use super::instance::{tell, witnesses};
use super::presence::named;
use super::programs::Target;
use super::rig::MAX_INTEGRITY;
use super::{HostId, Output, PlayerId, Step, Thing, World, address, choose, listed, usage};
use crate::commands::{Item, Verb};
use crate::content::Zone;

const CREWMATE: &str = " is in your crew. Crewmates cannot attack each other.";

impl World {
    pub(super) fn pvp(&self, player: PlayerId) -> bool {
        self.run_zone(player) == Some(Zone::Darknet)
    }

    pub(super) fn strangers(&self, player: PlayerId) -> Vec<(PlayerId, HostId, &str)> {
        let Some(copy) = self.instance_id(player) else {
            return Vec::new();
        };
        let crew = self.handle(player).and_then(|name| self.crews.of(name));
        self.players
            .iter()
            .filter(|&(&other, state)| {
                other != player
                    && state.run.as_ref().is_some_and(|run| run.instance == copy)
                    && !crew.is_some_and(|crew| crew.has(&state.name))
            })
            .filter_map(|(&other, state)| Some((other, *state.chain.last()?, state.name.as_str())))
            .collect()
    }

    pub(super) fn rivals(&self, player: PlayerId) -> Vec<(PlayerId, &str)> {
        if !self.pvp(player) {
            return Vec::new();
        }
        let here = self.here(player);
        self.strangers(player)
            .into_iter()
            .filter(|&(_, host, _)| host == here)
            .map(|(other, _, name)| (other, name))
            .collect()
    }

    fn rival(&self, player: PlayerId, name: &str) -> Option<PlayerId> {
        let found = self
            .rivals(player)
            .into_iter()
            .find(|&(_, rival)| rival == name);
        found.map(|(rival, _)| rival)
    }

    pub(super) fn aim(
        &self,
        player: PlayerId,
        command: &str,
        words: &[String],
        ice: &[&str],
    ) -> Result<Vec<Target>, Message> {
        let mut names = ice.to_vec();
        for (_, name) in self.rivals(player) {
            if !names.contains(&name) {
                names.push(name);
            }
        }
        if words.is_empty() {
            return Err(usage(&format!("{command} <target>"), Thing::Target, &names));
        }
        words
            .iter()
            .map(|word| {
                self.spare_crew(player, word)?;
                let name = choose(Thing::Target, word, &names)?.to_owned();
                let weak = false;
                Ok(if ice.contains(&name.as_str()) {
                    Target::Ice { name, weak }
                } else {
                    Target::Rig { name, weak }
                })
            })
            .collect()
    }

    pub(super) fn spare_crew(&self, player: PlayerId, word: &str) -> Result<(), Message> {
        let Some(me) = self.handle(player) else {
            return Ok(());
        };
        let mate = self
            .crews
            .of(me)
            .into_iter()
            .flat_map(|crew| &crew.members)
            .find(|member| member.eq_ignore_ascii_case(word) && !member.eq_ignore_ascii_case(me));
        match mate {
            Some(mate) => Err(Message::new(Kind::Error)
                .span(Style::Player, mate)
                .text(CREWMATE)),
            None => Ok(()),
        }
    }

    pub(super) fn probe_rig(&mut self, player: PlayerId, name: String, step: &mut Step) {
        let Some(rival) = self.rival(player, &name) else {
            return;
        };
        let Some(state) = self.players.get(&rival) else {
            return;
        };
        let up = state.run.as_ref().is_some_and(|run| run.firewall > 0);
        let shield = if up { ", firewall up" } else { "" };
        let integrity = state.rig.integrity;
        let line = Message::new(Kind::Game)
            .span(Style::Player, &name)
            .text(format!(
                ": integrity {integrity}/{MAX_INTEGRITY}{shield}. Programs: "
            ));
        let loaded = state.rig.loaded();
        step.shown
            .push(listed(line, Style::Command, loaded.iter().map(String::as_str)).text("."));
        step.items.push(Item::Rig { name, weak: true });
        let me = self.handle(player).unwrap_or_default();
        let probes = named(me, " probes your rig.");
        self.outbox.push(Output {
            player: rival,
            message: probes,
        });
    }

    pub(super) fn crack_rig(&mut self, player: PlayerId, name: &str, weak: bool) -> Step {
        let Some(rival) = self.rival(player, name) else {
            return Step::default();
        };
        self.touch(rival);
        let me = self.handle(player).unwrap_or_default().to_owned();
        let shield = self
            .players
            .get_mut(&rival)
            .and_then(|state| state.run.as_mut())
            .filter(|run| run.firewall > 0);
        if let Some(run) = shield {
            run.firewall -= 1;
            let stops = Message::game("Your firewall stops a crack from ")
                .span(Style::Player, &me)
                .text(".");
            self.outbox.push(Output {
                player: rival,
                message: stops,
            });
            return Step::show(vec![
                Message::game("The firewall of ")
                    .span(Style::Player, name)
                    .text(" stops your crack."),
            ]);
        }
        let crack = self.power(player, Verb::Crack);
        let strength = if weak { crack * 2 } else { crack };
        let before = self
            .players
            .get(&rival)
            .map_or(0, |state| state.rig.integrity);
        let after = before.saturating_sub(strength);
        let hit = named(
            &me,
            &format!(" cracks your rig. Integrity: {before} -> {after}."),
        );
        self.outbox.push(Output {
            player: rival,
            message: hit,
        });
        let seen = named(&me, " cracks the rig of ")
            .span(Style::Player, name)
            .text(".");
        let onlookers = self.onlookers(player, rival);
        self.outbox.extend(tell(&onlookers, &seen));
        let point = if weak { " in its weak point" } else { "" };
        let shown = Message::game("crack hits ")
            .span(Style::Player, name)
            .text(format!("{point}. Integrity: {before} -> {after}."));
        let crashed = self.damage(rival, strength);
        self.outbox.extend(address(rival, crashed));
        Step::show(vec![shown])
    }

    fn onlookers(&self, player: PlayerId, rival: PlayerId) -> Vec<PlayerId> {
        let Some(copy) = self.instance_id(player) else {
            return Vec::new();
        };
        let mut seen = witnesses(&self.members(copy), self.here(player), player);
        seen.retain(|&other| other != rival);
        seen
    }
}

#[cfg(test)]
mod tests {
    use crate::commands::Verb;
    use crate::testing::{dark, heard, inside, plain};
    use crate::world::builds::Build;
    use crate::{PlayerId, World};

    fn two() -> (World, PlayerId, PlayerId) {
        let mut world = dark();
        let nyx = inside(&mut world, "nyx");
        let zer0 = inside(&mut world, "zer0");
        (world, nyx, zer0)
    }

    fn integrity(world: &World, player: PlayerId) -> u32 {
        world
            .players
            .get(&player)
            .map_or(0, |state| state.rig.integrity)
    }

    fn crew(world: &mut World) -> (PlayerId, PlayerId) {
        let (nyx, _) = world.join("nyx");
        let (zer0, _) = world.join("zer0");
        world.command(nyx, "crew create Moths");
        world.command(nyx, "crew invite zer0");
        world.command(zer0, "crew join moths");
        (nyx, zer0)
    }

    #[test]
    fn crack_hits_the_rig_of_a_handle_on_your_host() {
        let (mut world, nyx, zer0) = two();
        let ghost = inside(&mut world, "ghost");
        let outputs = world.command(zer0, "crack nyx");
        assert_eq!(
            heard(&outputs, zer0),
            ["crack hits nyx. Integrity: 100 -> 85."]
        );
        assert_eq!(
            heard(&outputs, nyx),
            ["zer0 cracks your rig. Integrity: 100 -> 85."]
        );
        assert_eq!(heard(&outputs, ghost), ["zer0 cracks the rig of nyx."]);
        assert_eq!(integrity(&world, nyx), 85);
    }

    #[test]
    fn probe_shows_the_rig_and_gives_crack_its_weak_point() {
        let (mut world, nyx, zer0) = two();
        let outputs = world.command(zer0, "probe nyx");
        assert_eq!(
            heard(&outputs, zer0),
            ["nyx: integrity 100/100. Programs: scan, probe, crack, scrub, patch."]
        );
        assert_eq!(heard(&outputs, nyx), ["zer0 probes your rig."]);
        for _ in 0..2 {
            world.tick();
        }
        let outputs = world.command(zer0, "probe nyx | crack");
        assert_eq!(
            heard(&outputs, zer0),
            ["crack hits nyx in its weak point. Integrity: 100 -> 70."]
        );
        assert_eq!(
            heard(&outputs, nyx),
            [
                "zer0 probes your rig.",
                "zer0 cracks your rig. Integrity: 100 -> 70."
            ]
        );
    }

    #[test]
    fn a_firewall_stops_a_crack_from_a_handle() {
        let mut world = dark();
        let (nyx, _) = world.join("nyx");
        let lines = [
            "go home",
            "unload patch",
            "load firewall",
            "back",
            "go alley",
            "go alley",
            "firewall",
        ];
        for line in lines {
            world.command(nyx, line);
        }
        let zer0 = inside(&mut world, "zer0");
        assert_eq!(
            heard(&world.command(zer0, "probe nyx"), zer0),
            ["nyx: integrity 100/100, firewall up. Programs: scan, probe, crack, scrub, firewall."]
        );
        for _ in 0..2 {
            world.tick();
        }
        let outputs = world.command(zer0, "crack nyx");
        assert_eq!(
            heard(&outputs, zer0),
            ["The firewall of nyx stops your crack."]
        );
        assert_eq!(
            heard(&outputs, nyx),
            ["Your firewall stops a crack from zer0."]
        );
        assert_eq!(integrity(&world, nyx), 100);
    }

    #[test]
    fn crack_v3_hits_for_25() {
        let (mut world, nyx, zer0) = two();
        if let Some(state) = world.players.get_mut(&zer0) {
            let build = Build {
                version: 3,
                quiet: false,
            };
            state.rig.builds.push((Verb::Crack, build));
        }
        assert_eq!(
            heard(&world.command(zer0, "crack nyx"), zer0),
            ["crack hits nyx. Integrity: 100 -> 75."]
        );
        assert_eq!(integrity(&world, nyx), 75);
    }

    #[test]
    fn crewmates_cannot_attack_each_other() {
        let mut world = dark();
        let (nyx, zer0) = crew(&mut world);
        for player in [nyx, zer0] {
            for _ in 0..2 {
                world.command(player, "go alley");
            }
        }
        let refused = ["nyx is in your crew. Crewmates cannot attack each other."];
        assert_eq!(plain(&world.command(zer0, "crack nyx")), refused);
        assert_eq!(plain(&world.command(zer0, "probe NYX")), refused);
        assert_eq!(world.complete(zer0, "crack ").names, Vec::<String>::new());
        assert_eq!(integrity(&world, nyx), 100);
    }

    #[test]
    fn only_rivals_on_your_host_are_targets() {
        let (mut world, nyx, zer0) = two();
        world.command(nyx, "go den");
        assert_eq!(
            plain(&world.command(zer0, "crack nyx")),
            ["No target named nyx. Targets: none"]
        );
        world.command(zer0, "go den");
        assert_eq!(
            plain(&world.command(zer0, "probe")),
            ["Usage: probe <target>. Targets: nyx"]
        );
        assert_eq!(world.complete(zer0, "crack ").names, ["nyx"]);
    }

    #[test]
    fn handles_outside_a_darknet_are_no_targets() {
        let mut world = crate::testing::world();
        let (nyx, zer0) = crew(&mut world);
        for player in [nyx, zer0] {
            for line in ["go back-room", "go gate"] {
                world.command(player, line);
            }
        }
        assert_eq!(
            plain(&world.command(zer0, "crack nyx")),
            ["No ICE named nyx. Type scan to find ICE."]
        );
    }
}
