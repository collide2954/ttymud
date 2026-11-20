use super::ice::Ice;
use super::soc::Analyst;
use super::{HostId, NetworkId, PlayerId, World};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct InstanceId(u64);

#[derive(Debug)]
pub(super) struct Instance {
    pub(super) network: NetworkId,
    pub(super) ice: Vec<Ice>,
    pub(super) analysts: Vec<Analyst>,
    pub(super) logs: Vec<(HostId, u32)>,
}

impl World {
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
    use crate::testing::world;

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
