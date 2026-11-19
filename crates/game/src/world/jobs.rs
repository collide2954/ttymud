use ttymud_text::{Kind, Message, Style};

use super::alert::Alert;
use super::contract::{Contract, Job};
use super::{Host, HostId, Network, NetworkId, PlayerId, Step, World, lines, listed};
use crate::content::{CorpDef, FixerDef};
use crate::heat::Security;
use crate::random::{Random, mix};
use crate::target::{Layer, Target, generate};

const SLOTS: u64 = 3;
pub(super) const DAY: u64 = 86_400;

#[derive(Debug)]
pub(super) struct Fixer {
    pub(super) id: String,
    pub(super) name: String,
    pub(super) host: HostId,
    pub(super) corps: Vec<usize>,
    pub(super) max_heat: Option<u32>,
}

#[derive(Debug)]
pub(super) struct Offer {
    pub(super) fixer: usize,
    pub(super) day: u64,
    pub(super) slot: u64,
    pub(super) corp: usize,
    pub(super) target: Target,
    pub(super) pay: u32,
}

pub(super) fn fixers(
    defs: Vec<FixerDef>,
    corps: &[CorpDef],
    find: impl Fn(&str) -> HostId,
) -> Vec<Fixer> {
    defs.into_iter()
        .map(|def| Fixer {
            corps: def
                .corps
                .iter()
                .filter_map(|id| corps.iter().position(|corp| &corp.id == id))
                .collect(),
            id: def.id,
            name: def.name,
            host: find(&def.host),
            max_heat: def.max_heat,
        })
        .collect()
}

fn base_pay(budget: Security) -> u32 {
    match budget {
        Security::Low => 300,
        Security::Medium => 700,
        Security::High => 1500,
        Security::Top => 3000,
    }
}

fn needed(budget: Security) -> u32 {
    match budget {
        Security::Low => 0,
        Security::Medium => 2,
        Security::High => 5,
        Security::Top => 9,
    }
}

impl World {
    fn fixers_at(&self, host: HostId) -> Vec<usize> {
        (0..self.fixers.len())
            .filter(|&index| self.fixers[index].host == host)
            .collect()
    }

    pub(super) fn fixer_line(&self, host: HostId) -> Option<Message> {
        let here = self.fixers_at(host);
        if here.is_empty() {
            return None;
        }
        let names = here.iter().map(|&index| self.fixers[index].name.as_str());
        let line = listed(Message::game("Fixers here: "), Style::Player, names);
        Some(
            line.text(". Type ")
                .span(Style::Command, "jobs")
                .text(" to see their work."),
        )
    }

    pub(super) fn has_fixer(&self, host: HostId) -> bool {
        self.fixers.iter().any(|fixer| fixer.host == host)
    }

    pub(super) fn fixer_name(&self, fixer: usize) -> &str {
        self.fixers
            .get(fixer)
            .map_or("", |fixer| fixer.name.as_str())
    }

    fn rep_of(&self, player: PlayerId, fixer: usize) -> u32 {
        let id = self.fixers.get(fixer).map(|fixer| &fixer.id);
        let state = self.players.get(&player);
        state
            .zip(id)
            .and_then(|(state, id)| state.rep.get(id).copied())
            .unwrap_or(0)
    }

    fn refusal(&self, player: PlayerId, fixer: usize) -> Option<Message> {
        let most = self.fixers.get(fixer)?.max_heat?;
        let heat = self.players.get(&player)?.heat;
        (heat > most).then(|| {
            Message::new(Kind::Error)
                .span(Style::Player, self.fixer_name(fixer))
                .text(format!(
                    " does not work with handles at heat {} or more.",
                    most + 1
                ))
        })
    }

    fn is_done(&self, player: PlayerId, offer: &Offer) -> bool {
        let Some(state) = self.players.get(&player) else {
            return false;
        };
        let fixer = &self.fixers[offer.fixer].id;
        state
            .done
            .iter()
            .any(|(day, id, slot)| *day == offer.day && id == fixer && *slot == offer.slot)
    }

