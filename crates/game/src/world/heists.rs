use ttymud_text::{Kind, Message, Style};

use super::alert::{Alert, BREACH_ALERT};
use super::contract::Contract;
use super::crews::MAX_CREW;
use super::jobs::{DAY, Offer, base_pay, needed};
use super::presence::named;
use super::{HostId, NetworkId, PlayerId, Step, World};
use crate::random::{Random, mix};
use crate::target::{RoleBridge, generate_heist};

pub(super) const HEIST_SLOT: u64 = 3;
const HEIST_PAY: u32 = 3;
const MIN_CREW: usize = 2;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Heist {
    pub(super) fixer: String,
    pub(super) day: u64,
    pub(super) contract: Option<usize>,
    pub(super) entered: Vec<String>,
}

fn both(names: &[String]) -> String {
    match names {
        [rest @ .., last] if !rest.is_empty() => format!("{} and {last}", rest.join(", ")),
        _ => names.join(""),
    }
}

impl World {
    pub(super) fn posted_heist(&self, fixer: usize, day: u64) -> Option<Offer> {
        let def = &self.fixers[fixer];
        let mut random = Random::new(mix(&[self.seed, day, fixer as u64, HEIST_SLOT]));
        let corp = *def.corps.get(random.below(def.corps.len()))?;
        let budget = self.corps[corp].budget;
        let percent = 80 + u32::try_from(random.below(41)).unwrap_or(0);
        let pay = base_pay(budget) * HEIST_PAY * percent / 100 / 10 * 10;
        let target = generate_heist(&self.corps[corp], &self.templates, random.draw())?;
        Some(Offer {
            fixer,
            day,
            slot: HEIST_SLOT,
            corp,
            target,
            pay,
        })
    }

    pub(super) fn heists_at(&self, host: HostId, day: u64) -> Vec<Offer> {
        self.fixers_at(host)
            .into_iter()
            .filter_map(|fixer| self.posted_heist(fixer, day))
            .collect()
    }

    pub(super) fn heist_board(
        &self,
        player: PlayerId,
        number: &mut usize,
        offers: Vec<Offer>,
    ) -> Vec<Message> {
        if offers.is_empty() {
            return Vec::new();
        }
        let crew = self.crews.of(self.handle(player).unwrap_or_default());
        let mut shown = vec![Message::game("Heists for crews today:")];
        for offer in offers {
            *number += 1;
            let corp = &self.corps[offer.corp];
            let alert = match self.alert(offer.corp) {
                Alert::Green => String::new(),
                hot => format!(" ({} alert)", hot.word()),
            };
            let line = Message::game(format!("{number}. {}{alert}, from ", corp.name))
                .span(Style::Player, self.fixer_name(offer.fixer))
                .text(": get ")
                .span(Style::File, &offer.target.goal)
                .text(format!(
                    " from the vault. Pay: {} cr for the crew.",
                    offer.pay
                ));
            let need = needed(corp.budget);
            let id = &self.fixers[offer.fixer].id;
            let note = match crew {
                Some(crew) if crew.done.contains(&(offer.day, id.clone())) => " Done.".to_owned(),
                Some(crew) if crew.members.len() >= MIN_CREW && crew.rep < need => {
                    format!(" Needs {need} crew reputation.")
                }
                Some(crew) if crew.members.len() >= MIN_CREW => String::new(),
                _ => format!(" Needs a crew of {MIN_CREW} to {MAX_CREW}."),
            };
            shown.push(line.text(note));
        }
        shown
    }

