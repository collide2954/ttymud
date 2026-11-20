use std::cmp::Ordering;

use ttymud_text::{Message, Style};

use super::ice::deploy;
use super::instance::{Instance, InstanceId};
use super::run::Run;
use super::{HostId, NetworkId, PlayerId, World, address};
use crate::content::File;

pub(super) const BACK: &str = "The arena gives back your integrity, cycles, and files.";

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
    pub(super) captures: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Mode {
    Duel,
    Flags,
}

impl Mode {
    pub(super) fn minutes(self) -> u64 {
        match self {
            Self::Duel => 10,
            Self::Flags => 15,
        }
    }

    pub(super) fn word(self) -> &'static str {
        match self {
            Self::Duel => "duel",
            Self::Flags => "match",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum End {
    Crash,
    GiveUp,
    Gone,
    Flags,
    Time,
}

#[derive(Debug, Clone, Copy)]
pub(super) enum Outcome {
    Win(usize, End),
    Draw,
}

#[derive(Debug)]
pub(super) struct Match {
    pub(super) copy: InstanceId,
    pub(super) network: NetworkId,
    pub(super) mode: Mode,
    pub(super) sides: [Side; 2],
    pub(super) wager: u32,
    start: u64,
    pub(super) outcome: Option<Outcome>,
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

    pub(super) fn open_match(
        &mut self,
        network: NetworkId,
        mode: Mode,
        sides: [Side; 2],
        wager: u32,
    ) {
        let bases = self.networks[network.0].bases.clone();
        let files = match mode {
            Mode::Duel => Vec::new(),
            Mode::Flags => bases
                .iter()
                .zip(&sides)
                .map(|(&base, side)| (base, flag(&side.name)))
                .collect(),
        };
        let instance = Instance {
            network,
            ice: deploy(&self.hosts, network, 0),
            analysts: Vec::new(),
            logs: Vec::new(),
            bridges: Vec::new(),
            files,
            deleted: Vec::new(),
            clock: 0,
            cuts: Vec::new(),
            cutting: Vec::new(),
        };
        let copy = self.open_instance(instance);
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
            network,
            mode,
            sides,
            wager,
            start: self.now,
            outcome: None,
        });
    }

    pub(super) fn match_of(&self, player: PlayerId) -> Option<(usize, usize)> {
        self.matches.iter().enumerate().find_map(|(index, game)| {
            let side = game
                .sides
                .iter()
                .position(|side| side.fighters.iter().any(|fighter| fighter.player == player))?;
            Some((index, side))
        })
    }

    pub(super) fn match_mode(&self, player: PlayerId) -> Option<Mode> {
        let (index, _) = self.match_of(player)?;
        Some(self.matches[index].mode)
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

    pub(super) fn fighters_of(&self, index: usize) -> Vec<PlayerId> {
        let sides = self.matches[index].sides.iter();
        sides
            .flat_map(|side| side.fighters.iter().map(|fighter| fighter.player))
            .collect()
    }

    fn decide(&mut self, index: usize, outcome: Outcome) {
        let game = &mut self.matches[index];
        if game.outcome.is_none() {
            game.outcome = Some(outcome);
        }
    }

    pub(super) fn win(&mut self, index: usize, side: usize, end: End) {
        self.decide(index, Outcome::Win(side, end));
    }

    pub(super) fn match_crash(&mut self, player: PlayerId) -> Option<Vec<Message>> {
        let (index, side) = self.match_of(player)?;
        match self.matches[index].mode {
            Mode::Duel => {
                self.win(index, 1 - side, End::Crash);
                Some(Vec::new())
            }
            Mode::Flags => Some(self.respawn(index, side, player)),
        }
    }

    pub(super) fn quit_match(&mut self, player: PlayerId, end: End) -> Option<Vec<Message>> {
        let (index, side) = self.match_of(player)?;
        let game = &self.matches[index];
        if game.mode == Mode::Duel || game.sides[side].payer == player {
            self.win(index, 1 - side, end);
            return Some(Vec::new());
        }
        Some(self.leave_flags(index, side, player))
    }

    pub(super) fn time_matches(&mut self) {
        let now = self.now;
        for index in 0..self.matches.len() {
            let game = &self.matches[index];
            if now.saturating_sub(game.start) < game.mode.minutes() * 60 {
                continue;
            }
            let [first, second] = [0, 1].map(|side| game.sides[side].captures);
            let outcome = match first.cmp(&second) {
                Ordering::Equal => Outcome::Draw,
                Ordering::Greater => Outcome::Win(0, End::Time),
                Ordering::Less => Outcome::Win(1, End::Time),
            };
            self.decide(index, outcome);
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
            for fighter in &side.fighters {
                let mut shown = match game.mode {
                    Mode::Duel => super::duels::result(game, outcome, index),
                    Mode::Flags => super::flags::result(game, outcome, index, fighter.player),
                };
                if !shown.is_empty() {
                    shown.push(Message::game(BACK));
                }
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

    pub(super) fn give_back(&mut self, fighter: &Fighter) {
        let Some(state) = self.players.get_mut(&fighter.player) else {
            return;
        };
        state.run = None;
        state.chain.clone_from(&fighter.chain);
        state.rig.integrity = fighter.integrity;
        state.rig.cycles = fighter.cycles;
        state.rig.files.clone_from(&fighter.files);
    }

    pub(super) fn stay_in_match(&self, player: PlayerId) -> Option<Message> {
        let (what, then) = match self.match_mode(player)? {
            Mode::Duel => ("duel", " to give up."),
            Mode::Flags => ("match", " to leave it."),
        };
        Some(
            Message::error(format!("You are in a {what}. Type "))
                .span(Style::Command, "jackout")
                .text(then),
        )
    }
}

pub(super) fn flag(crew: &str) -> File {
    let name = format!("{}.flag", crew.to_ascii_lowercase());
    File::new(name, vec![format!("The flag of the crew {crew}.")])
}
