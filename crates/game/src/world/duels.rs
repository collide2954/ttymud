use ttymud_text::{Message, Style};

use super::matches::{DUEL_TIME, Side};
use super::presence::named;
use super::{NetworkId, Output, PlayerId, Step, Thing, World, address, choose};

const ANSWER: u64 = 60;
const USAGE: &str = "Usage: duel <handle> [credits].";

#[derive(Debug)]
pub(super) struct Challenge {
    from: PlayerId,
    to: PlayerId,
    wager: u32,
    when: u64,
}

impl World {
    pub(super) fn duel_names(&self, player: PlayerId) -> Vec<&str> {
        let online = self.online().into_iter();
        online
            .filter(|&(other, _)| other != player)
            .map(|(_, name)| name)
            .collect()
    }

    pub(super) fn duel(&mut self, player: PlayerId, args: &[String]) -> Result<Step, Message> {
        let Some(arena) = self.arena else {
            return Err(Message::error("This city has no arena."));
        };
        if self.running(player) {
            return Err(Message::error(
                "A duel starts from the city. Leave the network first.",
            ));
        }
        let (word, wager) = match args {
            [word] => (word, None),
            [word, credits] => (
                word,
                Some(credits.parse().map_err(|_| Message::error(USAGE))?),
            ),
            _ => return Err(Message::error(USAGE)),
        };
        let me = self.handle(player).unwrap_or_default().to_owned();
        if word.eq_ignore_ascii_case(&me) {
            return Err(Message::error("You cannot duel yourself."));
        }
        let online: Vec<(PlayerId, &str)> = self
            .online()
            .into_iter()
            .filter(|&(other, _)| other != player)
            .collect();
        let names: Vec<&str> = online.iter().map(|&(_, name)| name).collect();
        let name = choose(Thing::Handle, word, &names)?.to_owned();
        let Some(&(rival, _)) = online.iter().find(|&&(_, known)| known == name) else {
            return Ok(Step::default());
        };
        if self.running(rival) {
            return Err(Message::error(format!("{name} is in a network now.")));
        }
        let now = self.now;
        self.challenges
            .retain(|challenge| now.saturating_sub(challenge.when) <= ANSWER);
        let credits =
            |world: &Self, player| world.players.get(&player).map_or(0, |state| state.credits);
        let asked = self
            .challenges
            .iter()
            .position(|challenge| challenge.from == rival && challenge.to == player);
        if let Some(index) = asked {
            let offered = self.challenges[index].wager;
            if wager.is_some_and(|wager| wager != offered) {
                return Err(Message::error(format!(
                    "{name} challenged you for {offered} cr. Type duel {name} to accept."
                )));
            }
            let have = credits(self, player);
            if have < offered {
                return Err(Message::error(format!("You have only {have} cr.")));
            }
            if credits(self, rival) < offered {
                return Err(Message::error(format!("{name} cannot pay the stake now.")));
            }
            self.challenges.remove(index);
            self.start_duel(arena, [rival, player], offered);
            return Ok(Step::default());
        }
        let wager = wager.unwrap_or(0);
        let have = credits(self, player);
        if have < wager {
            return Err(Message::error(format!("You have only {have} cr.")));
        }
        self.challenges
            .retain(|challenge| challenge.from != player || challenge.to != rival);
        self.challenges.push(Challenge {
            from: player,
            to: rival,
            wager,
            when: now,
        });
        let stake = if wager > 0 {
            format!(" for {wager} cr")
        } else {
            String::new()
        };
        let asks = named(&me, &format!(" challenges you to a duel{stake}. Type "))
            .span(Style::Command, format!("duel {me}"))
            .text(format!(" in the next {ANSWER} seconds to accept."));
        self.outbox.push(Output {
            player: rival,
            message: asks,
        });
        Ok(Step::show(vec![
            Message::game("You challenge ")
                .span(Style::Player, &name)
                .text(format!(" to a duel{stake}. "))
                .span(Style::Player, &name)
                .text(format!(" has {ANSWER} seconds to answer.")),
        ]))
    }

