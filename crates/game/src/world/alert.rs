use std::collections::BTreeMap;

use super::{NetworkId, PlayerId, World};

const COOLING: u64 = 5 * 60;
const MAX_ALERT: u32 = 100;
pub(super) const TRACE_ALERT: u32 = 25;
pub(super) const BREACH_ALERT: u32 = 15;
pub(super) const MAX_VERSION: u32 = 9;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Alert {
    Green,
    Yellow,
    Orange,
    Red,
}

impl Alert {
    fn of(points: u32) -> Self {
        match points {
            0..25 => Self::Green,
            25..50 => Self::Yellow,
            50..75 => Self::Orange,
            _ => Self::Red,
        }
    }

    pub(super) fn steps(self) -> u32 {
        match self {
            Self::Green => 0,
            Self::Yellow => 1,
            Self::Orange => 2,
            Self::Red => 3,
        }
    }

    pub(super) fn word(self) -> &'static str {
        match self {
            Self::Green => "green",
            Self::Yellow => "yellow",
            Self::Orange => "orange",
            Self::Red => "red",
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct CorpState {
    pub(super) alert: u32,
    pub(super) since: u64,
    pub(super) used: BTreeMap<String, u32>,
    pub(super) patched: BTreeMap<String, u64>,
}

impl World {
    pub(super) fn alert_points(&self, corp: usize) -> u32 {
        let Some(state) = self.corp_states.get(corp) else {
            return 0;
        };
        let cooled = self.now.saturating_sub(state.since) / COOLING;
        state
            .alert
            .saturating_sub(u32::try_from(cooled).unwrap_or(u32::MAX))
    }

    pub(super) fn alert(&self, corp: usize) -> Alert {
        Alert::of(self.alert_points(corp))
    }

    pub(super) fn raise_alert(&mut self, corp: usize, points: u32) {
        let current = self.alert_points(corp);
        let now = self.now;
        if let Some(state) = self.corp_states.get_mut(corp) {
            state.alert = current.saturating_add(points).min(MAX_ALERT);
            state.since = now;
            self.state_dirty = true;
        }
    }

    pub(super) fn network_alert(&self, network: NetworkId) -> Option<Alert> {
        self.networks[network.0].corp.map(|corp| self.alert(corp))
    }

    pub(super) fn run_corp(&self, player: PlayerId) -> Option<usize> {
        let run = self.players.get(&player)?.run.as_ref()?;
        self.networks[run.network.0].corp
    }

    pub(super) fn ice_bonus(&self, network: NetworkId) -> u32 {
        self.network_alert(network).map_or(0, Alert::steps) + self.night_bonus(network)
    }
}

#[cfg(test)]
mod tests {
    use super::Alert;
    use crate::target::Level;
    use crate::testing::{plain, world};
    use crate::world::HostId;
    use crate::{PlayerId, World};

    const DAY: u64 = 20_000 * 86_400 + 12 * 3_600;

    fn acme() -> (World, PlayerId, usize) {
        let mut world = world();
        world.set_time(DAY);
        let (player, _) = world.join("ghost");
        for line in ["go bazaar", "accept 1", "go relay", "go back-room"] {
            world.command(player, line);
        }
        let corp = world.corps.iter().position(|corp| corp.id == "acme");
        (world, player, corp.unwrap_or_default())
    }

    fn enter(world: &mut World, player: PlayerId) -> Vec<String> {
        let entry = world.networks.last().map(|network| network.entries[0]);
        let id = entry.map_or_default(|entry| world.hosts[entry.0].id.clone());
        plain(&world.command(player, &format!("go {id}")))
    }

    fn trace(world: &World, player: PlayerId) -> u32 {
        let run = world
            .players
            .get(&player)
            .and_then(|state| state.run.as_ref());
        run.map_or(0, |run| run.trace)
    }

    #[test]
    fn an_alert_rises_with_points_and_cools_one_point_each_5_minutes() {
        let (mut world, _, corp) = acme();
        assert_eq!(world.alert(corp), Alert::Green);
        world.raise_alert(corp, 30);
        assert_eq!(
            (world.alert_points(corp), world.alert(corp)),
            (30, Alert::Yellow)
        );
        world.raise_alert(corp, 200);
        assert_eq!(world.alert_points(corp), 100);
        world.set_time(DAY + 26 * 300);
        assert_eq!(
            (world.alert_points(corp), world.alert(corp)),
            (74, Alert::Orange)
        );
        world.set_time(DAY + 76 * 300);
        assert_eq!(world.alert(corp), Alert::Green);
    }

    #[test]
    fn noise_and_traces_raise_the_alert_of_the_corp() {
        let (mut world, player, corp) = acme();
        enter(&mut world, player);
        world.command(player, "scan");
        assert_eq!(world.alert_points(corp), 2);
        world.raise_trace(player, 100);
        assert_eq!(world.alert_points(corp), 27);
    }

    #[test]
    fn a_breach_raises_the_alert() {
        let (mut world, player, corp) = acme();
        enter(&mut world, player);
        let goal = world
            .contracts
            .last()
            .map_or_default(|job| job.goal.clone());
        let holder = (0..world.hosts.len())
            .find(|&index| {
                world.hosts[index]
                    .files
                    .iter()
                    .any(|file| file.name == goal)
            })
            .map(HostId);
        if let (Some(state), Some(holder)) = (world.players.get_mut(&player), holder) {
            state.chain.push(holder);
            if let Some(run) = state.run.as_mut() {
                run.access = Level::Admin;
            }
        }
        world.command(player, &format!("download {goal}"));
        let before = world.alert_points(corp);
        world.command(player, "back");
        world.command(player, "back");
        assert_eq!(world.alert_points(corp), before + 15);
    }

    #[test]
    fn a_higher_alert_makes_more_trace_and_stronger_ice_in_new_runs() {
        let (mut world, player, corp) = acme();
        world.raise_alert(corp, 60);
        let shown = enter(&mut world, player);
        assert_eq!(
            shown[0],
            "You connect to the network of Acme Corp. It is on orange alert."
        );
        let network = world.networks.len() - 1;
        let base: Vec<u32> = world
            .hosts
            .iter()
            .filter(|host| host.network.is_some_and(|known| known.0 == network))
            .flat_map(|host| host.ice.iter().map(|ice| (ice.version + 2).min(9)))
            .collect();
        let run = world
            .players
            .get(&player)
            .and_then(|state| state.run.as_ref());
        let versions: Vec<u32> =
            run.map_or_default(|run| run.ice.iter().map(|ice| ice.version).collect());
        assert_eq!(versions, base);
        world.command(player, "scan");
        assert_eq!(trace(&world, player), 6);
    }

    #[test]
    fn the_board_and_status_show_the_alert() {
        let (mut world, player, corp) = acme();
        world.raise_alert(corp, 80);
        for line in ["go relay", "go bazaar"] {
            world.command(player, line);
        }
        let board = plain(&world.command(player, "jobs"));
        assert!(
            board[1].starts_with("1. Acme Corp (red alert): get "),
            "{board:?}"
        );
        for line in ["go relay", "go back-room"] {
            world.command(player, line);
        }
        enter(&mut world, player);
        let status = plain(&world.command(player, "status"));
        assert_eq!(status[1], "Alert: red");
    }

    #[test]
    fn the_alert_comes_back_from_world_ini() {
        let (mut world, _, corp) = acme();
        world.raise_alert(corp, 60);
        let saves = world.saves();
        let text = saves
            .iter()
            .find(|(path, _)| path == "world.ini")
            .map_or_default(|(_, text)| text.clone());
        assert!(
            text.ends_with(&format!("\n\n[corp acme]\nalert = 60\nsince = {DAY}\n")),
            "{text}"
        );
        let mut fresh = crate::testing::world();
        for (path, text) in &saves {
            fresh.restore(path, text).unwrap();
        }
        fresh.set_time(DAY + 300);
        assert_eq!(fresh.alert_points(corp), 59);
        assert!(
            fresh
                .restore(
                    "world.ini",
                    "[world test]\nseed = 7\n[corp gone]\nalert = 9\nsince = 1\n"
                )
                .is_ok()
        );
    }
}
