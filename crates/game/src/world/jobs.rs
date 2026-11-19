use ttymud_text::{Message, Style};

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
    pub(super) name: String,
    pub(super) host: HostId,
    pub(super) corps: Vec<usize>,
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
            name: def.name,
            host: find(&def.host),
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

    fn post(&mut self, offer: Offer) -> usize {
        let key = (offer.day, offer.fixer, offer.slot);
        if let Some(&index) = self.postings.get(&key) {
            return index;
        }
        let network = self.add_target(&offer.target);
        let corp = &self.corps[offer.corp].name;
        let goal = offer.target.goal;
        self.contracts.push(Contract {
            id: format!("job-{}-{}-{}", offer.day, offer.fixer, offer.slot),
            fixer: self.fixers[offer.fixer].name.clone(),
            network,
            offer: vec![
                format!("The target is {corp}."),
                format!("Get {goal} and leave clean."),
            ],
            goal,
            steps: Vec::new(),
            pay: offer.pay,
        });
        let index = self.contracts.len() - 1;
        self.postings.insert(key, index);
        index
    }

    fn add_target(&mut self, target: &Target) -> NetworkId {
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
            });
        }
        let security = target.hosts.iter().map(|host| host.security).max();
        self.networks.push(Network {
            name: target.name.clone(),
            security: security.unwrap_or(Security::Low),
            entries,
        });
        network
    }

    pub(super) fn jobs(&self, player: PlayerId) -> Result<Step, Message> {
        let here = self.here(player);
        if !self.has_fixer(here) {
            return Err(self.no_fixer());
        }
        let mut shown = Vec::new();
        let mut last = None;
        for (index, offer) in self.offers_at(here, self.now / DAY).iter().enumerate() {
            let fixer = &self.fixers[offer.fixer];
            if last != Some(offer.fixer) {
                last = Some(offer.fixer);
                let header = Message::game("").span(Style::Player, &fixer.name);
                shown.push(header.text(" has these jobs today:"));
            }
            let corp = &self.corps[offer.corp].name;
            let number = index + 1;
            shown.push(
                Message::game(format!("{number}. {corp}: get "))
                    .span(Style::File, &offer.target.goal)
                    .text(format!(". Pay: {} cr.", offer.pay)),
            );
        }
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
        assert_eq!(shown.len(), 5, "{shown:?}");
        assert_eq!(shown[0], "Pell has these jobs today:");
        assert!(shown[1].starts_with("1. Acme Corp: get "), "{shown:?}");
        for (index, line) in shown[1..4].iter().enumerate() {
            assert!(line.starts_with(&format!("{}. ", index + 1)), "{line}");
            assert!(line.contains(". Pay: "), "{line}");
        }
        assert_eq!(shown[4], "Type accept <number> to take a job.");
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
        }
        world.command(player, &format!("download {goal}"));
        world.command(player, "back");
        let shown = plain(&world.command(player, "back"));
        assert_eq!(
            shown[..3],
            [
                "You leave the network of Acme Corp. Clean exit.".to_owned(),
                "Pell: Clean work. The contract is complete.".to_owned(),
                format!("Pell takes {goal} and pays you {pay} cr."),
            ]
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
