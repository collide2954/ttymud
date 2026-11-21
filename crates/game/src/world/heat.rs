use ttymud_text::Message;

use super::{PlayerId, World};
use crate::heat::{COOLDOWN, MAX_HEAT};

pub(super) const FAST_HEAT: u32 = 3;

impl World {
    pub(super) fn add_heat(&mut self, player: PlayerId, amount: u32) -> Message {
        let now = self.now;
        let Some(state) = self.players.get_mut(&player) else {
            return Message::game("");
        };
        let before = state.heat;
        state.heat = (before + amount).min(MAX_HEAT);
        state.cooled = now;
        let after = state.heat;
        Message::game(format!("Your heat: {before} -> {after}."))
    }

    pub(super) fn cool(&mut self, player: PlayerId) -> Option<(u32, u32)> {
        let now = self.now;
        let state = self.players.get_mut(&player)?;
        let days = now.saturating_sub(state.cooled) / COOLDOWN;
        if state.heat == 0 || days == 0 {
            return None;
        }
        let before = state.heat;
        state.heat -= u32::try_from(days).unwrap_or(u32::MAX).min(before);
        state.cooled += days * COOLDOWN;
        Some((before, state.heat))
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
        world.command(player, "go back-room");
        let shown = plain(&world.command(player, "go gate"));
        assert_eq!(
            shown.last().map(String::as_str),
            Some("Unknown ICE starts to compile ALERT (4 ticks).")
        );
    }

    const DAY: u64 = 86_400;

    fn heated(heat: u32, since: u64) -> (World, PlayerId) {
        let (mut world, player) = joined();
        world.set_time(since);
        if let Some(state) = world.players.get_mut(&player) {
            state.heat = heat;
            state.cooled = since;
        }
        (world, player)
    }

    fn tick_at(world: &mut World, now: u64) -> Vec<String> {
        world.set_time(now);
        plain(&world.tick())
    }

    #[test]
    fn heat_cools_one_level_each_day_without_a_trace() {
        let start = 1_000_000;
        let (mut world, player) = heated(2, start);
        assert_eq!(tick_at(&mut world, start + DAY - 1), Vec::<String>::new());
        assert_eq!(
            tick_at(&mut world, start + DAY),
            ["Your heat cools down: 2 -> 1."]
        );
        assert_eq!(
            tick_at(&mut world, start + 3 * DAY),
            ["Your heat cools down: 1 -> 0."]
        );
        assert_eq!(heat(&mut world, player), ["Heat: 0/5"]);
    }

    #[test]
    fn new_heat_starts_the_day_again() {
        let start = 1_000_000;
        let (mut world, player) = heated(1, start);
        world.set_time(start + DAY - 10);
        for line in ["go back-room", "go gate"] {
            world.command(player, line);
        }
        set_trace(&mut world, player, 80);
        world.command(player, "jackout");
        assert_eq!(tick_at(&mut world, start + DAY + 10), Vec::<String>::new());
        assert_eq!(heat(&mut world, player), ["Heat: 2/5"]);
    }

    #[test]
    fn heat_cools_while_the_account_is_away() {
        let mut world = world();
        let (player, _) = world.connect("a");
        world.command(player, "nyx");
        world.command(player, "ghost");
        if let Some(state) = world.players.get_mut(&player) {
            state.heat = 3;
        }
        world.leave(player);
        world.set_time(2 * DAY + 5);
        let (player, outputs) = world.connect("a");
        assert!(
            plain(&outputs)
                .contains(&"Your heat cooled down while you were away: 3 -> 1.".to_owned()),
            "{:?}",
            plain(&outputs)
        );
        assert_eq!(heat(&mut world, player), ["Heat: 1/5"]);
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
