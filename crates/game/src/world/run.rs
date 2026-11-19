use ttymud_text::Message;

use super::{HostId, NetworkId, PlayerId, Step, World};

const SHOCK: u32 = 20;
const CRASH: &str = "Your rig crashes, and you lose the loot of this run.";

#[derive(Debug)]
pub(super) struct Run {
    pub(super) network: NetworkId,
    pub(super) start: usize,
    pub(super) trace: u32,
    pub(super) loot: Vec<String>,
}

impl World {
    pub(super) fn may_enter(&self, player: PlayerId, target: HostId) -> Result<(), Message> {
        let Some(state) = self.players.get(&player) else {
            return Ok(());
        };
        let entering = state.run.is_none() && self.hosts[target.0].network.is_some();
        if entering && state.rig.integrity == 0 {
            return Err(Message::error(
                "Your rig has no integrity left. Repair it before you connect.",
            ));
        }
        Ok(())
    }

    pub(super) fn jackout(&mut self, player: PlayerId) -> Result<Step, Message> {
        let Some(state) = self.players.get_mut(&player) else {
            return Ok(Step::default());
        };
        let Some(run) = &state.run else {
            return Err(Message::error("You are not in a target network."));
        };
        let name = &self.networks[run.network.0].name;
        let mut shown = vec![Message::game(format!(
            "You jack out of {name}. The shock costs your rig {SHOCK} integrity."
        ))];
        state.rig.integrity = state.rig.integrity.saturating_sub(SHOCK);
        let crashed = state.rig.integrity == 0;
        if crashed {
            shown.push(Message::game(CRASH));
        }
        shown.extend(self.drop_link(player, crashed));
        Ok(Step {
            shown,
            items: Vec::new(),
        })
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

    pub(super) fn cross(&mut self, player: PlayerId, from: HostId, to: HostId) -> Option<Message> {
        let state = self.players.get_mut(&player)?;
        match (self.hosts[from.0].network, self.hosts[to.0].network) {
            (None, Some(network)) => {
                state.run = Some(Run {
                    network,
                    start: state.chain.len() - 1,
                    trace: 0,
                    loot: Vec::new(),
                });
                let name = &self.networks[network.0].name;
                Some(Message::game(format!(
                    "You connect to the network of {name}."
                )))
            }
            (Some(network), None) => {
                state.run = None;
                let name = &self.networks[network.0].name;
                Some(Message::game(format!(
                    "You leave the network of {name}. Clean exit."
                )))
            }
            _ => None,
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
        let lines = ["go back-room", "go gate", "jackout", "status"];
        assert_eq!(walk(&lines).0, ["Integrity: 80/100", "Cycles: 4/4"]);
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
            ["Your rig has no integrity left. Repair it before you connect."]
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
        assert_eq!(walk(&["status"]).0[0], "Integrity: 100/100");
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