    pub(super) fn gain_rep(&mut self, player: PlayerId, fixer: usize) -> Option<Message> {
        let def = self.fixers.get(fixer)?;
        let rep = self
            .players
            .get_mut(&player)?
            .rep
            .entry(def.id.clone())
            .or_default();
        *rep += 1;
        let rep = *rep;
        Some(
            Message::game("Your reputation with ")
                .span(Style::Player, &def.name)
                .text(format!(": {rep}.")),
        )
    }

    fn posted(&self, fixer: usize, day: u64, slot: u64) -> Option<Offer> {
        let def = &self.fixers[fixer];
        let mut random = Random::new(mix(&[self.seed, day, fixer as u64, slot]));
        let corp = if slot == 0 {
            def.corps
                .iter()
                .copied()
                .min_by_key(|&corp| self.corps[corp].budget)?
        } else {
            *def.corps.get(random.below(def.corps.len()))?
        };
        let budget = self.corps[corp].budget;
        let percent = 80 + u32::try_from(random.below(41)).unwrap_or(0);
        let pay = base_pay(budget) * percent / 100 / 10 * 10;
        let target = generate(&self.corps[corp], &self.templates, random.draw());
        Some(Offer {
            fixer,
            day,
            slot,
            corp,
            target,
            pay,
        })
    }

    fn offers_at(&self, host: HostId, day: u64) -> Vec<Offer> {
        self.fixers_at(host)
            .into_iter()
            .flat_map(|fixer| (0..SLOTS).filter_map(move |slot| self.posted(fixer, day, slot)))
            .collect()
    }

    fn no_fixer(&self) -> Message {
        let mut hosts: Vec<&str> = self
            .fixers
            .iter()
            .map(|fixer| self.hosts[fixer.host.0].id.as_str())
            .collect();
        hosts.dedup();
        let message = Message::error("There is no fixer here. Fixers: ");
        listed(message, Style::Host, hosts)
    }

    pub(super) fn job_entries(&self, player: PlayerId) -> Vec<HostId> {
        let job = self.players.get(&player).and_then(|state| state.contract);
        job.and_then(|job| self.contracts.get(job.contract()))
            .map_or_else(Vec::new, |contract| {
                self.networks[contract.network.0].entries.clone()
            })
    }

    pub(super) fn accept(&mut self, player: PlayerId, args: &[String]) -> Result<Step, Message> {
        let here = self.here(player);
        if !self.has_fixer(here) {
            return Err(self.no_fixer());
        }
        let [word] = args else {
            return Err(Message::error("Usage: accept <number>. Type ")
                .span(Style::Command, "jobs")
                .text(" to see the jobs."));
        };
        let mut offers = self.offers_at(here, self.now / DAY);
        let number = word
            .parse()
            .ok()
            .filter(|number| (1..=offers.len()).contains(number));
        let Some(number) = number else {
            return Err(
                Message::error(format!("There is no job number {word}. Type "))
                    .span(Style::Command, "jobs")
                    .text(" to see the jobs."),
            );
        };
        if self
            .players
            .get(&player)
            .is_some_and(|state| state.contract.is_some())
        {
            return Err(Message::error("You already have a contract. Type ")
                .span(Style::Command, "contract drop")
                .text(" to end it."));
        }
        let offer = offers.swap_remove(number - 1);
        if let Some(refusal) = self.refusal(player, offer.fixer) {
            return Err(refusal);
        }
        if self.is_done(player, &offer) {
            return Err(Message::error("You did this job today. ")
                .span(Style::Player, self.fixer_name(offer.fixer))
                .text(" posts new jobs each day."));
        }
        let need = needed(self.corps[offer.corp].budget);
        let rep = self.rep_of(player, offer.fixer);
        if rep < need {
            let name = self.fixer_name(offer.fixer);
            return Err(Message::new(Kind::Error)
                .span(Style::Player, name)
                .text(format!(
                    " gives this job only to handles with {need} reputation. "
                ))
                .text("Your reputation with ")
                .span(Style::Player, name)
                .text(format!(": {rep}.")));
        }
        let fixer = self.fixers[offer.fixer].name.clone();
        let corp = self.corps[offer.corp].name.clone();
        let goal = offer.target.goal.clone();
        let contract = self.post(offer);
        if let Some(state) = self.players.get_mut(&player) {
            state.contract = Some(Job::new(contract));
        }
        let home = self.home.map_or_else(
            || "your home host".to_owned(),
            |home| self.hosts[home.0].id.clone(),
        );
        Ok(Step::show(vec![
            Message::game("")
                .span(Style::Player, &fixer)
                .text(format!(": The target is {corp}. Get "))
                .span(Style::File, &goal)
                .text(" and leave clean."),
            Message::game("Your way in starts at ")
                .span(Style::Host, &home)
                .text("."),
            Message::game("Type ")
                .span(Style::Command, "contract")
                .text(" to see the job."),
        ]))
    }