    pub(super) fn accept_heist(&mut self, player: PlayerId, offer: Offer) -> Result<Step, Message> {
        let name = self.handle(player).unwrap_or_default().to_owned();
        if self.crews.of(&name).is_none() {
            return Err(Message::error("A heist needs a crew. Type ")
                .span(Style::Command, "crew create <name>")
                .text(" to make one."));
        }
        let key = self.led_crew(&name)?;
        let crew = &self.crews.list[&key];
        if crew.members.len() < MIN_CREW {
            return Err(Message::error(format!(
                "A heist needs a crew of {MIN_CREW} to {MAX_CREW} handles. {} has {}.",
                crew.name,
                crew.members.len()
            )));
        }
        if crew.heist.is_some() {
            return Err(
                Message::error(format!("{} already has a heist. Type ", crew.name))
                    .span(Style::Command, "crew drop")
                    .text(" to end it."),
            );
        }
        let fixer = self.fixer_name(offer.fixer).to_owned();
        let id = self.fixers[offer.fixer].id.clone();
        if crew.done.contains(&(offer.day, id.clone())) {
            return Err(Message::error("Your crew did this heist today. ")
                .span(Style::Player, &fixer)
                .text(" posts a new heist each day."));
        }
        if let Some(refusal) = self.refusal(player, offer.fixer) {
            return Err(refusal);
        }
        let need = needed(self.corps[offer.corp].budget);
        if crew.rep < need {
            let (crew, rep) = (&crew.name, crew.rep);
            return Err(Message::new(Kind::Error).span(Style::Player, &fixer).text(format!(
                " gives this heist only to crews with {need} reputation. The reputation of {crew}: {rep}."
            )));
        }
        let crew = crew.name.clone();
        let target = self.corps[offer.corp].name.clone();
        let (goal, pay, day) = (offer.target.goal.clone(), offer.pay, offer.day);
        let contract = self.post(offer);
        if let Some(found) = self.crews.list.get_mut(&key) {
            found.heist = Some(Heist {
                fixer: id,
                day,
                contract: Some(contract),
                entered: Vec::new(),
            });
        }
        self.crews.dirty = true;
        let told = named(
            &name,
            &format!(" takes a heist from {fixer} for the crew {crew}. Type "),
        )
        .span(Style::Command, "contract")
        .text(" to see it.");
        self.tell_crew(&key, player, &told);
        Ok(Step::show(vec![
            Message::game("")
                .span(Style::Player, &fixer)
                .text(format!(": The target is {target}. Get "))
                .span(Style::File, &goal)
                .text(format!(
                    " from the vault and leave clean. The crew splits {pay} cr."
                )),
            Message::game("Each safehouse of the crew has a way in. Type ")
                .span(Style::Command, "contract")
                .text(" to see the stages."),
        ]))
    }

    pub(super) fn heist_contract(&self, player: PlayerId) -> Option<&Contract> {
        let crew = self.crews.of(self.handle(player)?)?;
        self.contracts.get(crew.heist.as_ref()?.contract?)
    }

    pub(super) fn note_entry(&mut self, player: PlayerId, network: NetworkId) {
        let is_heist = self
            .heist_contract(player)
            .is_some_and(|contract| contract.network == network);
        let name = self.handle(player).unwrap_or_default().to_owned();
        let Some(key) = self.crews.key_of(&name).filter(|_| is_heist) else {
            return;
        };
        let heist = self
            .crews
            .list
            .get_mut(&key)
            .and_then(|crew| crew.heist.as_mut());
        if let Some(heist) = heist
            && !heist
                .entered
                .iter()
                .any(|known| known.eq_ignore_ascii_case(&name))
        {
            heist.entered.push(name);
            self.crews.dirty = true;
        }
    }

    pub(super) fn finish_heist(&mut self, player: PlayerId, network: NetworkId) -> Vec<Message> {
        let Some(contract) = self
            .heist_contract(player)
            .filter(|contract| contract.network == network)
        else {
            return Vec::new();
        };
        let (goal, pay, fixer) = (contract.goal.clone(), contract.pay, contract.fixer);
        let Some(state) = self.players.get_mut(&player) else {
            return Vec::new();
        };
        let Some(index) = state.rig.files.iter().position(|file| file.name == goal) else {
            return Vec::new();
        };
        state.rig.files.remove(index);
        let name = state.name.clone();
        let Some(key) = self.crews.key_of(&name) else {
            return Vec::new();
        };
        let today = self.now / DAY;
        let Some(crew) = self.crews.list.get_mut(&key) else {
            return Vec::new();
        };
        let Some(heist) = crew.heist.take() else {
            return Vec::new();
        };
        crew.rep += 1;
        crew.done.retain(|&(day, _)| day >= today);
        crew.done.push((heist.day, heist.fixer));
        let (crew_name, rep) = (crew.name.clone(), crew.rep);
        self.crews.dirty = true;
        let mut entered = heist.entered;
        if !entered.contains(&name) {
            entered.push(name);
        }
        let share = pay / u32::try_from(entered.len()).unwrap_or(1).max(1);
        for member in &entered {
            self.pay_handle(member, share);
        }
        if let Some(corp) = self.networks[network.0].corp {
            self.raise_alert(corp, BREACH_ALERT * 2);
            let target = &self.corps[corp].name;
            self.publish(format!("{target} loses {goal} from its vault in a heist."));
        }
        let said = |rest: String| {
            Message::game("")
                .span(Style::Player, self.fixer_name(fixer))
                .text(rest)
        };
        let shown = vec![
            said(": Clean work. The heist is complete.".to_owned()),
            said(format!(
                " pays the crew {pay} cr: {share} cr to each of {}.",
                both(&entered)
            )),
            Message::game(format!("The reputation of {crew_name}: {rep}.")),
        ];
        for message in &shown {
            self.tell_crew(&key, player, message);
        }
        shown
    }

