use ttymud_text::{Message, Style};

use super::alert::BREACH_ALERT;
use super::jobs::{DAY, Fixer};
use super::{HostId, NetworkId, PlayerId, Step, World, lines};
use crate::content::ContractDef;
use crate::contract::Event;

const HINT_TICKS: u32 = 15;
const FEE_HEAT: u32 = 4;
const FEE_PERCENT: u32 = 25;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Task {
    Steal,
    Wipe,
    Plant(HostId),
}

#[derive(Debug)]
pub(super) struct Contract {
    pub(super) id: String,
    pub(super) fixer: usize,
    pub(super) network: NetworkId,
    pub(super) goal: String,
    pub(super) task: Task,
    pub(super) offer: Vec<String>,
    pub(super) steps: Vec<(Event, String)>,
    pub(super) pay: u32,
    pub(super) posting: Option<(u64, usize, u64)>,
}

impl Contract {
    pub(super) fn hand_made(
        def: ContractDef,
        fixers: &[Fixer],
        network: impl Fn(&str) -> Option<NetworkId>,
    ) -> Self {
        Self {
            id: def.id,
            fixer: fixers
                .iter()
                .position(|fixer| fixer.id == def.fixer)
                .unwrap_or(0),
            network: network(&def.network).unwrap_or(NetworkId(0)),
            goal: def.goal,
            task: Task::Steal,
            offer: def.offer,
            steps: def.steps,
            pay: def.pay,
            posting: None,
        }
    }
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

    pub(super) fn restart_steps(&mut self, player: PlayerId, network: NetworkId) {
        if !self.is_job(player, network) {
            return;
        }
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
        let enter = contract
            .steps
            .iter()
            .position(|&(step, _)| step == Event::Enter);
        if let Some(enter) = enter.filter(|&enter| enter < job.progress) {
            job.progress = enter;
            job.idle = 0;
            job.hinted = false;
        }
    }

    fn next_step(&self, player: PlayerId) -> Option<&str> {
        let state = self.players.get(&player)?;
        let job = state.contract?;
        let steps = self.contracts.get(job.contract)?.steps.iter();
        let (_, hint) = steps.skip(job.progress).find(|(event, _)| {
            event
                .program()
                .is_none_or(|program| state.know.contains(&program))
        })?;
        Some(hint)
    }

    pub(super) fn hint(&mut self, player: PlayerId) -> Option<Message> {
        let job = self.players.get_mut(&player)?.contract.as_mut()?;
        job.idle += 1;
        if job.idle < HINT_TICKS || job.hinted {
            return None;
        }
        job.hinted = true;
        let hint = self.next_step(player)?;
        Some(Message::game(format!("Hint: {hint}")))
    }

    pub(super) fn finish_contract(
        &mut self,
        player: PlayerId,
        network: NetworkId,
        done: bool,
    ) -> Vec<Message> {
        let Some(state) = self.players.get_mut(&player) else {
            return Vec::new();
        };
        let Some(contract) = state
            .contract
            .and_then(|job| self.contracts.get(job.contract))
        else {
            return Vec::new();
        };
        if contract.network != network {
            return Vec::new();
        }
        let files = &mut state.rig.files;
        let carried = files.iter().position(|file| file.name == contract.goal);
        let complete = match contract.task {
            Task::Steal => carried.is_some(),
            Task::Wipe | Task::Plant(_) => done,
        };
        if !complete {
            return Vec::new();
        }
        if let Some(goal) = carried {
            files.remove(goal);
        }
        state.end_contract();
        let fee = if state.heat >= FEE_HEAT {
            contract.pay * FEE_PERCENT / 100
        } else {
            0
        };
        let pay = contract.pay - fee;
        state.credits += pay;
        if let Some((day, fixer, slot)) = contract.posting {
            let today = self.now / DAY;
            state.done.retain(|&(done, ..)| done >= today);
            let id = self
                .fixers
                .get(fixer)
                .map_or_default(|fixer| fixer.id.clone());
            state.done.push((day, id, slot));
        }
        let (heat, fixer, loot, task) = (
            state.heat,
            contract.fixer,
            contract.goal.clone(),
            contract.task,
        );
        let taken = match (carried.is_some(), pay) {
            (true, 0) => format!(" takes {loot}."),
            (true, pay) => format!(" takes {loot} and pays you {pay} cr."),
            (false, 0) => " is happy with your work.".to_owned(),
            (false, pay) => format!(" pays you {pay} cr."),
        };
        let name = self.fixer_name(fixer);
        let said = |rest: String| Message::game("").span(Style::Player, name).text(rest);
        let mut shown = vec![
            said(": Clean work. The contract is complete.".to_owned()),
            said(taken),
        ];
        if fee > 0 {
            shown.push(said(format!(
                " keeps {fee} cr of the pay because your heat is {heat}."
            )));
        }
        shown.extend(self.gain_rep(player, fixer));
        if let Some(corp) = self.networks[network.0].corp {
            self.raise_alert(corp, BREACH_ALERT);
            let name = &self.corps[corp].name;
            self.publish(match task {
                Task::Steal => format!("{name} loses {loot} in a breach."),
                Task::Wipe => format!("{name} loses {loot}, and it has no copy."),
                Task::Plant(_) => format!("Investigators find {loot} on a server of {name}."),
            });
        }
        shown
    }