    #[cfg(test)]
    pub(crate) fn job_entry(&self) -> String {
        let entry = self
            .networks
            .last()
            .and_then(|network| network.entries.first());
        entry.map_or_default(|entry| self.hosts[entry.0].id.clone())
    }

    pub(super) fn resume_posting(&mut self, fixer: &str, day: u64, slot: u64) -> Option<usize> {
        let fixer = self.fixers.iter().position(|known| known.id == fixer)?;
        let offer = self.posted(fixer, day, slot)?;
        Some(self.post(offer))
    }

    fn post(&mut self, offer: Offer) -> usize {
        let key = (offer.day, offer.fixer, offer.slot);
        if let Some(&index) = self.postings.get(&key) {
            return index;
        }
        let network = self.add_target(&offer.target, offer.corp);
        let corp = &self.corps[offer.corp].name;
        let goal = offer.target.goal;
        self.contracts.push(Contract {
            id: format!("job-{}-{}-{}", offer.day, offer.fixer, offer.slot),
            fixer: offer.fixer,
            network,
            offer: vec![
                format!("The target is {corp}."),
                format!("Get {goal} and leave clean."),
            ],
            goal,
            steps: Vec::new(),
            pay: offer.pay,
            posting: Some(key),
        });
        let index = self.contracts.len() - 1;
        self.postings.insert(key, index);
        index
    }

    fn add_target(&mut self, target: &Target, corp: usize) -> NetworkId {
        let network = NetworkId(self.networks.len());
        let base = self.hosts.len();
        let mut entries = Vec::new();
        for (index, def) in target.hosts.iter().enumerate() {
            let mut routes = Vec::new();
            if def.layer == Layer::Edge {
                entries.push(HostId(base + index));
                routes.extend(self.home);
            }
            routes.extend(def.routes.iter().map(|&route| HostId(base + route)));
            self.hosts.push(Host {
                id: def.id.clone(),
                name: def.name.clone(),
                description: def.description.clone(),
                network: Some(network),
                routes,
                files: def.files.clone(),
                ice: def.ice.clone(),
                security: Some(def.security),
                software: def.software.clone(),
            });
        }
        let security = target.hosts.iter().map(|host| host.security).max();
        self.networks.push(Network {
            name: target.name.clone(),
            security: security.unwrap_or(Security::Low),
            entries,
            keys: target.keys.clone(),
            corp: Some(corp),
        });
        network
    }

