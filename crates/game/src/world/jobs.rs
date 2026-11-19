use ttymud_text::{Message, Style};

use super::{HostId, PlayerId, Step, World, lines, listed};
use crate::content::{CorpDef, FixerDef};
use crate::heat::Security;
use crate::random::{Random, mix};
use crate::target::{Target, generate};

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

    pub(super) fn jobs(&self, player: PlayerId) -> Result<Step, Message> {
        let here = self.here(player);
        if !self.has_fixer(here) {
            let mut hosts: Vec<&str> = self
                .fixers
                .iter()
                .map(|fixer| self.hosts[fixer.host.0].id.as_str())
                .collect();
            hosts.dedup();
            let message = Message::error("There is no fixer here. Fixers: ");
            return Err(listed(message, Style::Host, hosts));
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
    use crate::testing::{plain, run, walk, world};

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
}
