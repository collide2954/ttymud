use ttymud_text::{Message, Style};

use super::jobs::DAY;
use super::{PlayerId, Step, World, lines};
use crate::random::{Random, mix};

const PER_DAY: usize = 3;
const SALT: u64 = 0x7275;
const TOPICS: [Topic; 3] = [Topic::Alert, Topic::Patch, Topic::Shift];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Topic {
    Alert,
    Patch,
    Shift,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Truth {
    True,
    Old,
    False,
}

fn truth(rep: u32, roll: usize) -> Truth {
    let chance = rep.saturating_mul(10).saturating_add(50).min(90);
    match roll {
        roll if roll < usize::try_from(chance).unwrap_or(90) => Truth::True,
        roll if roll.is_multiple_of(2) => Truth::Old,
        _ => Truth::False,
    }
}

impl World {
    pub(super) fn rumors(&self, player: PlayerId) -> Result<Step, Message> {
        let here = self.here(player);
        if !self.has_fixer(here) {
            return Err(self.no_fixer());
        }
        let handle = self.handle(player).unwrap_or_default();
        let named: Vec<u64> = handle.bytes().map(u64::from).collect();
        let day = self.now / DAY;
        let mut shown = Vec::new();
        for fixer in self.fixers_at(here) {
            let corps = &self.fixers[fixer].corps;
            let rep = self.rep_of(player, fixer);
            let seed = mix(&[self.seed, day, fixer as u64, mix(&named), SALT]);
            let mut random = Random::new(seed);
            for _ in 0..PER_DAY {
                let corp = corps.get(random.below(corps.len())).copied();
                let topic = TOPICS[random.below(TOPICS.len())];
                let truth = truth(rep, random.below(100));
                let (thing, lie) = (random.below(1_000), random.below(1_000));
                if let Some(corp) = corp {
                    let rumor = self.rumor(corp, topic, truth, thing, lie);
                    shown.push(
                        Message::game("")
                            .span(Style::Player, self.fixer_name(fixer))
                            .text(format!(": I hear that {rumor}")),
                    );
                }
            }
        }
        Ok(lines(shown))
    }

    fn rumor(&self, corp: usize, topic: Topic, truth: Truth, thing: usize, lie: usize) -> String {
        let name = &self.corps[corp].name;
        let mut families: Vec<&str> = self
            .templates
            .iter()
            .filter_map(|template| template.software.as_deref())
            .collect();
        families.sort_unstable();
        families.dedup();
        match topic {
            Topic::Patch if !families.is_empty() => {
                let family = families[thing % families.len()];
                let patched = self.patched(corp, family);
                let said = match truth {
                    Truth::True => patched,
                    Truth::Old => false,
                    Truth::False => !patched,
                };
                if said {
                    format!("{name} patched the hole in {family}.")
                } else {
                    format!("the hole in {family} at {name} is still open.")
                }
            }
            Topic::Shift => {
                let day = thing.is_multiple_of(2);
                let crew = self.crew(corp, day);
                let Some((person, habit)) = crew.get(thing / 2 % crew.len().max(1)) else {
                    return format!("{name} is on {} alert.", self.alert(corp).word());
                };
                let habit = match truth {
                    Truth::True | Truth::Old => *habit,
                    Truth::False => habit.other(lie),
                };
                let (shift, word) = (if day { "days" } else { "nights" }, habit.word());
                format!("{person} works {shift} at {name} and is {word}.")
            }
            Topic::Alert | Topic::Patch => {
                let level = match truth {
                    Truth::True => self.alert(corp),
                    Truth::Old => self.last_alert(corp),
                    Truth::False => self.alert(corp).other(lie),
                };
                format!("{name} is on {} alert.", level.word())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Topic, Truth, truth};
    use crate::testing::{plain, walk, world};
    use crate::{PlayerId, World};

    const NOON: u64 = 20_000 * 86_400 + 12 * 3_600;

    fn at_bazaar(time: u64) -> (World, PlayerId) {
        let mut world = world();
        world.set_time(time);
        let (player, _) = world.join("ghost");
        world.command(player, "go bazaar");
        (world, player)
    }

    fn acme(world: &World) -> usize {
        let found = world.corps.iter().position(|corp| corp.id == "acme");
        found.unwrap_or_default()
    }

    #[test]
    fn rumors_need_a_fixer() {
        assert_eq!(
            walk(&["rumors"]).0,
            ["There is no fixer here. Fixers: bazaar"]
        );
    }

    #[test]
    fn a_fixer_tells_three_rumors_each_day_and_the_same_ones_again() {
        let (mut world, player) = at_bazaar(NOON);
        let shown = plain(&world.command(player, "rumors"));
        assert_eq!(shown.len(), 3, "{shown:?}");
        assert!(
            shown
                .iter()
                .all(|line| line.starts_with("Pell: I hear that ")),
            "{shown:?}"
        );
        assert_eq!(plain(&world.command(player, "rumours")), shown);
        let later: Vec<Vec<String>> = (1..10)
            .map(|day| {
                world.set_time(NOON + day * 86_400);
                plain(&world.command(player, "rumors"))
            })
            .collect();
        assert!(later.iter().any(|told| *told != shown), "{later:?}");
    }

    #[test]
    fn a_better_reputation_makes_a_true_rumor_more_likely() {
        assert_eq!(truth(0, 49), Truth::True);
        assert_ne!(truth(0, 50), Truth::True);
        assert_eq!(truth(2, 69), Truth::True);
        assert_eq!(truth(9, 89), Truth::True);
        assert_ne!(truth(9, 90), Truth::True);
        assert_eq!([truth(0, 60), truth(0, 61)], [Truth::Old, Truth::False]);
    }

    #[test]
    fn an_alert_rumor_is_now_before_the_last_cooling_or_wrong() {
        let (mut world, _) = at_bazaar(NOON);
        let acme = acme(&world);
        world.raise_alert(acme, 60);
        world.set_time(NOON + 20 * 300);
        let told = |truth| world.rumor(acme, Topic::Alert, truth, 0, 0);
        assert_eq!(told(Truth::True), "Acme Corp is on yellow alert.");
        assert_eq!(told(Truth::Old), "Acme Corp is on orange alert.");
        assert_eq!(told(Truth::False), "Acme Corp is on orange alert.");
        assert_eq!(
            world.rumor(acme, Topic::Alert, Truth::False, 0, 1),
            "Acme Corp is on red alert."
        );
    }

    #[test]
    fn a_patch_rumor_is_now_before_the_patch_or_wrong() {
        let (mut world, _) = at_bazaar(NOON);
        let acme = acme(&world);
        for _ in 0..12 {
            world.expose(acme, "Deskline OS");
        }
        let told = |truth| world.rumor(acme, Topic::Patch, truth, 0, 0);
        assert_eq!(
            told(Truth::True),
            "Acme Corp patched the hole in Deskline OS."
        );
        let open = "the hole in Deskline OS at Acme Corp is still open.";
        assert_eq!(told(Truth::Old), open);
        assert_eq!(told(Truth::False), open);
        assert_eq!(
            world.rumor(acme, Topic::Patch, Truth::True, 1, 0),
            "the hole in Gatekeep VPN at Acme Corp is still open."
        );
    }

    #[test]
    fn a_shift_rumor_names_an_analyst_and_a_habit_that_is_true_or_wrong() {
        let (world, _) = at_bazaar(NOON);
        let acme = acme(&world);
        let crew = world.crew(acme, true);
        let (name, habit) = &crew[1];
        let told = |truth| world.rumor(acme, Topic::Shift, truth, 2, 0);
        let kind = habit.word();
        let true_rumor = format!("{name} works days at Acme Corp and is {kind}.");
        assert_eq!(told(Truth::True), true_rumor);
        assert_eq!(told(Truth::Old), true_rumor);
        assert_ne!(told(Truth::False), true_rumor);
        let (night, _) = &world.crew(acme, false)[0];
        assert!(
            world
                .rumor(acme, Topic::Shift, Truth::True, 1, 0)
                .starts_with(&format!("{night} works nights at Acme Corp and is "))
        );
    }
}