    fn pay_handle(&mut self, name: &str, amount: u32) {
        let online = self
            .players
            .iter()
            .find(|(_, state)| state.name.eq_ignore_ascii_case(name))
            .map(|(&player, _)| player);
        if let Some(player) = online {
            if let Some(state) = self.players.get_mut(&player) {
                state.credits += amount;
            }
            self.touch(player);
            return;
        }
        let saved = self.accounts.iter_mut().find(|(_, account)| {
            account
                .handle
                .as_ref()
                .is_some_and(|handle| handle.name.eq_ignore_ascii_case(name))
        });
        if let Some((key, account)) = saved
            && let Some(handle) = account.handle.as_mut()
        {
            handle.credits += amount;
            self.dirty.insert(key.clone());
        }
    }

    pub(super) fn heist_lines(&self, player: PlayerId) -> Vec<Message> {
        self.heist_text(player).unwrap_or_default()
    }

    fn heist_text(&self, player: PlayerId) -> Option<Vec<Message>> {
        let crew = self.crews.of(self.handle(player)?)?;
        let heist = crew.heist.as_ref()?;
        let contract = self.contracts.get(heist.contract?)?;
        let network = &self.networks[contract.network.0];
        let vault = network.vault.as_ref()?;
        let host = |index: usize| self.hosts[index].id.as_str();
        let role = match &vault.bridge {
            RoleBridge::Employee { host: desk, name } => {
                format!(" A Face can also type pretext {name} on {}.", host(*desk))
            }
            RoleBridge::Modem { host: line } => {
                format!(" A Phreak can also type proxy on {}.", host(*line))
            }
        };
        let ways: Vec<&str> = network.entries.iter().map(|entry| host(entry.0)).collect();
        let mut shown = vec![
            Message::game("Heist from ")
                .span(Style::Player, self.fixer_name(contract.fixer))
                .text(format!(
                    " for the crew {}: {}. Get ",
                    crew.name, network.name
                ))
                .span(Style::File, &contract.goal)
                .text(" from the vault."),
            Message::game(format!(
                "Pay: {} cr, split among the crewmates who enter the target.",
                contract.pay
            )),
            Message::game(format!(
                "1. Get in from your safehouse: {}.",
                ways.join(" or ")
            )),
            Message::game(format!(
                "2. Bridge the vault: wait on {} for its sync.{role}",
                host(vault.laptop)
            )),
            Message::game("3. Download ")
                .span(Style::File, &contract.goal)
                .text(" in the vault."),
            Message::game("4. Leave the network with a clean exit."),
        ];
        if !heist.entered.is_empty() {
            shown.push(Message::game(format!(
                "In the target so far: {}.",
                heist.entered.join(", ")
            )));
        }
        Some(shown)
    }

    pub(super) fn crew_drop(
        &mut self,
        player: PlayerId,
        name: &str,
    ) -> Result<Vec<Message>, Message> {
        let key = self.led_crew(name)?;
        let crew = &self.crews.list[&key];
        let Some(heist) = &crew.heist else {
            return Err(Message::error(format!("{} has no heist.", crew.name)));
        };
        let fixer = heist
            .contract
            .and_then(|index| self.contracts.get(index))
            .map_or("the fixer", |contract| self.fixer_name(contract.fixer))
            .to_owned();
        if let Some(crew) = self.crews.list.get_mut(&key) {
            crew.heist = None;
        }
        self.crews.dirty = true;
        let told = named(name, " drops the heist from ")
            .span(Style::Player, &fixer)
            .text(".");
        self.tell_crew(&key, player, &told);
        Ok(vec![
            Message::game("You drop the heist from ")
                .span(Style::Player, fixer)
                .text("."),
        ])
    }

