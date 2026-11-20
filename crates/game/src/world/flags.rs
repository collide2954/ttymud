use ttymud_text::{Message, Style};

use super::duels::{ANSWER, Offer, stake, wager_of};
use super::instance::tell;
use super::matches::{BACK, End, Match, Mode, Outcome, Side, flag};
use super::presence::named;
use super::rig::MAX_INTEGRITY;
use super::{NetworkId, PlayerId, Step, World, address, listed};
use crate::content::File;

const USAGE: &str = "Usage: match <crew> [credits].";
const CAPTURES: u32 = 3;

impl World {
    pub(super) fn crew_names(&self, player: PlayerId) -> Vec<&str> {
        let own = self.handle(player).and_then(|name| self.crews.key_of(name));
        let others = self.crews.list.iter();
        others
            .filter(|&(key, _)| own.as_ref() != Some(key))
            .map(|(_, crew)| crew.name.as_str())
            .collect()
    }

    pub(super) fn crew_match(
        &mut self,
        player: PlayerId,
        args: &[String],
    ) -> Result<Step, Message> {
        let arena = self.arena_for(player, "match")?;
        let me = self.handle(player).unwrap_or_default().to_owned();
        let own = self.led_crew(&me)?;
        let (word, wager) = wager_of(args, USAGE)?;
        if word.eq_ignore_ascii_case(&own) {
            return Err(Message::error("You cannot fight your own crew."));
        }
        let names = self.crew_names(player);
        let Some(name) = names.iter().find(|name| name.eq_ignore_ascii_case(&word)) else {
            let message = Message::error(format!("No crew named {word}. Crews: "));
            return Err(listed(message, Style::Player, names));
        };
        let (name, key) = ((*name).to_owned(), word.to_ascii_lowercase());
        let leader = self.crews.list[&key].leader.clone();
        let online = self.online();
        let found = online
            .into_iter()
            .find(|&(_, known)| known.eq_ignore_ascii_case(&leader));
        let Some((chief, _)) = found else {
            return Err(Message::error(format!(
                "{leader}, the leader of {name}, is not online."
            )));
        };
        if self.running(chief) {
            return Err(Message::error(format!(
                "The leader of {name} is in a network now."
            )));
        }
        let crew = self.crews.list[&own].name.clone();
        let offer = Offer {
            kind: Mode::Flags,
            from: player,
            to: chief,
            wager,
            whom: "your crew",
            them: &leader,
            accept: format!("match {crew}"),
            answer: format!("match {name}"),
        };
        let Some(wager) = self.propose(offer)? else {
            let stake = stake(wager.unwrap_or(0));
            return Ok(Step::show(vec![
                Message::game(format!("You challenge the crew {name} to a match{stake}. "))
                    .span(Style::Player, &leader)
                    .text(format!(" has {ANSWER} seconds to answer.")),
            ]));
        };
        self.start_flags(arena, [(chief, key), (player, own)], wager);
        Ok(Step::default())
    }

    fn start_flags(&mut self, arena: NetworkId, crews: [(PlayerId, String); 2], wager: u32) {
        let sides = crews.map(|(leader, key)| {
            let crew = &self.crews.list[&key];
            let fighters = self
                .online()
                .into_iter()
                .filter(|&(member, name)| crew.has(name) && !self.running(member))
                .filter_map(|(member, _)| self.fighter(member))
                .collect();
            Side {
                name: crew.name.clone(),
                payer: leader,
                fighters,
                captures: 0,
            }
        });
        self.open_match(arena, Mode::Flags, sides, wager);
        let index = self.matches.len() - 1;
        let bases = self.networks[arena.0].bases.clone();
        let network = self.networks[arena.0].name.clone();
        let stake = if wager > 0 {
            format!(" The winning leader takes {wager} cr from the other leader.")
        } else {
            String::new()
        };
        let minutes = Mode::Flags.minutes();
        for side in 0..2 {
            let other = self.matches[index].sides[1 - side].name.clone();
            let (home, away) = (bases[side], bases[1 - side]);
            let rules = [
                Message::game("The match against ")
                    .span(Style::Player, &other)
                    .text(format!(" starts in {network}.{stake}")),
                Message::game("Take ")
                    .span(Style::File, flag(&other).name)
                    .text(" from ")
                    .span(Style::Host, &self.hosts[away.0].id)
                    .text(" and carry it to ")
                    .span(Style::Host, &self.hosts[home.0].id)
                    .text(format!(
                        ". {CAPTURES} captures win. After {minutes} minutes, the crew with more captures wins."
                    )),
            ];
            let players: Vec<PlayerId> = self.matches[index].sides[side]
                .fighters
                .iter()
                .map(|fighter| fighter.player)
                .collect();
            for player in players {
                let mut shown = rules.to_vec();
                shown.extend(self.scene(home, player));
                self.outbox.extend(address(player, shown));
            }
        }
    }

