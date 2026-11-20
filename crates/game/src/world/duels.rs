use ttymud_text::{Message, Style};

use super::matches::{End, Match, Mode, Outcome, Side};
use super::presence::named;
use super::{NetworkId, Output, PlayerId, Step, Thing, World, address, choose};

pub(super) const ANSWER: u64 = 60;
const USAGE: &str = "Usage: duel <handle> [credits].";

#[derive(Debug)]
pub(super) struct Challenge {
    kind: Mode,
    from: PlayerId,
    to: PlayerId,
    wager: u32,
    when: u64,
}

pub(super) struct Offer<'a> {
    pub(super) kind: Mode,
    pub(super) from: PlayerId,
    pub(super) to: PlayerId,
    pub(super) wager: Option<u32>,
    pub(super) whom: &'a str,
    pub(super) them: &'a str,
    pub(super) accept: String,
    pub(super) answer: String,
}

pub(super) fn wager_of(args: &[String], usage: &str) -> Result<(String, Option<u32>), Message> {
    match args {
        [word] => Ok((word.clone(), None)),
        [word, credits] => {
            let credits = credits.parse().map_err(|_| Message::error(usage))?;
            Ok((word.clone(), Some(credits)))
        }
        _ => Err(Message::error(usage)),
    }
}

impl World {
    pub(super) fn duel_names(&self, player: PlayerId) -> Vec<&str> {
        let online = self.online().into_iter();
        online
            .filter(|&(other, _)| other != player)
            .map(|(_, name)| name)
            .collect()
    }

    pub(super) fn arena_for(&self, player: PlayerId, what: &str) -> Result<NetworkId, Message> {
        let Some(arena) = self.arena else {
            return Err(Message::error("This city has no arena."));
        };
        if self.running(player) {
            return Err(Message::error(format!(
                "A {what} starts from the city. Leave the network first."
            )));
        }
        Ok(arena)
    }

    pub(super) fn duel(&mut self, player: PlayerId, args: &[String]) -> Result<Step, Message> {
        let arena = self.arena_for(player, "duel")?;
        let (word, wager) = wager_of(args, USAGE)?;
        let me = self.handle(player).unwrap_or_default().to_owned();
        if word.eq_ignore_ascii_case(&me) {
            return Err(Message::error("You cannot duel yourself."));
        }
        let names = self.duel_names(player);
        let name = choose(Thing::Handle, &word, &names)?.to_owned();
        let Some((rival, _)) = self.online().into_iter().find(|&(_, known)| known == name) else {
            return Ok(Step::default());
        };
        if self.running(rival) {
            return Err(Message::error(format!("{name} is in a network now.")));
        }
        let offer = Offer {
            kind: Mode::Duel,
            from: player,
            to: rival,
            wager,
            whom: "you",
            them: &name,
            accept: format!("duel {me}"),
            answer: format!("duel {name}"),
        };
        let Some(wager) = self.propose(offer)? else {
            let stake = stake(wager.unwrap_or(0));
            return Ok(Step::show(vec![
                Message::game("You challenge ")
                    .span(Style::Player, &name)
                    .text(format!(" to a duel{stake}. "))
                    .span(Style::Player, &name)
                    .text(format!(" has {ANSWER} seconds to answer.")),
            ]));
        };
        self.start_duel(arena, [rival, player], wager);
        Ok(Step::default())
    }

    pub(super) fn propose(&mut self, offer: Offer<'_>) -> Result<Option<u32>, Message> {
        let now = self.now;
        self.challenges
            .retain(|challenge| now.saturating_sub(challenge.when) <= ANSWER);
        let credits =
            |world: &Self, player| world.players.get(&player).map_or(0, |state| state.credits);
        let asked = self.challenges.iter().position(|challenge| {
            challenge.kind == offer.kind && challenge.from == offer.to && challenge.to == offer.from
        });
        let wager = match asked {
            Some(index) => self.challenges[index].wager,
            None => offer.wager.unwrap_or(0),
        };
        let (them, whom) = (offer.them, offer.whom);
        if asked.is_some() && offer.wager.is_some_and(|asked| asked != wager) {
            let answer = &offer.answer;
            return Err(Message::error(format!(
                "{them} challenged {whom} for {wager} cr. Type {answer} to accept."
            )));
        }
        let have = credits(self, offer.from);
        if have < wager {
            return Err(Message::error(format!("You have only {have} cr.")));
        }
        if let Some(index) = asked {
            if credits(self, offer.to) < wager {
                return Err(Message::error(format!("{them} cannot pay the stake now.")));
            }
            self.challenges.remove(index);
            return Ok(Some(wager));
        }
        self.challenges.retain(|challenge| {
            challenge.kind != offer.kind || challenge.from != offer.from || challenge.to != offer.to
        });
        self.challenges.push(Challenge {
            kind: offer.kind,
            from: offer.from,
            to: offer.to,
            wager,
            when: now,
        });
        let me = self.handle(offer.from).unwrap_or_default().to_owned();
        let what = offer.kind.word();
        let asks = named(
            &me,
            &format!(" challenges {whom} to a {what}{}. Type ", stake(wager)),
        )
        .span(Style::Command, offer.accept)
        .text(format!(" in the next {ANSWER} seconds to accept."));
        self.outbox.push(Output {
            player: offer.to,
            message: asks,
        });
        Ok(None)
    }

    fn start_duel(&mut self, arena: NetworkId, fighters: [PlayerId; 2], wager: u32) {
        let names = fighters.map(|player| self.handle(player).unwrap_or_default().to_owned());
        let sides = [0, 1].map(|index| Side {
            name: names[index].clone(),
            payer: fighters[index],
            fighters: self.fighter(fighters[index]).into_iter().collect(),
            captures: 0,
        });
        self.open_match(arena, Mode::Duel, sides, wager);
        let network = self.networks[arena.0].name.clone();
        let stake = if wager > 0 {
            format!(" The winner takes {wager} cr from the loser.")
        } else {
            String::new()
        };
        let minutes = Mode::Duel.minutes();
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

pub(super) fn stake(wager: u32) -> String {
    if wager > 0 {
        format!(" for {wager} cr")
    } else {
        String::new()
    }
}

pub(super) fn result(game: &Match, outcome: Outcome, side: usize) -> Vec<Message> {
    let (other, wager) = (&game.sides[1 - side].name, game.wager);
    let player = |text: &str| Message::game(text).span(Style::Player, other);
    let line = match outcome {
        Outcome::Draw => player("Time is up. The duel against ").text(" is a draw."),
        Outcome::Win(winner, end) if winner == side => {
            let how = match end {
                End::GiveUp => named(other, " gives up."),
                End::Gone => named(other, " disconnects."),
                End::Crash | End::Flags | End::Time => player("The rig of ").text(" crashes."),
            };
            let won = how.text(" You win the duel");
            if wager > 0 {
                won.text(format!(" and take {wager} cr from "))
                    .span(Style::Player, other)
                    .text(".")
            } else {
                won.text(".")
            }
        }
        Outcome::Win(_, end) => {
            let lost = match end {
                End::GiveUp => player("You give up the duel against "),
                End::Gone => return Vec::new(),
                End::Crash | End::Flags | End::Time => {
                    player("Your rig crashes. You lose the duel against ")
                }
            };
            if wager > 0 {
                lost.text(format!(" and pay {wager} cr to "))
                    .span(Style::Player, other)
                    .text(".")
            } else {
                lost.text(".")
            }
        }
    };
    vec![line]
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
