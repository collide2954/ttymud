use ttymud_text::{Message, Style};

use super::ice::deploy;
use super::instance::{Instance, InstanceId};
use super::presence::named;
use super::run::Run;
use super::{HostId, NetworkId, PlayerId, World, address};
use crate::content::File;

pub(super) const DUEL_TIME: u64 = 600;
const BACK: &str = "The arena gives back your integrity, cycles, and files.";

#[derive(Debug)]
pub(super) struct Fighter {
    pub(super) player: PlayerId,
    pub(super) integrity: u32,
    cycles: u32,
    chain: Vec<HostId>,
    pub(super) files: Vec<File>,
}

#[derive(Debug)]
pub(super) struct Side {
    pub(super) name: String,
    pub(super) payer: PlayerId,
    pub(super) fighters: Vec<Fighter>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum End {
    Crash,
    GiveUp,
    Gone,
}

#[derive(Debug, Clone, Copy)]
enum Outcome {
    Win(usize, End),
    Draw,
}

#[derive(Debug)]
pub(super) struct Match {
    copy: InstanceId,
    sides: [Side; 2],
    wager: u32,
    start: u64,
    outcome: Option<Outcome>,
}

impl World {
    pub(super) fn fighter(&self, player: PlayerId) -> Option<Fighter> {
        let state = self.players.get(&player)?;
        Some(Fighter {
            player,
            integrity: state.rig.integrity,
            cycles: state.rig.cycles,
            chain: state.chain.clone(),
            files: state.rig.files.clone(),
        })
    }

    pub(super) fn open_match(&mut self, network: NetworkId, sides: [Side; 2], wager: u32) {
        let instance = Instance {
            network,
            ice: deploy(&self.hosts, network, 0),
            analysts: Vec::new(),
            logs: Vec::new(),
            bridges: Vec::new(),
            clock: 0,
        };
        let copy = self.open_instance(instance);
        let bases = self.networks[network.0].bases.clone();
        let mut moves = Vec::new();
        for (side, &base) in sides.iter().zip(&bases) {
            for fighter in &side.fighters {
                let from = self.here(fighter.player);
                if let Some(state) = self.players.get_mut(&fighter.player) {
                    state.chain.push(base);
                    state.run = Some(Run::new(copy, network, state.chain.len() - 1));
                }
                moves.push((fighter.player, from, base));
            }
        }
        for (player, from, base) in moves {
            self.depart(player, from, base);
        }
        self.matches.push(Match {
            copy,
            sides,
            wager,
            start: self.now,
            outcome: None,
        });
    }

    fn match_of(&self, player: PlayerId) -> Option<(usize, usize)> {
        self.matches.iter().enumerate().find_map(|(index, game)| {
            let side = game
                .sides
                .iter()
                .position(|side| side.fighters.iter().any(|fighter| fighter.player == player))?;
            Some((index, side))
        })
    }

    pub(super) fn in_match(&self, player: PlayerId) -> bool {
        self.match_of(player).is_some()
    }

    pub(super) fn saved_rig(&self, player: PlayerId) -> Option<&Fighter> {
        let (index, side) = self.match_of(player)?;
        let fighters = &self.matches[index].sides[side].fighters;
        fighters.iter().find(|fighter| fighter.player == player)
    }

    pub(super) fn foes(&self, player: PlayerId) -> Vec<(PlayerId, HostId, &str)> {
        let Some((index, side)) = self.match_of(player) else {
            return Vec::new();
        };
        let others = &self.matches[index].sides[1 - side].fighters;
        others
            .iter()
            .filter_map(|fighter| {
                let state = self.players.get(&fighter.player)?;
                Some((fighter.player, *state.chain.last()?, state.name.as_str()))
            })
            .collect()
    }

    pub(super) fn lose(&mut self, player: PlayerId, end: End) -> bool {
        let Some((index, side)) = self.match_of(player) else {
            return false;
        };
        let game = &mut self.matches[index];
        if game.outcome.is_none() {
            game.outcome = Some(Outcome::Win(1 - side, end));
        }
        true
    }

    pub(super) fn time_matches(&mut self) {
        let now = self.now;
        for game in &mut self.matches {
            if game.outcome.is_none() && now.saturating_sub(game.start) >= DUEL_TIME {
                game.outcome = Some(Outcome::Draw);
            }
        }
    }

    pub(super) fn settle_matches(&mut self) {
        if self.matches.iter().all(|game| game.outcome.is_none()) {
            return;
        }
        let (done, open) = std::mem::take(&mut self.matches)
            .into_iter()
            .partition(|game| game.outcome.is_some());
        self.matches = open;
        for game in done {
            self.settle(&game);
        }
    }

    fn settle(&mut self, game: &Match) {
        let Some(outcome) = game.outcome else {
            return;
        };
        if let Outcome::Win(winner, _) = outcome {
            let (loser, taker) = (game.sides[1 - winner].payer, game.sides[winner].payer);
            let paid = self.players.get_mut(&loser).map_or(0, |state| {
                let paid = state.credits.min(game.wager);
                state.credits -= paid;
                paid
            });
            if let Some(state) = self.players.get_mut(&taker) {
                state.credits += paid;
            }
        }
        let everyone = || game.sides.iter().flat_map(|side| &side.fighters);
        for fighter in everyone() {
            self.give_back(fighter);
        }
        for (index, side) in game.sides.iter().enumerate() {
            let other = &game.sides[1 - index].name;
            for fighter in &side.fighters {
                let mut shown = result(outcome, index, other, game.wager);
                shown.push(Message::game(BACK));
                shown.extend(self.scene(self.here(fighter.player), fighter.player));
                self.outbox.extend(address(fighter.player, shown));
                self.touch(fighter.player);
            }
        }
        for fighter in everyone() {
            self.appear(fighter.player, self.here(fighter.player));
        }
        self.close_instance(game.copy);
    }

    fn give_back(&mut self, fighter: &Fighter) {
        let Some(state) = self.players.get_mut(&fighter.player) else {
            return;
        };
        state.run = None;
        state.chain.clone_from(&fighter.chain);
        state.rig.integrity = fighter.integrity;
        state.rig.cycles = fighter.cycles;
        state.rig.files.clone_from(&fighter.files);
    }
}

fn result(outcome: Outcome, side: usize, other: &str, wager: u32) -> Vec<Message> {
    let player = |text: &str| Message::game(text).span(Style::Player, other);
    match outcome {
        Outcome::Draw => vec![player("Time is up. The duel against ").text(" is a draw.")],
        Outcome::Win(winner, end) if winner == side => {
            let how = match end {
                End::Crash => player("The rig of ").text(" crashes."),
                End::GiveUp => named(other, " gives up."),
                End::Gone => named(other, " disconnects."),
            };
            let won = how.text(" You win the duel");
            vec![if wager > 0 {
                won.text(format!(" and take {wager} cr from "))
                    .span(Style::Player, other)
                    .text(".")
            } else {
                won.text(".")
            }]
        }
        Outcome::Win(_, end) => {
            let lost = match end {
                End::Crash => player("Your rig crashes. You lose the duel against "),
                End::GiveUp => player("You give up the duel against "),
                End::Gone => return Vec::new(),
            };
            vec![if wager > 0 {
                lost.text(format!(" and pay {wager} cr to "))
                    .span(Style::Player, other)
                    .text(".")
            } else {
                lost.text(".")
            }]
        }
    }
}