    pub(super) fn own_flag(&self, player: PlayerId, file: &File) -> Option<Message> {
        let (index, side) = self.match_of(player)?;
        let game = &self.matches[index];
        let own = game.mode == Mode::Flags && file.name == flag(&game.sides[side].name).name;
        own.then(|| Message::error("This is the flag of your crew. Guard it."))
    }

    pub(super) fn capture(&mut self, player: PlayerId) {
        let Some((index, side)) = self.match_of(player) else {
            return;
        };
        let game = &self.matches[index];
        let bases = &self.networks[game.network.0].bases;
        if game.mode != Mode::Flags || game.outcome.is_some() || self.here(player) != bases[side] {
            return;
        }
        let (enemy, copy, away) = (
            game.sides[1 - side].name.clone(),
            game.copy,
            bases[1 - side],
        );
        let goal = flag(&enemy).name;
        let Some(state) = self.players.get_mut(&player) else {
            return;
        };
        let Some(at) = state.rig.files.iter().position(|file| file.name == goal) else {
            return;
        };
        let file = state.rig.files.remove(at);
        let name = state.name.clone();
        if let Some(instance) = self.instances.get_mut(&copy) {
            instance.files.push((away, file));
        }
        let sides = &mut self.matches[index].sides;
        sides[side].captures += 1;
        let score = format!(
            "{} {}, {} {}.",
            sides[0].name, sides[0].captures, sides[1].name, sides[1].captures
        );
        let won = sides[side].captures >= CAPTURES;
        let line = named(&name, &format!(" captures the flag of {enemy}. {score}"));
        let everyone = self.fighters_of(index);
        self.outbox.extend(tell(&everyone, &line));
        if won {
            self.win(index, side, End::Flags);
        }
    }

    fn return_flag(&mut self, index: usize, side: usize, player: PlayerId) -> Option<Message> {
        let game = &self.matches[index];
        let (enemy, copy) = (game.sides[1 - side].name.clone(), game.copy);
        let away = self.networks[game.network.0].bases[1 - side];
        let goal = flag(&enemy).name;
        let state = self.players.get_mut(&player)?;
        let at = state.rig.files.iter().position(|file| file.name == goal)?;
        let file = state.rig.files.remove(at);
        self.instances.get_mut(&copy)?.files.push((away, file));
        let line = Message::game("The flag of ")
            .span(Style::Player, &enemy)
            .text(" goes back to its base.");
        let mut others = self.fighters_of(index);
        others.retain(|&other| other != player);
        self.outbox.extend(tell(&others, &line));
        Some(line)
    }

    pub(super) fn respawn(&mut self, index: usize, side: usize, player: PlayerId) -> Vec<Message> {
        let base = self.networks[self.matches[index].network.0].bases[side];
        let here = self.here(player);
        let name = self.handle(player).unwrap_or_default().to_owned();
        let crashes = Message::game("The rig of ")
            .span(Style::Player, &name)
            .text(" crashes.");
        self.notify(here, player, &crashes);
        let returned = self.return_flag(index, side, player);
        let Some(state) = self.players.get_mut(&player) else {
            return Vec::new();
        };
        let start = state
            .run
            .as_ref()
            .map_or(state.chain.len(), |run| run.start);
        state.chain.truncate(start);
        state.chain.push(base);
        state.rig.integrity = MAX_INTEGRITY;
        state.rig.cycles = 0;
        let mut shown = vec![
            Message::game("Your rig crashes. You wake up at ")
                .span(Style::Host, &self.hosts[base.0].id)
                .text(" with full integrity and 0 cycles."),
        ];
        shown.extend(self.scene(base, player));
        shown.extend(returned);
        self.appear(player, base);
        shown
    }

