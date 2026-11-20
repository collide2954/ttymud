use ttymud_text::{Message, Style};

use super::instance::InstanceId;
use super::{HostId, NetworkId, PlayerId, World};
use crate::content::Zone;

const CONSENT: u64 = 60;

impl World {
    pub(super) fn zone(&self, network: NetworkId) -> Zone {
        self.networks[network.0].zone
    }

    pub(super) fn consent(&mut self, player: PlayerId, target: HostId) -> Result<(), Message> {
        let Some(network) = self.hosts[target.0].network else {
            return Ok(());
        };
        if self.zone(network) != Zone::Darknet {
            return Ok(());
        }
        let now = self.now;
        let Some(state) = self.players.get_mut(&player) else {
            return Ok(());
        };
        if state.run.is_some() {
            return Ok(());
        }
        let agreed = state
            .consent
            .is_some_and(|(host, when)| host == target && now.saturating_sub(when) <= CONSENT);
        if agreed {
            state.consent = None;
            return Ok(());
        }
        state.consent = Some((target, now));
        let name = &self.networks[network.0].name;
        let id = &self.hosts[target.0].id;
        Err(Message::error(format!(
            "{name} is a darknet. Other handles there can crash your rig, take your files, and tip off the police. Type "
        ))
        .span(Style::Command, format!("go {id}"))
        .text(" again to agree."))
    }

    pub(super) fn darknet_copy(&self, network: NetworkId) -> Option<InstanceId> {
        let found = self
            .instances
            .iter()
            .find(|(_, instance)| instance.network == network);
        found.map(|(&id, _)| id)
    }
}

#[cfg(test)]
mod tests {
    use crate::testing::{DARK, heard, plain};
    use crate::{Content, PlayerId, World};

    const ASK: &str = "The Undercity is a darknet. Other handles there can crash your rig, take your files, and tip off the police. Type go alley again to agree.";

    fn city() -> World {
        World::new(Content::parse(DARK).unwrap(), 0)
    }

    fn inside(world: &mut World, name: &str) -> PlayerId {
        let (player, _) = world.join(name);
        for _ in 0..2 {
            world.command(player, "go alley");
        }
        player
    }

    #[test]
    fn a_darknet_asks_for_a_second_go_on_each_entry() {
        let mut world = city();
        let (nyx, _) = world.join("nyx");
        assert_eq!(plain(&world.command(nyx, "go alley")), [ASK]);
        assert_eq!(world.prompt(nyx).as_deref(), Some("nyx@hub $ "));
        let shown = plain(&world.command(nyx, "go alley"));
        assert_eq!(
            shown[0],
            "You enter the darknet The Undercity. Other handles can attack you here."
        );
        assert_eq!(shown[1], "Alley");
        assert_eq!(
            plain(&world.command(nyx, "go den"))[0],
            "Den",
            "a move inside the darknet does not ask"
        );
        world.command(nyx, "back");
        assert_eq!(
            plain(&world.command(nyx, "back"))[0],
            "You leave the darknet The Undercity."
        );
        assert_eq!(plain(&world.command(nyx, "go alley")), [ASK]);
    }

    #[test]
    fn the_consent_lasts_one_minute() {
        let mut world = city();
        let (nyx, _) = world.join("nyx");
        world.command(nyx, "go alley");
        world.set_time(61);
        assert_eq!(plain(&world.command(nyx, "go alley")), [ASK]);
        world.set_time(120);
        assert_eq!(world.prompt(nyx).as_deref(), Some("nyx@hub $ "));
        world.command(nyx, "go alley");
        assert_eq!(
            world.prompt(nyx).as_deref(),
            Some("[integ 100] nyx@alley $ ")
        );
    }

    #[test]
    fn all_handles_in_a_darknet_share_one_copy() {
        let mut world = city();
        let nyx = inside(&mut world, "nyx");
        let (zer0, _) = world.join("zer0");
        world.command(zer0, "go alley");
        let outputs = world.command(zer0, "go alley");
        assert_eq!(heard(&outputs, nyx), ["zer0 arrives."]);
        assert!(
            heard(&outputs, zer0).contains(&"Here: nyx".to_owned()),
            "{outputs:?}"
        );
        assert_eq!(world.instance_id(nyx), world.instance_id(zer0));
        assert_eq!(world.instances.len(), 1);
        assert_eq!(world.mates(nyx), Vec::new(), "strangers are not crewmates");
    }

    #[test]
    fn noise_in_a_darknet_makes_no_trace() {
        let mut world = city();
        let nyx = inside(&mut world, "nyx");
        world.command(nyx, "scan");
        let trace = world
            .players
            .get(&nyx)
            .and_then(|state| state.run.as_ref())
            .map(|run| run.trace);
        assert_eq!(trace, Some(0));
    }
}