    pub(super) fn jobs(&self, player: PlayerId) -> Result<Step, Message> {
        let here = self.here(player);
        if !self.has_fixer(here) {
            return Err(self.no_fixer());
        }
        let mut shown = Vec::new();
        let mut number = 0;
        let day = self.now / DAY;
        for fixer in self.fixers_at(here) {
            let name = Message::game("").span(Style::Player, self.fixer_name(fixer));
            let offers: Vec<Offer> = (0..SLOTS)
                .filter_map(|slot| self.posted(fixer, day, slot))
                .collect();
            if offers.is_empty() {
                shown.push(name.text(" has no jobs today."));
                continue;
            }
            shown.push(name.text(" has these jobs today:"));
            shown.extend(self.refusal(player, fixer));
            let rep = self.rep_of(player, fixer);
            for offer in offers {
                number += 1;
                let corp = &self.corps[offer.corp];
                let alert = match self.alert(offer.corp) {
                    Alert::Green => String::new(),
                    hot => format!(" ({} alert)", hot.word()),
                };
                let mut line = Message::game(format!("{number}. {}{alert}: get ", corp.name))
                    .span(Style::File, &offer.target.goal)
                    .text(format!(". Pay: {} cr.", offer.pay));
                let need = needed(corp.budget);
                if self.is_done(player, &offer) {
                    line = line.text(" Done.");
                } else if rep < need {
                    line = line.text(format!(" Needs {need} reputation."));
                }
                shown.push(line);
            }
        }
        let standing: Vec<String> = self
            .fixers_at(here)
            .into_iter()
            .map(|fixer| format!("{} {}", self.fixer_name(fixer), self.rep_of(player, fixer)))
            .collect();
        let standing = standing.iter().map(String::as_str);
        shown.push(listed(
            Message::game("Your reputation: "),
            Style::Plain,
            standing,
        ));
        shown.push(
            Message::game("Type ")
                .span(Style::Command, "accept <number>")
                .text(" to take a job."),
        );
        Ok(lines(shown))
    }
}

#[cfg(test)]
mod tests {
    use crate::heat::Security;
    use crate::target::Level;
    use crate::testing::{plain, run, walk, world};
    use crate::world::{HostId, NetworkId};
    use crate::{PlayerId, World};

    const DAY: u64 = 86_400;

    fn board(day: u64) -> Vec<String> {
        let mut world = world();
        world.set_time(day * DAY + 100);
        let (player, _) = world.join("ghost");
        world.command(player, "go bazaar");
        plain(&world.command(player, "jobs"))
    }

    #[test]
    fn jobs_lists_the_offers_of_each_fixer_for_the_day() {
        let shown = board(20_000);
        assert_eq!(shown.len(), 6, "{shown:?}");
        assert_eq!(shown[0], "Pell has these jobs today:");
        assert!(shown[1].starts_with("1. Acme Corp: get "), "{shown:?}");
        for (index, line) in shown[1..4].iter().enumerate() {
            assert!(line.starts_with(&format!("{}. ", index + 1)), "{line}");
            assert!(line.contains(". Pay: "), "{line}");
        }
        assert_eq!(shown[4], "Your reputation: Pell 0");
        assert_eq!(shown[5], "Type accept <number> to take a job.");
    }

    fn globex() -> (World, PlayerId, String) {
        for day in 20_000.. {
            let mut world = world();
            world.set_time(day * DAY + 100);
            let (player, _) = world.join("ghost");
            world.command(player, "go bazaar");
            let shown = plain(&world.command(player, "jobs"));
            if let Some(line) = shown.iter().find(|line| line.contains(" Globex: ")) {
                let number = line.split('.').next().unwrap_or_default().to_owned();
                assert!(line.ends_with(" cr. Needs 2 reputation."), "{line}");
                return (world, player, number);
            }
        }
        unreachable!()
    }

    #[test]
    fn a_job_against_a_bigger_corp_needs_reputation() {
        let (mut world, player, number) = globex();
        assert_eq!(
            plain(&world.command(player, &format!("accept {number}"))),
            [
                "Pell gives this job only to handles with 2 reputation. Your reputation with Pell: 0."
            ]
        );
        if let Some(state) = world.players.get_mut(&player) {
            state.rep.insert("pell".to_owned(), 2);
        }
        let shown = plain(&world.command(player, &format!("accept {number}")));
        assert!(
            shown[0].starts_with("Pell: The target is Globex."),
            "{shown:?}"
        );
    }