    pub(super) fn leave_flags(
        &mut self,
        index: usize,
        side: usize,
        player: PlayerId,
    ) -> Vec<Message> {
        self.return_flag(index, side, player);
        let fighters = &mut self.matches[index].sides[side].fighters;
        let Some(at) = fighters.iter().position(|fighter| fighter.player == player) else {
            return Vec::new();
        };
        let fighter = fighters.remove(at);
        let name = self.handle(player).unwrap_or_default().to_owned();
        let everyone = self.fighters_of(index);
        self.outbox
            .extend(tell(&everyone, &named(&name, " leaves the match.")));
        self.give_back(&fighter);
        let here = self.here(player);
        let mut shown = vec![Message::game("You leave the match."), Message::game(BACK)];
        shown.extend(self.scene(here, player));
        self.appear(player, here);
        self.touch(player);
        shown
    }
}

pub(super) fn result(
    game: &Match,
    outcome: Outcome,
    side: usize,
    player: PlayerId,
) -> Vec<Message> {
    let (mine, other) = (&game.sides[side], &game.sides[1 - side]);
    let score = format!(", {} to {}.", mine.captures, other.captures);
    let crew = |text: &str| Message::game(text).span(Style::Player, &other.name);
    let line = match outcome {
        Outcome::Draw => crew("Time is up. The match against ").text(format!(" is a draw{score}")),
        Outcome::Win(winner, end) if winner == side => match end {
            End::Flags => crew(&format!(
                "Your crew captures {CAPTURES} flags and wins the match against "
            ))
            .text("."),
            End::Time => crew("Time is up. Your crew wins the match against ").text(score),
            End::Gone => crew("The leader of ").text(" disconnects. Your crew wins the match."),
            End::GiveUp | End::Crash => named(&other.name, " gives up. Your crew wins the match."),
        },
        Outcome::Win(_, End::Gone) if player == mine.payer => return Vec::new(),
        Outcome::Win(_, end) => match end {
            End::Flags => named(
                &other.name,
                &format!(" captures {CAPTURES} flags. Your crew loses the match."),
            ),
            End::Time => crew("Time is up. Your crew loses the match against ").text(score),
            End::Gone => {
                crew("Your leader disconnects. Your crew loses the match against ").text(".")
            }
            End::GiveUp | End::Crash => crew("Your crew gives up the match against ").text("."),
        },
    };
    let mut shown = vec![line];
    if let Outcome::Win(winner, _) = outcome
        && game.wager > 0
        && player == mine.payer
    {
        let wager = game.wager;
        shown.push(if winner == side {
            crew(&format!("You take {wager} cr from the leader of ")).text(".")
        } else {
            crew(&format!("You pay {wager} cr to the leader of ")).text(".")
        });
    }
    shown
}

#[cfg(test)]
mod tests {
    use crate::testing::{dark, heard, plain};
    use crate::{PlayerId, World};

    const BACK: &str = "The arena gives back your integrity, cycles, and files.";

    struct Crews {
        world: World,
        nyx: PlayerId,
        ghost: PlayerId,
        zer0: PlayerId,
    }

    fn crews() -> Crews {
        let mut world = dark();
        let (nyx, _) = world.join("nyx");
        let (ghost, _) = world.join("ghost");
        let (zer0, _) = world.join("zer0");
        world.command(nyx, "crew create Moths");
        world.command(nyx, "crew invite ghost");
        world.command(ghost, "crew join moths");
        world.command(zer0, "crew create Owls");
        for player in [nyx, ghost, zer0] {
            if let Some(state) = world.players.get_mut(&player) {
                state.credits = 300;
            }
        }
        Crews {
            world,
            nyx,
            ghost,
            zer0,
        }
    }

    fn started() -> Crews {
        let mut crews = crews();
        crews.world.command(crews.nyx, "match owls 100");
        crews.world.command(crews.zer0, "match moths");
        crews
    }