    pub(super) fn complete_task(&mut self, player: PlayerId, task: Task, file: &str) {
        let Some(state) = self.players.get(&player) else {
            return;
        };
        let (Some(job), Some(run)) = (state.contract, state.run.as_ref()) else {
            return;
        };
        let instance = run.instance;
        let matches = self.contracts.get(job.contract).is_some_and(|contract| {
            contract.task == task && contract.goal == file && contract.network == run.network
        });
        if !matches {
            return;
        }
        for state in self.players.values_mut() {
            let same = state.contract.map(Job::contract) == Some(job.contract);
            let run = state.run.as_mut().filter(|run| run.instance == instance);
            if let (true, Some(run)) = (same, run) {
                run.done = true;
            }
        }
    }

    pub(super) fn offer(&self, player: PlayerId) -> Vec<Message> {
        let Some((contract, _)) = self.contract_of(player) else {
            return Vec::new();
        };
        let mut shown = vec![
            Message::game("")
                .span(Style::Player, self.fixer_name(contract.fixer))
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
        let heist = self.heist_lines(player);
        let Some((contract, _)) = self.contract_of(player) else {
            if args.is_empty() && !heist.is_empty() {
                return Ok(lines(heist));
            }
            return Ok(Step::show(vec![Message::game("You have no contract.")]));
        };
        match args {
            [] => {
                let network = &self.networks[contract.network.0].name;
                let fixer = self.fixer_name(contract.fixer);
                let goal = match contract.task {
                    Task::Steal => {
                        Message::game("Goal: download ").span(Style::File, &contract.goal)
                    }
                    Task::Wipe => Message::game("Goal: delete ")
                        .span(Style::File, &contract.goal)
                        .text(" with rm"),
                    Task::Plant(host) => Message::game("Goal: upload ")
                        .span(Style::File, &contract.goal)
                        .text(" to ")
                        .span(Style::Host, &self.hosts[host.0].id),
                };
                let mut shown = vec![
                    Message::game("Contract from ").span(Style::Player, fixer),
                    Message::game(format!("Target: {network}")),
                    goal.text(", then leave the network with a clean exit."),
                ];
                if contract.pay > 0 {
                    shown.push(Message::game(format!("Pay: {} cr", contract.pay)));
                }
                if let Some(hint) = self.next_step(player) {
                    shown.push(Message::game(format!("Next: {hint}")));
                }
                shown.extend(heist);
                Ok(lines(shown))
            }
            [word] if word.eq_ignore_ascii_case("drop") => {
                let message = Message::game("You drop the contract from ")
                    .span(Style::Player, self.fixer_name(contract.fixer))
                    .text(".");
                if let Some(state) = self.players.get_mut(&player) {
                    state.end_contract();
                }
                Ok(Step::show(vec![message]))
            }
            _ => Err(Message::error("Usage: contract [drop].")),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::testing::{CITY, JOB, hired, hired_world, plain, run, step};
    use crate::{Content, PlayerId, World};

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
                "Pay: 100 cr",
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
            plain(&world.command(player, "back"))[..5],
            [
                "You leave the network of Test Lab. Clean exit.",
                "Pell: Clean work. The contract is complete.",
                "Pell takes data.db and pays you 100 cr.",
                "Your reputation with Pell: 1.",
                "The Back Room",
            ]
        );
        assert_eq!(
            plain(&world.command(player, "contract")),
            ["You have no contract."]
        );
    }

    #[test]
    fn at_heat_4_the_fixer_keeps_a_fee() {
        let (mut world, player) = hired();
        if let Some(state) = world.players.get_mut(&player) {
            state.heat = 4;
        }
        for line in ["go back-room", "go gate", "go core", "get data.db", "back"] {
            world.command(player, line);
        }
        assert_eq!(
            plain(&world.command(player, "back"))[2..4],
            [
                "Pell takes data.db and pays you 75 cr.",
                "Pell keeps 25 cr of the pay because your heat is 4.",
            ]
        );
        assert_eq!(
            plain(&world.command(player, "status | grep credits")),
            ["Credits: 75 cr"]
        );
    }

    #[test]
    fn the_fixer_keeps_the_goal_and_the_pay_goes_to_the_rig() {
        let (mut world, player) = hired();
        for line in [
            "go back-room",
            "go gate",
            "go core",
            "get data.db",
            "back",
            "back",
        ] {
            world.command(player, line);
        }
        assert_eq!(
            plain(&world.command(player, "inventory")),
            ["Your rig has no files."]
        );
        assert_eq!(
            plain(&world.command(player, "status | grep credits")),
            ["Credits: 100 cr"]
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

    fn breaker_with_a_scrub_step() -> (World, PlayerId) {
        let job = JOB.replace(
            "step = scan: Type scan to find the ICE.",
            "step = scrub: Type scrub.",
        );
        let city = CITY.replace("home = back-room", "home = back-room\ncontract = lab-job");
        let mut world = World::new(Content::parse(&format!("{city}{job}")).unwrap(), 0);
        let (player, _) = world.connect("a");
        for line in ["nyx", "breaker", "go back-room", "go gate"] {
            world.command(player, line);
        }
        (world, player)
    }

    #[test]
    fn a_step_for_a_program_that_the_handle_does_not_know_is_skipped() {
        let (mut world, player) = breaker_with_a_scrub_step();
        assert_eq!(
            next(&mut world, player).as_deref(),
            Some("Next: Go to core and type download data.db.")
        );
        assert_eq!(
            plain(&step(&mut world, player, "~15")),
            ["Hint: Go to core and type download data.db."]
        );
    }

    fn breaking_job() -> (World, PlayerId) {
        let job = JOB.replace(
            "step = scan: Type scan to find the ICE.",
            "step = scan: Type scan to find the ICE.\nstep = break: Type probe sentry | crack.",
        );
        let city = CITY.replace("home = back-room", "home = back-room\ncontract = lab-job");
        let mut world = World::new(Content::parse(&format!("{city}{job}")).unwrap(), 0);
        let (player, _) = world.join("ghost");
        for line in ["go back-room", "go gate", "scan"] {
            world.command(player, line);
        }
        if let Some(state) = world.players.get_mut(&player) {
            state.rig.cycles = 20;
        }
        (world, player)
    }

    #[test]
    fn a_break_step_ends_only_when_the_ice_breaks() {
        let (mut world, player) = breaking_job();
        world.command(player, "crack sentry");
        assert_eq!(
            next(&mut world, player).as_deref(),
            Some("Next: Type probe sentry | crack.")
        );
        world.command(player, "probe sentry | crack");
        assert_eq!(
            next(&mut world, player).as_deref(),
            Some("Next: Go to core and type download data.db.")
        );
    }

    #[test]
    fn a_run_that_ends_without_the_goal_starts_the_steps_again() {
        let enter = Some("Next: Type go gate to connect to the lab.");
        let (mut world, player) = hired();
        for line in ["go back-room", "go gate", "scan", "jackout"] {
            world.command(player, line);
        }
        assert_eq!(next(&mut world, player).as_deref(), enter);
        for line in ["go gate", "scan", "back"] {
            world.command(player, line);
        }
        assert_eq!(next(&mut world, player).as_deref(), enter);
    }

    #[test]
    fn without_an_offer_there_is_no_contract() {
        assert_eq!(run("contract"), ["You have no contract."]);
        assert_eq!(run("contract drop"), ["You have no contract."]);
    }
}
