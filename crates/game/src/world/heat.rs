use ttymud_text::Message;

use super::{PlayerId, World};
use crate::heat::MAX_HEAT;

pub(super) const FAST_HEAT: u32 = 3;

impl World {
    pub(super) fn add_heat(&mut self, player: PlayerId, amount: u32) -> Message {
        let Some(state) = self.players.get_mut(&player) else {
            return Message::game("");
        };
        let before = state.heat;
        state.heat = (before + amount).min(MAX_HEAT);
        let after = state.heat;
        Message::game(format!("Your heat: {before} -> {after}."))
    }

    pub(super) fn heat_of(&self, player: PlayerId) -> u32 {
        self.players.get(&player).map_or(0, |state| state.heat)
    }
}

#[cfg(test)]
mod tests {
    use crate::testing::{joined, plain, world};
    use crate::{PlayerId, World};

    fn ready(lines: &[&str]) -> (World, PlayerId) {
        let (mut world, player) = joined();
        for line in lines {
            world.command(player, line);
        }
        (world, player)
    }

    fn set_trace(world: &mut World, player: PlayerId, trace: u32) {
        if let Some(run) = world
            .players
            .get_mut(&player)
            .and_then(|state| state.run.as_mut())
        {
            run.trace = trace;
        }
    }

    fn heat(world: &mut World, player: PlayerId) -> Vec<String> {
        plain(&world.command(player, "status | grep heat"))
    }

    #[test]
    fn a_trace_adds_heat_by_the_security_level() {
        let (mut world, player) = ready(&["go back-room", "go gate", "go core"]);
        set_trace(&mut world, player, 98);
        let shown = plain(&world.command(player, "scan"));
        assert!(
            shown.contains(&"Your heat: 0 -> 2.".to_owned()),
            "{shown:?}"
        );
        assert_eq!(heat(&mut world, player), ["Heat: 2/5"]);
    }

    #[test]
    fn a_late_jackout_adds_one_heat() {
        let (mut world, player) = ready(&["go back-room", "go gate"]);
        set_trace(&mut world, player, 80);
        let shown = plain(&world.command(player, "jackout"));
        assert_eq!(shown[1], "The trace was 75% or more. Your heat: 0 -> 1.");
    }

    #[test]
    fn an_early_jackout_adds_no_heat() {
        let (mut world, player) = ready(&["go back-room", "go gate", "jackout"]);
        assert_eq!(heat(&mut world, player), ["Heat: 0/5"]);
    }

    #[test]
    fn at_heat_three_sentries_alert_faster() {
        let (mut world, player) = joined();
        if let Some(state) = world.players.get_mut(&player) {
            state.heat = 3;
        }
        for line in ["go back-room", "go gate"] {
            world.command(player, line);
        }
        let shown = plain(&world.command(player, "scan"));
        assert_eq!(
            shown.last().map(String::as_str),
            Some("sentry starts to compile ALERT (2 ticks).")
        );
    }

    #[test]
    fn heat_stays_with_the_handle() {
        let mut world = world();
        let (player, _) = world.connect("a");
        world.command(player, "nyx");
        world.command(player, "ghost");
        if let Some(state) = world.players.get_mut(&player) {
            state.heat = 2;
        }
        world.leave(player);
        let (player, _) = world.connect("a");
        assert_eq!(heat(&mut world, player), ["Heat: 2/5"]);
    }
}
