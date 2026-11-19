use ttymud_text::{Message, Style};

use super::{NetworkId, PlayerId, Step, World, lines};
use crate::contract::Event;

const HINT_TICKS: u32 = 15;

#[derive(Debug)]
pub(super) struct Contract {
    pub(super) id: String,
    pub(super) fixer: String,
    pub(super) network: NetworkId,
    pub(super) goal: String,
    pub(super) offer: Vec<String>,
    pub(super) steps: Vec<(Event, String)>,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct Job {
    contract: usize,
    progress: usize,
    idle: u32,
    hinted: bool,
}

impl Job {
    pub(super) fn new(contract: usize) -> Self {
        Self::resume(contract, 0)
    }

    pub(super) fn resume(contract: usize, progress: usize) -> Self {
        Self {
            contract,
            progress,
            idle: 0,
            hinted: false,
        }
    }

    pub(super) fn contract(self) -> usize {
        self.contract
    }

    pub(super) fn progress(self) -> usize {
        self.progress
    }
}

impl World {
    fn contract_of(&self, player: PlayerId) -> Option<(&Contract, Job)> {
        let job = self.players.get(&player)?.contract?;
        Some((self.contracts.get(job.contract)?, job))
    }

    pub(super) fn is_job(&self, player: PlayerId, network: NetworkId) -> bool {
        self.contract_of(player)
            .is_some_and(|(contract, _)| contract.network == network)
    }

    pub(super) fn is_goal(&self, player: PlayerId, file: &str) -> bool {
        self.contract_of(player)
            .is_some_and(|(contract, _)| contract.goal == file)
    }

    pub(super) fn advance(&mut self, player: PlayerId, event: Event) {
        let Some(job) = self
            .players
            .get_mut(&player)
            .and_then(|state| state.contract.as_mut())
        else {
            return;
        };
        let Some(contract) = self.contracts.get(job.contract) else {
            return;
        };
        let mut steps = contract.steps.iter().skip(job.progress);
        if let Some(offset) = steps.position(|&(step, _)| step == event) {
            job.progress += offset + 1;
            job.idle = 0;
            job.hinted = false;
        }
    }

    pub(super) fn hint(&mut self, player: PlayerId) -> Option<Message> {
        let job = self.players.get_mut(&player)?.contract.as_mut()?;
        let (_, hint) = self.contracts.get(job.contract)?.steps.get(job.progress)?;
        job.idle += 1;
        if job.idle < HINT_TICKS || job.hinted {
            return None;
        }
        job.hinted = true;
        Some(Message::game(format!("Hint: {hint}")))
    }

    pub(super) fn finish_contract(
        &mut self,
        player: PlayerId,
        network: NetworkId,
    ) -> Option<Message> {
        let state = self.players.get(&player)?;
        let contract = self.contracts.get(state.contract?.contract)?;
        let has_goal = state
            .rig
            .files
            .iter()
            .any(|file| file.name == contract.goal);
        if contract.network != network || !has_goal {
            return None;
        }
        let message = Message::game("")
            .span(Style::Player, &contract.fixer)
            .text(": Clean work. The contract is complete.");
        self.players.get_mut(&player)?.contract = None;
        Some(message)
    }

    pub(super) fn offer(&self, player: PlayerId) -> Vec<Message> {
        let Some((contract, _)) = self.contract_of(player) else {
            return Vec::new();
        };
        let mut shown = vec![
            Message::game("")
                .span(Style::Player, &contract.fixer)
                .text(" sends you a job:"),
        ];
        shown.extend(contract.offer.iter().map(Message::game));
        shown.push(
            Message::game("Type ")
                .span(Style::Command, "contract")
                .text(" to see the job again."),
        );
        shown
    }