    #[test]
    fn the_board_is_the_same_for_all_and_new_each_day() {
        assert_eq!(board(20_000), board(20_000));
        assert_ne!(board(20_000), board(20_001));
    }

    #[test]
    fn jobs_works_only_where_a_fixer_is() {
        assert_eq!(run("jobs"), ["There is no fixer here. Fixers: bazaar"]);
    }

    #[test]
    fn the_scene_names_the_fixers_here() {
        assert_eq!(
            walk(&["go bazaar"]).0.last().map(String::as_str),
            Some("Fixers here: Pell. Type jobs to see their work.")
        );
    }

    fn hired(job: &str) -> (World, PlayerId, Vec<String>) {
        let mut world = world();
        world.set_time(20_000 * DAY + 100);
        let (player, _) = world.join("ghost");
        world.command(player, "go bazaar");
        let shown = plain(&world.command(player, &format!("accept {job}")));
        (world, player, shown)
    }

    fn entries(world: &World) -> Vec<String> {
        let network = world.networks.len() - 1;
        world.networks[network]
            .entries
            .iter()
            .map(|&entry| world.hosts[entry.0].id.clone())
            .collect()
    }

    #[test]
    fn accept_takes_a_job_and_opens_a_way_in_at_home() {
        let (mut world, player, shown) = hired("1");
        assert_eq!(shown.len(), 3, "{shown:?}");
        assert!(
            shown[0].starts_with("Pell: The target is Acme Corp. Get "),
            "{shown:?}"
        );
        assert_eq!(
            shown[1..],
            [
                "Your way in starts at back-room.",
                "Type contract to see the job."
            ]
        );
        assert_eq!(
            plain(&world.command(player, "contract"))[1],
            "Target: Acme Corp"
        );
        for line in ["go relay", "go back-room"] {
            world.command(player, line);
        }
        let routes = plain(&world.command(player, "ls"));
        for entry in entries(&world) {
            assert!(routes.contains(&format!("{entry}/")), "{entry}: {routes:?}");
        }
        let entry = entries(&world).remove(0);
        let shown = plain(&world.command(player, &format!("go {entry}")));
        assert_eq!(shown[0], "You connect to the network of Acme Corp.");
        assert!(shown[1].starts_with("Acme Corp "), "{shown:?}");
        assert!(shown[3].starts_with("Routes: back-room, "), "{shown:?}");
    }

    #[test]
    fn others_do_not_see_the_way_in() {
        let (mut world, _, _) = hired("1");
        let (other, _) = world.join("nyx");
        world.command(other, "go back-room");
        let routes = plain(&world.command(other, "ls"));
        assert_eq!(routes, ["relay/", "gate/"]);
    }

    #[test]
    fn a_clean_exit_with_the_goal_pays_for_the_job() {
        let (mut world, player, _) = hired("1");
        let entry = entries(&world).remove(0);
        for line in ["go relay", "go back-room", &format!("go {entry}")] {
            world.command(player, line);
        }
        let goal = world
            .contracts
            .last()
            .map_or_default(|job| job.goal.clone());
        let pay = world.contracts.last().map_or(0, |job| job.pay);
        let holder = (0..world.hosts.len())
            .find(|&index| {
                world.hosts[index]
                    .files
                    .iter()
                    .any(|file| file.name == goal)
            })
            .map(HostId);
        if let (Some(state), Some(holder)) = (world.players.get_mut(&player), holder) {
            state.chain.push(holder);
            if let Some(run) = state.run.as_mut() {
                run.access = Level::Admin;
            }
        }
        world.command(player, &format!("download {goal}"));
        world.command(player, "back");
        let shown = plain(&world.command(player, "back"));
        assert_eq!(
            shown[..4],
            [
                "You leave the network of Acme Corp. Clean exit.".to_owned(),
                "Pell: Clean work. The contract is complete.".to_owned(),
                format!("Pell takes {goal} and pays you {pay} cr."),
                "Your reputation with Pell: 1.".to_owned(),
            ]
        );
    }

