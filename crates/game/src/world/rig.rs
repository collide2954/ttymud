use ttymud_text::Message;

use super::{PlayerId, Step, World, lines};
use crate::content::File;

const MAX_INTEGRITY: u32 = 100;
const MAX_CYCLES: u32 = 4;

#[derive(Debug)]
pub(super) struct Rig {
    pub(super) integrity: u32,
    pub(super) cycles: u32,
    pub(super) files: Vec<File>,
}

impl Rig {
    pub(super) fn new() -> Self {
        Self {
            integrity: MAX_INTEGRITY,
            cycles: MAX_CYCLES,
            files: Vec::new(),
        }
    }

    pub(super) fn tick(&mut self) {
        self.cycles = (self.cycles + 1).min(MAX_CYCLES);
    }
}

impl World {
    pub(super) fn status(&self, player: PlayerId) -> Step {
        let Some(rig) = self.players.get(&player).map(|state| &state.rig) else {
            return Step::default();
        };
        lines(vec![
            Message::game(format!("Integrity: {}/{MAX_INTEGRITY}", rig.integrity)),
            Message::game(format!("Cycles: {}/{MAX_CYCLES}", rig.cycles)),
        ])
    }
}

#[cfg(test)]
mod tests {
    use crate::testing::{joined, plain, run};
    use crate::{PlayerId, World};

    fn cycles(world: &mut World, player: PlayerId) -> Vec<String> {
        plain(&world.command(player, "status | grep cycles"))
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

    #[test]
    fn status_shows_the_rig() {
        assert_eq!(run("status"), ["Integrity: 100/100", "Cycles: 4/4"]);
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
