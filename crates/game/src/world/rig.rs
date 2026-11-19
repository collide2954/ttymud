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
    use crate::testing::run;

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
