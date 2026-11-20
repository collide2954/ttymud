use super::rig::Rig;
use super::{PlayerId, World};
use crate::commands::{self, Program, Verb};

pub(super) const TOP_VERSION: u32 = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Build {
    pub(super) version: u32,
    pub(super) quiet: bool,
}

impl Default for Build {
    fn default() -> Self {
        Self {
            version: 1,
            quiet: false,
        }
    }
}

pub(super) fn step(verb: Verb) -> u32 {
    match verb {
        Verb::Crack | Verb::Scrub | Verb::Patch => 5,
        Verb::Decoy => 2,
        Verb::Firewall | Verb::Mask => 1,
        _ => 0,
    }
}

impl Rig {
    pub(super) fn build(&self, verb: Verb) -> Build {
        let found = self.builds.iter().find(|&&(known, _)| known == verb);
        found.map_or_default(|&(_, build)| build)
    }

    pub(super) fn boosted(&self, verb: Verb, base: u32) -> u32 {
        base + step(verb) * (self.build(verb).version - 1)
    }

    pub(super) fn program(&self, verb: Verb) -> Option<Program> {
        let mut program = verb.program()?;
        if self.build(verb).quiet {
            program.noise = program.noise.saturating_sub(1);
            program.cycles += 1;
        }
        Some(program)
    }

    pub(super) fn label(&self, verb: Verb) -> String {
        let build = self.build(verb);
        let mut label = commands::name_of(verb).to_owned();
        if build.version > 1 {
            label = format!("{label} v{}", build.version);
        }
        if build.quiet {
            label.push_str(" (quiet)");
        }
        label
    }
}

impl World {
    pub(super) fn boost(&self, player: PlayerId, verb: Verb, base: u32) -> u32 {
        let state = self.players.get(&player);
        state.map_or(base, |state| state.rig.boosted(verb, base))
    }
}

#[cfg(test)]
mod tests {
    use super::Build;
    use crate::commands::Verb;
    use crate::testing::{joined, plain};
    use crate::{PlayerId, World};

    fn built(builds: &[(Verb, Build)]) -> (World, PlayerId) {
        let (mut world, player) = joined();
        if let Some(state) = world.players.get_mut(&player) {
            state.rig.builds = builds.to_vec();
            state
                .rig
                .loads
                .extend([Verb::Firewall, Verb::Mask, Verb::Decoy]);
        }
        for line in ["go back-room", "go gate"] {
            world.command(player, line);
        }
        if let Some(state) = world.players.get_mut(&player) {
            state.rig.cycles = 20;
        }
        (world, player)
    }

    fn version(version: u32) -> Build {
        Build {
            version,
            quiet: false,
        }
    }

    fn said(world: &mut World, player: PlayerId, line: &str) -> Vec<String> {
        plain(&world.command(player, line))
    }

    fn vitals(world: &World, player: PlayerId) -> (u32, u32) {
        let state = world.players.get(&player);
        let trace = state
            .and_then(|state| state.run.as_ref())
            .map_or(0, |run| run.trace);
        (state.map_or(0, |state| state.rig.cycles), trace)
    }

    #[test]
    fn a_higher_version_makes_the_main_effect_of_a_program_stronger() {
        let (mut world, player) = built(&[
            (Verb::Crack, version(3)),
            (Verb::Scrub, version(2)),
            (Verb::Patch, version(3)),
            (Verb::Firewall, version(2)),
            (Verb::Mask, version(3)),
            (Verb::Decoy, version(2)),
        ]);
        said(&mut world, player, "scan");
        assert_eq!(
            said(&mut world, player, "crack warden"),
            ["crack hits warden. Integrity: 40 -> 15."]
        );
        if let Some(state) = world.players.get_mut(&player) {
            state.rig.integrity = 50;
            if let Some(run) = state.run.as_mut() {
                run.trace = 30;
            }
        }
        assert_eq!(
            said(&mut world, player, "scrub"),
            ["scrub deletes your log lines. Trace: 30% -> 10%."]
        );
        assert_eq!(
            said(&mut world, player, "patch"),
            ["patch repairs your rig. Integrity: 50 -> 80."]
        );
        assert_eq!(
            said(&mut world, player, "firewall"),
            ["firewall is up. It stops the next 2 attacks on your integrity."]
        );
        assert_eq!(
            said(&mut world, player, "mask")[0],
            "mask hides your noise for 7 ticks."
        );
        assert_eq!(
            said(&mut world, player, "decoy")[0],
            "decoy draws the ICE on this host to you for 12 ticks."
        );
    }

    #[test]
    fn the_quiet_mod_trades_one_noise_for_one_cycle() {
        let mut costs = Vec::new();
        for quiet in [false, true] {
            let crack = Build { version: 1, quiet };
            let (mut world, player) = built(&[(Verb::Crack, crack)]);
            said(&mut world, player, "scan");
            let before = vitals(&world, player);
            said(&mut world, player, "crack warden");
            let after = vitals(&world, player);
            costs.push((before.0 - after.0, after.1 - before.1));
        }
        assert_eq!(costs, [(3, 6), (4, 4)]);
    }

    #[test]
    fn status_shows_the_versions_and_the_mods() {
        let crack = Build {
            version: 2,
            quiet: true,
        };
        let (mut world, player) = built(&[(Verb::Crack, crack)]);
        assert_eq!(
            said(&mut world, player, "status | grep programs"),
            ["Programs: scan, probe, crack v2 (quiet), scrub, patch, firewall, decoy, mask"]
        );
    }

    #[test]
    fn versions_and_mods_come_back_from_the_save() {
        let mut world = crate::testing::world();
        let (player, _) = world.connect("a");
        for line in ["nyx", "breaker"] {
            world.command(player, line);
        }
        if let Some(state) = world.players.get_mut(&player) {
            state.rig.builds = vec![
                (Verb::Crack, version(3)),
                (
                    Verb::Probe,
                    Build {
                        version: 1,
                        quiet: true,
                    },
                ),
            ];
        }
        world.leave(player);
        let saves = world.saves();
        let text = saves
            .iter()
            .find(|(path, _)| path == "accounts/a.ini")
            .map_or_default(|(_, text)| text.clone());
        assert!(
            text.contains("\nversion = crack 3\nquiet = probe\n"),
            "{text}"
        );
        let mut fresh = crate::testing::world();
        for (path, text) in &saves {
            fresh.restore(path, text).unwrap();
        }
        let (player, _) = fresh.connect("a");
        assert_eq!(
            said(&mut fresh, player, "status | grep programs"),
            ["Programs: scan, probe (quiet), crack v3, patch, firewall"]
        );
    }
}