    pub(super) fn resume_heists(&mut self) {
        let keys: Vec<String> = self.crews.list.keys().cloned().collect();
        for key in keys {
            let Some(heist) = self
                .crews
                .list
                .get(&key)
                .and_then(|crew| crew.heist.clone())
            else {
                continue;
            };
            let fixer = self.fixers.iter().position(|known| known.id == heist.fixer);
            let offer = fixer.and_then(|fixer| self.posted_heist(fixer, heist.day));
            let contract = offer.map(|offer| self.post(offer));
            let Some(crew) = self.crews.list.get_mut(&key) else {
                continue;
            };
            match (contract, crew.heist.as_mut()) {
                (Some(index), Some(heist)) => heist.contract = Some(index),
                _ => crew.heist = None,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::content::File;
    use crate::testing::{heard, plain, world};
    use crate::{PlayerId, World};

    const NOON: u64 = 20_000 * 86_400 + 12 * 3_600;

    fn crewed(names: &[&str]) -> (World, Vec<PlayerId>) {
        let mut world = world();
        world.set_time(NOON);
        let players: Vec<PlayerId> = names.iter().map(|name| world.join(name).0).collect();
        world.command(players[0], "crew create Moths");
        for (index, name) in names.iter().enumerate().skip(1) {
            world.command(players[0], &format!("crew invite {name}"));
            world.command(players[index], "crew join moths");
        }
        if let Some(crew) = world.crews.list.get_mut("moths") {
            crew.rep = 2;
        }
        for &player in &players {
            world.command(player, "go bazaar");
        }
        (world, players)
    }

    fn entry(world: &World) -> String {
        let network = world.networks.last();
        let entry = network.and_then(|network| network.entries.first());
        entry.map_or_default(|entry| world.hosts[entry.0].id.clone())
    }

    fn credits(world: &World, player: PlayerId) -> u32 {
        world.players.get(&player).map_or(0, |state| state.credits)
    }

    #[test]
    fn each_fixer_posts_one_heist_a_day_for_crews() {
        let (mut world, players) = crewed(&["nyx"]);
        let board = plain(&world.command(players[0], "jobs"));
        let heading = board
            .iter()
            .position(|line| line == "Heists for crews today:")
            .unwrap_or_default();
        let line = &board[heading + 1];
        assert!(
            line.starts_with("4. ")
                && line.contains(", from Pell: get ")
                && line.contains(" from the vault. Pay: ")
                && line.ends_with(" cr for the crew. Needs a crew of 2 to 4."),
            "{board:?}"
        );
    }

    #[test]
    fn only_the_leader_of_a_crew_of_two_or_more_takes_a_heist() {
        let (mut world, players) = crewed(&["nyx"]);
        assert_eq!(
            plain(&world.command(players[0], "accept 4")),
            ["A heist needs a crew of 2 to 4 handles. Moths has 1."]
        );
        let (mut world, players) = crewed(&["nyx", "zer0"]);
        let (nyx, zer0) = (players[0], players[1]);
        let (solo, _) = world.join("ghost");
        world.command(solo, "go bazaar");
        assert_eq!(
            plain(&world.command(solo, "accept 4")),
            ["A heist needs a crew. Type crew create <name> to make one."]
        );
        assert_eq!(
            plain(&world.command(zer0, "accept 4")),
            ["Only the leader of Moths can do that. The leader is nyx."]
        );
        let outputs = world.command(nyx, "accept 4");
        let shown = heard(&outputs, nyx);
        assert!(
            shown[0].starts_with("Pell: The target is ") && shown[0].contains(" from the vault"),
            "{shown:?}"
        );
        assert_eq!(
            heard(&outputs, zer0),
            ["nyx takes a heist from Pell for the crew Moths. Type contract to see it."]
        );
        assert_eq!(
            plain(&world.command(nyx, "accept 4")),
            ["Moths already has a heist. Type crew drop to end it."]
        );
        let contract = plain(&world.command(zer0, "contract"));
        assert!(
            contract[0].starts_with("Heist from Pell for the crew Moths: Acme Corp")
                || contract[0].starts_with("Heist from Pell for the crew Moths: Globex"),
            "{contract:?}"
        );
        assert!(
            contract
                .iter()
                .any(|line| line.starts_with("2. Bridge the vault: wait on maint for its sync.")),
            "{contract:?}"
        );
    }

    #[test]
    fn each_safehouse_gets_a_way_in_and_the_heist_counts_who_enters() {
        let (mut world, players) = crewed(&["nyx", "zer0", "kid"]);
        world.command(players[0], "accept 4");
        let entry = entry(&world);
        for &player in &players[..2] {
            for line in ["go relay", "go back-room", &format!("go {entry}")] {
                world.command(player, line);
            }
        }
        let heist = world.crews.of("nyx").and_then(|crew| crew.heist.clone());
        assert_eq!(
            heist.map(|heist| heist.entered),
            Some(vec!["nyx".to_owned(), "zer0".to_owned()])
        );
        assert_eq!(world.instance_id(players[0]), world.instance_id(players[1]));
    }

    #[test]
    fn a_clean_exit_with_the_vault_file_pays_the_crewmates_who_entered() {
        let (mut world, players) = crewed(&["nyx", "zer0", "kid"]);
        let (nyx, zer0, kid) = (players[0], players[1], players[2]);
        world.command(nyx, "accept 4");
        let entry = entry(&world);
        for &player in &[nyx, zer0] {
            for line in ["go relay", "go back-room", &format!("go {entry}")] {
                world.command(player, line);
            }
        }
        let contract = world
            .crews
            .of("nyx")
            .and_then(|crew| crew.heist.as_ref())
            .and_then(|heist| heist.contract)
            .and_then(|index| world.contracts.get(index));
        let (goal, pay) = contract.map_or_default(|contract| (contract.goal.clone(), contract.pay));
        if let Some(state) = world.players.get_mut(&nyx) {
            state
                .rig
                .files
                .push(File::new(&goal, vec!["Secrets.".to_owned()]));
        }
        let outputs = world.command(nyx, "back");
        let share = pay / 2;
        let paid = format!("Pell pays the crew {pay} cr: {share} cr to each of nyx and zer0.");
        let shown = heard(&outputs, nyx);
        assert_eq!(
            shown[1..4],
            [
                "Pell: Clean work. The heist is complete.".to_owned(),
                paid.clone(),
                "The reputation of Moths: 3.".to_owned(),
            ]
        );
        assert!(heard(&outputs, zer0).contains(&paid), "{outputs:?}");
        assert_eq!(
            (
                credits(&world, nyx),
                credits(&world, zer0),
                credits(&world, kid)
            ),
            (share, share, 0)
        );
        let crew = world.crews.of("nyx").cloned();
        assert_eq!(
            crew.as_ref().map(|crew| (crew.rep, crew.heist.is_none())),
            Some((3, true))
        );
        let news = plain(&world.command(kid, "news"));
        assert!(
            news[0].ends_with(&format!(" loses {goal} from its vault in a heist.")),
            "{news:?}"
        );
        for line in ["go relay", "go bazaar"] {
            world.command(nyx, line);
        }
        assert_eq!(
            plain(&world.command(nyx, "accept 4")),
            ["Your crew did this heist today. Pell posts a new heist each day."]
        );
    }

    #[test]
    fn the_leader_can_drop_the_heist() {
        let (mut world, players) = crewed(&["nyx", "zer0"]);
        world.command(players[0], "accept 4");
        let outputs = world.command(players[0], "crew drop");
        assert_eq!(
            heard(&outputs, players[0]),
            ["You drop the heist from Pell."]
        );
        assert_eq!(
            heard(&outputs, players[1]),
            ["nyx drops the heist from Pell."]
        );
        assert_eq!(
            plain(&world.command(players[1], "contract")),
            ["You have no contract."]
        );
        assert_eq!(
            plain(&world.command(players[0], "crew drop")),
            ["Moths has no heist."]
        );
    }

    #[test]
    fn a_heist_comes_back_from_crews_ini() {
        let (mut world, players) = crewed(&["nyx", "zer0"]);
        world.command(players[0], "accept 4");
        let entry = entry(&world);
        for line in ["go relay", "go back-room", &format!("go {entry}")] {
            world.command(players[0], line);
        }
        let saves = world.saves();
        let text = saves
            .iter()
            .find(|(path, _)| path == "crews.ini")
            .map_or_default(|(_, text)| text.clone());
        assert!(
            text.contains("\nheist = pell 20000\nentered = nyx\n"),
            "{text}"
        );
        let mut fresh = crate::testing::world();
        fresh.restore("crews.ini", &text).unwrap();
        let heist = fresh.crews.of("zer0").and_then(|crew| crew.heist.clone());
        assert_eq!(
            heist.as_ref().map(|heist| heist.entered.clone()),
            Some(vec!["nyx".to_owned()])
        );
        let contract = heist.and_then(|heist| heist.contract);
        let goal = contract
            .and_then(|index| fresh.contracts.get(index))
            .map(|contract| contract.goal.clone());
        let before = world
            .crews
            .of("nyx")
            .and_then(|crew| crew.heist.as_ref())
            .and_then(|heist| heist.contract);
        let goal_before = before
            .and_then(|index| world.contracts.get(index))
            .map(|contract| contract.goal.clone());
        assert_eq!(goal, goal_before);
    }
}