    fn start_duel(&mut self, arena: NetworkId, fighters: [PlayerId; 2], wager: u32) {
        let names = fighters.map(|player| self.handle(player).unwrap_or_default().to_owned());
        let sides = [0, 1].map(|index| Side {
            name: names[index].clone(),
            payer: fighters[index],
            fighters: self.fighter(fighters[index]).into_iter().collect(),
        });
        self.open_match(arena, sides, wager);
        let network = self.networks[arena.0].name.clone();
        let stake = if wager > 0 {
            format!(" The winner takes {wager} cr from the loser.")
        } else {
            String::new()
        };
        let minutes = DUEL_TIME / 60;
        for (index, player) in fighters.into_iter().enumerate() {
            let other = &names[1 - index];
            let mut shown = vec![
                Message::game("The duel against ")
                    .span(Style::Player, other)
                    .text(format!(" starts in {network}.{stake}")),
                Message::game("Crash the rig of ")
                    .span(Style::Player, other)
                    .text(" to win. Type ")
                    .span(Style::Command, "jackout")
                    .text(format!(
                        " to give up. After {minutes} minutes, the duel is a draw."
                    )),
            ];
            shown.extend(self.scene(self.here(player), player));
            self.outbox.extend(address(player, shown));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::testing::{dark, hacker, heard, plain};
    use crate::{PlayerId, World};

    const RULES: &str = "Crash the rig of zer0 to win. Type jackout to give up. After 10 minutes, the duel is a draw.";
    const BACK: &str = "The arena gives back your integrity, cycles, and files.";

    fn credits(world: &World, player: PlayerId) -> u32 {
        world.players.get(&player).map_or(0, |state| state.credits)
    }

    fn ready() -> (World, PlayerId, PlayerId) {
        let mut world = dark();
        let (nyx, _) = world.join("nyx");
        let (zer0, _) = world.join("zer0");
        for player in [nyx, zer0] {
            if let Some(state) = world.players.get_mut(&player) {
                state.credits = 300;
            }
        }
        (world, nyx, zer0)
    }

    fn in_duel(wager: &str) -> (World, PlayerId, PlayerId) {
        let (mut world, nyx, zer0) = ready();
        world.command(nyx, &format!("duel zer0 {wager}"));
        world.command(zer0, "duel nyx");
        (world, nyx, zer0)
    }

    #[test]
    fn a_duel_starts_when_the_other_handle_answers_within_a_minute() {
        let (mut world, nyx, zer0) = ready();
        let outputs = world.command(nyx, "duel zer0 100");
        assert_eq!(
            heard(&outputs, nyx),
            ["You challenge zer0 to a duel for 100 cr. zer0 has 60 seconds to answer."]
        );
        assert_eq!(
            heard(&outputs, zer0),
            [
                "nyx challenges you to a duel for 100 cr. Type duel nyx in the next 60 seconds to accept."
            ]
        );
        let outputs = world.command(zer0, "duel nyx");
        let told = heard(&outputs, nyx);
        assert_eq!(
            told[..3],
            [
                "The duel against zer0 starts in The Grid. The winner takes 100 cr from the loser.",
                RULES,
                "West",
            ]
        );
        assert_eq!(heard(&outputs, zer0)[2], "East");
        assert_eq!(
            world.prompt(nyx).as_deref(),
            Some("[integ 100] nyx@west $ ")
        );
        assert_eq!(
            world.prompt(zer0).as_deref(),
            Some("[integ 100] zer0@east $ ")
        );
        assert_eq!(world.instance_id(nyx), world.instance_id(zer0));
    }

    #[test]
    fn a_challenge_lasts_one_minute() {
        let (mut world, nyx, zer0) = ready();
        world.command(nyx, "duel zer0");
        world.set_time(61);
        assert_eq!(
            plain(&world.command(zer0, "duel nyx"))[0],
            "You challenge nyx to a duel. nyx has 60 seconds to answer."
        );
        assert_eq!(world.prompt(zer0).as_deref(), Some("zer0@hub $ "));
    }

    #[test]
    fn the_winner_takes_the_wager_and_both_rigs_come_back() {
        let (mut world, nyx, zer0) = in_duel("100");
        for player in [nyx, zer0] {
            world.command(player, "go mid");
        }
        world.command(nyx, "get map.txt");
        if let Some(state) = world.players.get_mut(&zer0) {
            state.rig.integrity = 10;
            state.rig.cycles = 4;
        }
        let outputs = world.command(zer0, "crack nyx");
        assert_eq!(
            heard(&outputs, nyx),
            ["zer0 cracks your rig. Integrity: 100 -> 85."]
        );
        if let Some(state) = world.players.get_mut(&nyx) {
            state.rig.cycles = 4;
        }
        let outputs = world.command(nyx, "crack zer0");
        assert_eq!(
            heard(&outputs, nyx)[..4],
            [
                "crack hits zer0. Integrity: 10 -> 0.",
                "The rig of zer0 crashes. You win the duel and take 100 cr from zer0.",
                BACK,
                "Hub",
            ]
        );
        assert_eq!(
            heard(&outputs, zer0)[..3],
            [
                "nyx cracks your rig. Integrity: 10 -> 0.",
                "Your rig crashes. You lose the duel against nyx and pay 100 cr to nyx.",
                BACK,
            ]
        );
        assert_eq!((credits(&world, nyx), credits(&world, zer0)), (400, 200));
        assert_eq!(world.prompt(nyx).as_deref(), Some("nyx@hub $ "));
        assert_eq!(
            plain(&world.command(nyx, "status | grep integrity")),
            ["Integrity: 100/100"]
        );
        assert_eq!(
            plain(&world.command(nyx, "status | grep cycles")),
            ["Cycles: 4/4"]
        );
        assert_eq!(
            plain(&world.command(nyx, "inventory")),
            ["Your rig has no files."]
        );
        assert!(world.instances.is_empty());
        assert!(world.matches.is_empty());
    }

    #[test]
    fn jackout_gives_up_the_duel_without_a_shock() {
        let (mut world, nyx, zer0) = in_duel("100");
        let outputs = world.command(zer0, "jackout");
        assert_eq!(
            heard(&outputs, zer0)[..2],
            [
                "You give up the duel against nyx and pay 100 cr to nyx.",
                BACK
            ]
        );
        assert_eq!(
            heard(&outputs, nyx)[0],
            "zer0 gives up. You win the duel and take 100 cr from zer0."
        );
        assert_eq!(
            plain(&world.command(zer0, "status | grep integrity")),
            ["Integrity: 100/100"]
        );
    }

    #[test]
    fn a_disconnect_gives_up_the_duel() {
        let (mut world, nyx, zer0) = in_duel("100");
        let outputs = world.leave(zer0);
        assert_eq!(
            heard(&outputs, nyx)[0],
            "zer0 disconnects. You win the duel and take 100 cr from zer0."
        );
        assert_eq!(credits(&world, nyx), 400);
        assert!(world.matches.is_empty());
    }

    #[test]
    fn after_10_minutes_the_duel_is_a_draw() {
        let (mut world, nyx, zer0) = in_duel("100");
        world.set_time(599);
        assert_eq!(heard(&world.tick(), nyx), Vec::<String>::new());
        world.set_time(600);
        let outputs = world.tick();
        assert_eq!(
            heard(&outputs, nyx)[..2],
            ["Time is up. The duel against zer0 is a draw.", BACK]
        );
        assert_eq!((credits(&world, nyx), credits(&world, zer0)), (300, 300));
        assert_eq!(world.prompt(zer0).as_deref(), Some("zer0@hub $ "));
    }

    #[test]
    fn back_cannot_leave_the_arena() {
        let (mut world, nyx, _) = in_duel("");
        world.command(nyx, "go mid");
        assert_eq!(plain(&world.command(nyx, "back"))[0], "West");
        assert_eq!(
            plain(&world.command(nyx, "back")),
            ["You are in a duel. Type jackout to give up."]
        );
    }

    #[test]
    fn a_duel_needs_free_handles_and_enough_credits() {
        let (mut world, nyx, zer0) = ready();
        assert_eq!(
            plain(&world.command(nyx, "duel zer0 500")),
            ["You have only 300 cr."]
        );
        assert_eq!(
            plain(&world.command(nyx, "duel nyx")),
            ["You cannot duel yourself."]
        );
        assert_eq!(
            plain(&world.command(nyx, "duel zer0 lots")),
            ["Usage: duel <handle> [credits]."]
        );
        world.command(nyx, "duel zer0 100");
        assert_eq!(
            plain(&world.command(zer0, "duel nyx 50")),
            ["nyx challenged you for 100 cr. Type duel nyx to accept."]
        );
        for _ in 0..2 {
            world.command(zer0, "go alley");
        }
        assert_eq!(
            plain(&world.command(nyx, "duel zer0")),
            ["zer0 is in a network now."]
        );
        assert_eq!(
            plain(&world.command(zer0, "duel nyx")),
            ["A duel starts from the city. Leave the network first."]
        );
        let mut city = crate::testing::world();
        let (ghost, _) = city.join("ghost");
        city.join("moth");
        assert_eq!(
            plain(&city.command(ghost, "duel moth")),
            ["This city has no arena."]
        );
    }

    #[test]
    fn the_save_keeps_the_rig_from_before_the_duel() {
        let mut world = dark();
        let nyx = hacker(&mut world, "a", "nyx");
        let zer0 = hacker(&mut world, "b", "zer0");
        world.command(nyx, "duel zer0");
        world.command(zer0, "duel nyx");
        if let Some(state) = world.players.get_mut(&nyx) {
            state.rig.integrity = 10;
        }
        world.command(nyx, "status");
        let saves = world.saves();
        let text = saves
            .iter()
            .find(|(path, _)| path == "accounts/a.ini")
            .map_or("", |(_, text)| text.as_str());
        assert!(text.contains("\nintegrity = 100\n"), "{text}");
    }
}