    fn finish(world: &mut World, player: PlayerId) {
        let entry = entries(world).remove(0);
        for line in ["go relay", "go back-room", &format!("go {entry}")] {
            world.command(player, line);
        }
        let goal = world
            .contracts
            .last()
            .map_or_default(|job| job.goal.clone());
        let holder = (0..world.hosts.len())
            .find(|&index| {
                world.hosts[index]
                    .files
                    .iter()
                    .any(|file| file.name == goal)
            })
            .map(HostId);
        if let (Some(state), Some(holder)) = (world.players.get_mut(&player), holder) {
            state.chain.push(holder);
            if let Some(run) = state.run.as_mut() {
                run.access = Level::Admin;
            }
        }
        for line in [
            &format!("download {goal}"),
            "back",
            "back",
            "go relay",
            "go bazaar",
        ] {
            world.command(player, line);
        }
    }

    #[test]
    fn a_finished_job_cannot_be_taken_again_that_day() {
        let (mut world, player, _) = hired("1");
        finish(&mut world, player);
        let board = plain(&world.command(player, "jobs"));
        assert!(board[1].ends_with(" cr. Done."), "{board:?}");
        assert_eq!(
            plain(&world.command(player, "accept 1")),
            ["You did this job today. Pell posts new jobs each day."]
        );
        world.set_time(20_001 * DAY + 100);
        let shown = plain(&world.command(player, "accept 1"));
        assert!(shown[0].starts_with("Pell: The target is "), "{shown:?}");
    }

    #[test]
    fn a_careful_fixer_refuses_a_hot_handle() {
        let (mut world, player, _) = hired("0");
        world.fixers[0].max_heat = Some(3);
        if let Some(state) = world.players.get_mut(&player) {
            state.heat = 4;
        }
        assert_eq!(
            plain(&world.command(player, "jobs"))[1],
            "Pell does not work with handles at heat 4 or more."
        );
        assert_eq!(
            plain(&world.command(player, "accept 1")),
            ["Pell does not work with handles at heat 4 or more."]
        );
    }

    #[test]
    fn one_offer_makes_one_network_for_all() {
        let (mut world, _, _) = hired("1");
        let hosts = world.hosts.len();
        let (other, _) = world.join("nyx");
        world.command(other, "go bazaar");
        world.command(other, "accept 1");
        assert_eq!(world.hosts.len(), hosts);
        world.command(other, "contract drop");
        world.command(other, "accept 2");
        assert!(world.hosts.len() > hosts);
    }

    #[test]
    fn accept_needs_a_fixer_a_job_number_and_no_contract() {
        assert_eq!(run("accept 1"), ["There is no fixer here. Fixers: bazaar"]);
        let (mut world, player, _) = hired("9");
        let shown = plain(&world.command(player, "accept 9"));
        assert_eq!(
            shown,
            ["There is no job number 9. Type jobs to see the jobs."]
        );
        world.command(player, "accept 1");
        assert_eq!(
            plain(&world.command(player, "accept 2")),
            ["You already have a contract. Type contract drop to end it."]
        );
    }

    #[test]
    fn a_trace_in_a_target_gives_the_heat_of_the_host() {
        let (mut world, player, _) = hired("1");
        let entry = entries(&world).remove(0);
        for line in ["go relay", "go back-room", &format!("go {entry}")] {
            world.command(player, line);
        }
        let network = world.networks.len() - 1;
        let core = (0..world.hosts.len())
            .find(|&index| {
                world.hosts[index].network == Some(NetworkId(network))
                    && world.hosts[index].security == Some(Security::Medium)
            })
            .map(HostId);
        if let (Some(state), Some(core)) = (world.players.get_mut(&player), core) {
            state.chain.push(core);
            if let Some(run) = state.run.as_mut() {
                run.trace = 98;
            }
        }
        let shown = plain(&world.command(player, "scan"));
        assert!(
            shown.contains(&"Your heat: 0 -> 2.".to_owned()),
            "{shown:?}"
        );
    }
}