    pub(super) fn contract(&mut self, player: PlayerId, args: &[String]) -> Result<Step, Message> {
        let Some((contract, job)) = self.contract_of(player) else {
            return Ok(Step::show(vec![Message::game("You have no contract.")]));
        };
        match args {
            [] => {
                let network = &self.networks[contract.network.0].name;
                let mut shown = vec![
                    Message::game("Contract from ").span(Style::Player, &contract.fixer),
                    Message::game(format!("Target: {network}")),
                    Message::game("Goal: download ")
                        .span(Style::File, &contract.goal)
                        .text(", then leave the network with a clean exit."),
                ];
                if let Some((_, hint)) = contract.steps.get(job.progress) {
                    shown.push(Message::game(format!("Next: {hint}")));
                }
                Ok(lines(shown))
            }
            [word] if word.eq_ignore_ascii_case("drop") => {
                let message = Message::game("You drop the contract from ")
                    .span(Style::Player, &contract.fixer)
                    .text(".");
                if let Some(state) = self.players.get_mut(&player) {
                    state.contract = None;
                }
                Ok(Step::show(vec![message]))
            }
            _ => Err(Message::error("Usage: contract [drop].")),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::testing::{hired, hired_world, plain, run, step};
    use crate::{PlayerId, World};

    #[test]
    fn a_new_player_gets_the_offer_of_the_fixer() {
        let (_, outputs) = hired_world().join("ghost");
        assert_eq!(
            plain(&outputs)[4..],
            [
                "Pell sends you a job:",
                "The lab has data that a client wants.",
                "Get data.db and leave clean.",
                "Type contract to see the job again.",
            ]
        );
    }

    #[test]
    fn contract_shows_the_job() {
        let (mut world, player) = hired();
        assert_eq!(
            plain(&world.command(player, "contract")),
            [
                "Contract from Pell",
                "Target: Test Lab",
                "Goal: download data.db, then leave the network with a clean exit.",
                "Next: Go to back-room. Your rig is there.",
            ]
        );
    }

    fn next(world: &mut World, player: PlayerId) -> Option<String> {
        plain(&world.command(player, "contract")).pop()
    }

    #[test]
    fn each_event_finishes_its_step() {
        let (mut world, player) = hired();
        world.command(player, "go back-room");
        assert_eq!(
            next(&mut world, player).as_deref(),
            Some("Next: Type go gate to connect to the lab.")
        );
    }

    #[test]
    fn a_later_event_also_finishes_the_earlier_steps() {
        let (mut world, player) = hired();
        for line in ["go back-room", "go gate", "go core", "get data.db"] {
            world.command(player, line);
        }
        assert_eq!(
            next(&mut world, player).as_deref(),
            Some("Next: Type back until you leave the lab.")
        );
    }

    #[test]
    fn a_hint_comes_after_some_quiet_ticks() {
        let (mut world, player) = hired();
        assert_eq!(
            plain(&step(&mut world, player, "~14")),
            Vec::<String>::new()
        );
        assert_eq!(
            plain(&step(&mut world, player, "~")),
            ["Hint: Go to back-room. Your rig is there."]
        );
        assert_eq!(
            plain(&step(&mut world, player, "~30")),
            Vec::<String>::new()
        );
    }

    #[test]
    fn progress_starts_the_hint_clock_again() {
        let (mut world, player) = hired();
        step(&mut world, player, "~10");
        world.command(player, "go back-room");
        assert_eq!(
            plain(&step(&mut world, player, "~10")),
            Vec::<String>::new()
        );
        assert_eq!(
            plain(&step(&mut world, player, "~5")),
            ["Hint: Type go gate to connect to the lab."]
        );
    }

    #[test]
    fn contract_drop_ends_the_job() {
        let (mut world, player) = hired();
        assert_eq!(
            plain(&world.command(player, "contract drop")),
            ["You drop the contract from Pell."]
        );
        assert_eq!(
            plain(&world.command(player, "contract")),
            ["You have no contract."]
        );
    }

    #[test]
    fn a_clean_exit_with_the_goal_completes_the_contract() {
        let (mut world, player) = hired();
        for line in ["go back-room", "go gate", "go core", "get data.db", "back"] {
            world.command(player, line);
        }
        assert_eq!(
            plain(&world.command(player, "back"))[..3],
            [
                "You leave the network of Test Lab. Clean exit.",
                "Pell: Clean work. The contract is complete.",
                "The Back Room",
            ]
        );
        assert_eq!(
            plain(&world.command(player, "contract")),
            ["You have no contract."]
        );
    }

    #[test]
    fn a_clean_exit_without_the_goal_keeps_the_contract() {
        let (mut world, player) = hired();
        for line in ["go back-room", "go gate"] {
            world.command(player, line);
        }
        assert_eq!(plain(&world.command(player, "back"))[1], "The Back Room");
        assert_eq!(
            plain(&world.command(player, "contract"))[0],
            "Contract from Pell"
        );
    }

    #[test]
    fn without_an_offer_there_is_no_contract() {
        assert_eq!(run("contract"), ["You have no contract."]);
        assert_eq!(run("contract drop"), ["You have no contract."]);
    }
}