    fn credits(world: &World, player: PlayerId) -> u32 {
        world.players.get(&player).map_or(0, |state| state.credits)
    }

    fn from(lines: &[String], first: &str) -> Vec<String> {
        let start = lines
            .iter()
            .position(|line| line == first)
            .unwrap_or(lines.len());
        lines[start..].to_vec()
    }

    fn walk(world: &mut World, player: PlayerId, lines: &[&str]) -> Vec<crate::Output> {
        let mut outputs = Vec::new();
        for line in lines {
            outputs = world.command(player, line);
        }
        outputs
    }

    #[test]
    fn a_match_starts_when_the_other_leader_answers_and_takes_the_crewmates_in_the_city() {
        let Crews {
            mut world,
            nyx,
            ghost,
            zer0,
        } = crews();
        let outputs = world.command(nyx, "match owls 100");
        assert_eq!(
            heard(&outputs, nyx),
            ["You challenge the crew Owls to a match for 100 cr. zer0 has 60 seconds to answer."]
        );
        assert_eq!(
            heard(&outputs, zer0),
            [
                "nyx challenges your crew to a match for 100 cr. Type match Moths in the next 60 seconds to accept."
            ]
        );
        let outputs = world.command(zer0, "match moths");
        assert_eq!(
            heard(&outputs, ghost)[..3],
            [
                "The match against Owls starts in The Grid. The winning leader takes 100 cr from the other leader.",
                "Take owls.flag from east and carry it to west. 3 captures win. After 15 minutes, the crew with more captures wins.",
                "West",
            ]
        );
        assert_eq!(
            world.prompt(ghost).as_deref(),
            Some("[integ 100] ghost@west $ ")
        );
        assert_eq!(
            world.prompt(zer0).as_deref(),
            Some("[integ 100] zer0@east $ ")
        );
        assert_eq!(plain(&world.command(nyx, "ls")), ["mid/", "moths.flag"]);
    }

    #[test]
    fn a_crew_scores_when_it_carries_the_enemy_flag_to_its_own_base() {
        let Crews {
            mut world,
            nyx,
            ghost,
            zer0,
        } = started();
        let lines = ["go mid", "go east", "get owls.flag", "back"];
        assert_eq!(
            heard(&walk(&mut world, nyx, &lines[..3]), nyx),
            ["Downloaded owls.flag."]
        );
        assert_eq!(plain(&world.command(zer0, "ls")), ["mid/"]);
        walk(&mut world, nyx, &lines[3..]);
        let outputs = world.command(nyx, "back");
        let score = "nyx captures the flag of Owls. Moths 1, Owls 0.";
        for player in [nyx, ghost, zer0] {
            assert!(
                heard(&outputs, player).contains(&score.to_owned()),
                "{outputs:?}"
            );
        }
        assert_eq!(plain(&world.command(zer0, "ls")), ["mid/", "owls.flag"]);
        assert_eq!(
            plain(&world.command(nyx, "inventory")),
            ["Your rig has no files."]
        );
        assert_eq!(
            plain(&world.command(nyx, "get moths.flag")),
            ["This is the flag of your crew. Guard it."]
        );
    }

    #[test]
    fn a_crash_sends_the_handle_to_its_base_and_the_flag_back_to_its_own() {
        let Crews {
            mut world,
            nyx,
            zer0,
            ..
        } = started();
        walk(&mut world, nyx, &["go mid", "go east", "get owls.flag"]);
        if let Some(state) = world.players.get_mut(&nyx) {
            state.rig.integrity = 10;
        }
        let outputs = world.command(zer0, "crack nyx");
        assert_eq!(
            heard(&outputs, nyx)[..4],
            [
                "zer0 cracks your rig. Integrity: 10 -> 0.",
                "Your rig crashes. You wake up at west with full integrity and 0 cycles.",
                "West",
                "The west base.",
            ]
        );
        assert!(
            heard(&outputs, zer0).contains(&"The flag of Owls goes back to its base.".to_owned()),
            "{outputs:?}"
        );
        assert_eq!(plain(&world.command(zer0, "ls")), ["mid/", "owls.flag"]);
        let state = world
            .players
            .get(&nyx)
            .map(|state| (state.rig.integrity, state.rig.cycles));
        assert_eq!(state, Some((100, 0)));
        assert_eq!(
            world.prompt(nyx).as_deref(),
            Some("[integ 100] nyx@west $ ")
        );
    }

