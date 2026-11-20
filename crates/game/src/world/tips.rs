use ttymud_text::{Message, Style};

use super::{Output, PlayerId, Step, Thing, World, choose, usage};
use crate::content::Zone;
use crate::heat::MAX_HEAT;

const DAY: u64 = 86_400;

impl World {
    pub(super) fn in_darknet(&self, player: PlayerId) -> bool {
        self.run_zone(player) == Some(Zone::Darknet)
    }

    pub(super) fn tip_targets(&self, player: PlayerId) -> Vec<(PlayerId, &str)> {
        if !self.in_darknet(player) {
            return Vec::new();
        }
        let strangers = self.strangers(player).into_iter();
        strangers.map(|(other, _, name)| (other, name)).collect()
    }

    pub(super) fn tip(&mut self, player: PlayerId, args: &[String]) -> Result<Step, Message> {
        if !self.in_darknet(player) {
            return Err(Message::error(
                "You can tip off the police only from a darknet.",
            ));
        }
        let targets = self.tip_targets(player);
        let names: Vec<&str> = targets.iter().map(|&(_, name)| name).collect();
        let [word] = args else {
            return Err(usage("tip <handle>", Thing::Target, &names));
        };
        self.spare_crew(player, word)?;
        let name = choose(Thing::Target, word, &names)?.to_owned();
        let Some(&(target, _)) = targets.iter().find(|&&(_, known)| known == name) else {
            return Ok(Step::default());
        };
        let now = self.now;
        let Some(state) = self.players.get_mut(&target) else {
            return Ok(Step::default());
        };
        if state
            .tipped
            .is_some_and(|when| now.saturating_sub(when) < DAY)
        {
            return Err(Message::error(format!(
                "The police already have a tip about {name} from the last 24 hours."
            )));
        }
        state.tipped = Some(now);
        let heat = state.heat;
        let told = Message::game("Someone tips off the police about you. ");
        let told = if heat + 1 < MAX_HEAT {
            told.text(self.add_heat(target, 1).plain())
        } else {
            told.text(format!("Your heat stays at {heat}."))
        };
        self.outbox.push(Output {
            player: target,
            message: told,
        });
        self.touch(target);
        Ok(Step::show(vec![
            Message::game("You tip off the police about ")
                .span(Style::Player, &name)
                .text("."),
        ]))
    }
}

#[cfg(test)]
mod tests {
    use crate::testing::{dark, hacker, heard, inside, plain};
    use crate::{PlayerId, World};

    const DAY: u64 = 86_400;

    fn heat(world: &World, player: PlayerId) -> u32 {
        world.players.get(&player).map_or(0, |state| state.heat)
    }

    #[test]
    fn a_tip_adds_one_heat_to_a_handle_anywhere_in_the_darknet() {
        let mut world = dark();
        let nyx = inside(&mut world, "nyx");
        let zer0 = inside(&mut world, "zer0");
        world.command(zer0, "go den");
        let outputs = world.command(nyx, "tip zer0");
        assert_eq!(heard(&outputs, nyx), ["You tip off the police about zer0."]);
        assert_eq!(
            heard(&outputs, zer0),
            ["Someone tips off the police about you. Your heat: 0 -> 1."]
        );
        assert_eq!(heat(&world, zer0), 1);
    }

    #[test]
    fn a_handle_gets_one_tip_in_24_hours() {
        let mut world = dark();
        let nyx = inside(&mut world, "nyx");
        let zer0 = inside(&mut world, "zer0");
        let ghost = inside(&mut world, "ghost");
        world.command(nyx, "tip zer0");
        let again = ["The police already have a tip about zer0 from the last 24 hours."];
        assert_eq!(plain(&world.command(ghost, "tip zer0")), again);
        world.set_time(DAY - 1);
        assert_eq!(plain(&world.command(nyx, "tip zer0")), again);
        world.set_time(DAY);
        world.command(ghost, "tip zer0");
        assert_eq!(heat(&world, zer0), 2);
    }

    #[test]
    fn a_tip_never_takes_the_heat_from_4_to_5() {
        let mut world = dark();
        let nyx = inside(&mut world, "nyx");
        let zer0 = inside(&mut world, "zer0");
        if let Some(state) = world.players.get_mut(&zer0) {
            state.heat = 4;
        }
        let outputs = world.command(nyx, "tip zer0");
        assert_eq!(heard(&outputs, nyx), ["You tip off the police about zer0."]);
        assert_eq!(
            heard(&outputs, zer0),
            ["Someone tips off the police about you. Your heat stays at 4."]
        );
        assert_eq!(heat(&world, zer0), 4);
    }

    #[test]
    fn tips_work_only_between_strangers_in_a_darknet() {
        let mut world = dark();
        let (nyx, _) = world.join("nyx");
        let zer0 = inside(&mut world, "zer0");
        assert_eq!(
            plain(&world.command(nyx, "tip zer0")),
            ["You can tip off the police only from a darknet."]
        );
        for _ in 0..2 {
            world.command(nyx, "go alley");
        }
        assert_eq!(
            plain(&world.command(nyx, "tip")),
            ["Usage: tip <handle>. Targets: zer0"]
        );
        assert_eq!(world.complete(nyx, "tip ").names, ["zer0"]);
        world.command(zer0, "crew create Moths");
        world.command(zer0, "crew invite nyx");
        world.command(nyx, "crew join moths");
        assert_eq!(
            plain(&world.command(nyx, "tip zer0")),
            ["zer0 is in your crew. Crewmates cannot attack each other."]
        );
        assert_eq!(heat(&world, zer0), 0);
    }

    #[test]
    fn the_save_of_a_handle_keeps_the_time_of_its_last_tip() {
        let mut world = dark();
        world.set_time(1_000);
        let nyx = hacker(&mut world, "a", "nyx");
        let zer0 = hacker(&mut world, "b", "zer0");
        for player in [nyx, zer0] {
            for _ in 0..2 {
                world.command(player, "go alley");
            }
        }
        world.command(nyx, "tip zer0");
        world.leave(zer0);
        let saves = world.saves();
        let text = saves
            .iter()
            .find(|(path, _)| path == "accounts/b.ini")
            .map_or("", |(_, text)| text.as_str());
        assert!(text.contains("\ntipped = 1000\n"), "{text}");
        let mut fresh = dark();
        for (path, text) in &saves {
            fresh.restore(path, text).unwrap();
        }
        let (zer0, _) = fresh.connect("b");
        let tipped = fresh.players.get(&zer0).and_then(|state| state.tipped);
        assert_eq!(tipped, Some(1_000));
    }
}
