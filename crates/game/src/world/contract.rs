use ttymud_text::{Message, Style};

use super::{NetworkId, PlayerId, Step, World, lines};

#[derive(Debug)]
pub(super) struct Contract {
    pub(super) fixer: String,
    pub(super) network: NetworkId,
    pub(super) goal: String,
    pub(super) offer: Vec<String>,
}

impl World {
    fn contract_of(&self, player: PlayerId) -> Option<&Contract> {
        let index = self.players.get(&player)?.contract?;
        self.contracts.get(index)
    }

    pub(super) fn offer(&self, player: PlayerId) -> Vec<Message> {
        let Some(contract) = self.contract_of(player) else {
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
        let Some(contract) = self.contract_of(player) else {
            return Ok(Step::show(vec![Message::game("You have no contract.")]));
        };
        match args {
            [] => {
                let network = &self.networks[contract.network.0].name;
                Ok(lines(vec![
                    Message::game("Contract from ").span(Style::Player, &contract.fixer),
                    Message::game(format!("Target: {network}")),
                    Message::game("Goal: download ")
                        .span(Style::File, &contract.goal)
                        .text(", then leave the network with a clean exit."),
                ]))
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
    use crate::testing::{hired, hired_world, plain, run};

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
            ]
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
    fn without_an_offer_there_is_no_contract() {
        assert_eq!(run("contract"), ["You have no contract."]);
        assert_eq!(run("contract drop"), ["You have no contract."]);
    }
}