    #[test]
    fn three_captures_win_the_match_and_the_wager() {
        let Crews {
            mut world,
            nyx,
            ghost,
            zer0,
        } = started();
        world.matches[0].sides[0].captures = 2;
        walk(
            &mut world,
            nyx,
            &["go mid", "go east", "get owls.flag", "back"],
        );
        let outputs = world.command(nyx, "back");
        let won = "Your crew captures 3 flags and wins the match against Owls.";
        assert_eq!(
            from(&heard(&outputs, nyx), won)[..3],
            [won, "You take 100 cr from the leader of Owls.", BACK]
        );
        assert_eq!(from(&heard(&outputs, ghost), won)[..2], [won, BACK]);
        let lost = "Moths captures 3 flags. Your crew loses the match.";
        assert_eq!(
            from(&heard(&outputs, zer0), lost)[..2],
            [lost, "You pay 100 cr to the leader of Moths."]
        );
        assert_eq!((credits(&world, nyx), credits(&world, zer0)), (400, 200));
        assert_eq!(world.prompt(ghost).as_deref(), Some("ghost@hub $ "));
        assert!(world.matches.is_empty());
    }

    #[test]
    fn after_15_minutes_the_crew_with_more_captures_wins() {
        let Crews {
            mut world,
            nyx,
            zer0,
            ..
        } = started();
        world.matches[0].sides[1].captures = 1;
        world.set_time(899);
        world.tick();
        assert_eq!(world.matches.len(), 1);
        world.set_time(900);
        let outputs = world.tick();
        assert_eq!(
            heard(&outputs, zer0)[0],
            "Time is up. Your crew wins the match against Moths, 1 to 0."
        );
        assert_eq!(
            heard(&outputs, nyx)[0],
            "Time is up. Your crew loses the match against Owls, 0 to 1."
        );
        let mut tie = started();
        tie.world.set_time(900);
        let outputs = tie.world.tick();
        assert_eq!(
            heard(&outputs, tie.nyx)[..2],
            [
                "Time is up. The match against Owls is a draw, 0 to 0.",
                BACK
            ]
        );
    }

    #[test]
    fn a_crewmate_can_leave_but_a_leader_who_leaves_gives_up() {
        let Crews {
            mut world,
            nyx,
            ghost,
            zer0,
        } = started();
        let outputs = world.command(ghost, "jackout");
        assert_eq!(
            heard(&outputs, ghost)[..3],
            ["You leave the match.", BACK, "Hub"]
        );
        assert_eq!(heard(&outputs, nyx), ["ghost leaves the match."]);
        assert_eq!(world.matches.len(), 1);
        let outputs = world.command(nyx, "jackout");
        assert_eq!(
            heard(&outputs, zer0)[..2],
            [
                "Moths gives up. Your crew wins the match.",
                "You take 100 cr from the leader of Moths."
            ]
        );
        assert_eq!(
            heard(&outputs, nyx)[..2],
            [
                "Your crew gives up the match against Owls.",
                "You pay 100 cr to the leader of Owls."
            ]
        );
    }

    #[test]
    fn only_a_leader_starts_a_match_against_another_crew() {
        let Crews {
            mut world,
            nyx,
            ghost,
            ..
        } = crews();
        assert_eq!(
            plain(&world.command(ghost, "match owls")),
            ["Only the leader of Moths can do that. The leader is nyx."]
        );
        assert_eq!(
            plain(&world.command(nyx, "match moths")),
            ["You cannot fight your own crew."]
        );
        assert_eq!(
            plain(&world.command(nyx, "match bats")),
            ["No crew named bats. Crews: Owls"]
        );
        assert_eq!(world.complete(nyx, "match ").names, ["Owls"]);
        let (moth, _) = world.join("moth");
        assert_eq!(
            plain(&world.command(moth, "match owls")),
            ["You are not in a crew. Type crew create <name> to make one."]
        );
    }
}
