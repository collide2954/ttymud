use ttymud_text::{Kind, Message, Style};

use super::run::MAX_TRACE;
use super::{HostId, NetworkId, PlayerId, World};
use crate::random::{Random, mix};

const POOL: usize = 6;
const DAY_STAFF: usize = 2;
const NIGHT_STAFF: usize = 1;
const READ_TICKS: u32 = 2;
const FOUND_TRACE: u32 = 5;
const SALT: u64 = 0x050c;
const INITIALS: &[u8] = b"abcdefghjklmnprstvwz";
const SURNAMES: [&str; 16] = [
    "ortiz", "lin", "okafor", "novak", "silva", "haddad", "kowal", "tanaka", "mbeki", "larsen",
    "costa", "ivanova", "reyes", "osei", "becker", "nakamura",
];
const HABITS: [Habit; 3] = [Habit::Careful, Habit::Strict, Habit::Lazy];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Habit {
    Careful,
    Strict,
    Lazy,
}

impl Habit {
    fn every(self) -> u32 {
        match self {
            Self::Careful => 8,
            Self::Strict => 6,
            Self::Lazy => 12,
        }
    }

    fn word(self) -> &'static str {
        match self {
            Self::Careful => "careful",
            Self::Strict => "strict",
            Self::Lazy => "lazy",
        }
    }
}

#[derive(Debug, Clone)]
pub(super) struct Analyst {
    pub(super) name: String,
    pub(super) habit: Habit,
    pub(super) wait: u32,
    pub(super) target: Option<HostId>,
}

pub(super) fn day_shift(now: u64) -> bool {
    (8..20).contains(&(now / 3_600 % 24))
}

fn pool(seed: u64, corp: usize) -> Vec<(String, Habit)> {
    let mut random = Random::new(mix(&[seed, corp as u64, SALT]));
    let mut people: Vec<(String, Habit)> = Vec::new();
    for _ in 0..POOL * 10 {
        let initial = char::from(INITIALS[random.below(INITIALS.len())]);
        let name = format!("{initial}.{}", SURNAMES[random.below(SURNAMES.len())]);
        let habit = HABITS[random.below(HABITS.len())];
        if !people.iter().any(|(known, _)| *known == name) {
            people.push((name, habit));
        }
        if people.len() == POOL {
            break;
        }
    }
    people
}

impl World {
    pub(super) fn staff(&self, network: NetworkId) -> Vec<Analyst> {
        let Some(corp) = self.networks[network.0].corp else {
            return Vec::new();
        };
        let day = day_shift(self.now);
        let base = if day { DAY_STAFF } else { NIGHT_STAFF };
        let extra = usize::try_from(self.alert(corp).steps()).unwrap_or(0);
        let mut people = pool(self.seed, corp);
        if !day {
            people.reverse();
        }
        let shift = people.into_iter().take(base + extra);
        shift
            .zip(0..)
            .map(|((name, habit), index)| Analyst {
                name,
                habit,
                wait: habit.every() + 2 * index,
                target: None,
            })
            .collect()
    }

    pub(super) fn night_bonus(&self, network: NetworkId) -> u32 {
        u32::from(self.networks[network.0].corp.is_some() && !day_shift(self.now))
    }

    pub(super) fn soc_line(&self, player: PlayerId) -> Option<Message> {
        let run = self.players.get(&player)?.run.as_ref()?;
        let (first, rest) = run.analysts.split_first()?;
        let named = |line: Message, analyst: &Analyst| {
            line.span(Style::Player, &analyst.name)
                .text(format!(" ({})", analyst.habit.word()))
        };
        let mut line = named(Message::game("SOC on shift: "), first);
        for analyst in rest {
            line = named(line.text(", "), analyst);
        }
        Some(line.text("."))
    }

    pub(super) fn tick_soc(&mut self, player: PlayerId) -> Vec<Message> {
        let here = self.here(player);
        let Some(run) = self
            .players
            .get_mut(&player)
            .and_then(|state| state.run.as_mut())
        else {
            return Vec::new();
        };
        let network = run.network;
        let hosts: Vec<HostId> = (0..self.hosts.len())
            .filter(|&index| self.hosts[index].network == Some(network))
            .map(HostId)
            .collect();
        let noisiest = run
            .logs
            .iter()
            .max_by_key(|&&(_, noise)| noise)
            .map(|&(host, _)| host);
        let mut events = Vec::new();
        for analyst in &mut run.analysts {
            analyst.wait = analyst.wait.saturating_sub(1);
            if analyst.wait > 0 {
                continue;
            }
            if let Some(host) = analyst.target.take() {
                analyst.wait = analyst.habit.every();
                events.push((analyst.name.clone(), host, true));
                continue;
            }
            let host = match analyst.habit {
                Habit::Strict => here,
                Habit::Careful => noisiest.unwrap_or(here),
                Habit::Lazy => hosts
                    .get(self.random.below(hosts.len()))
                    .copied()
                    .unwrap_or(here),
            };
            analyst.target = Some(host);
            analyst.wait = READ_TICKS;
            events.push((analyst.name.clone(), host, false));
        }
        let mut shown = Vec::new();
        for (name, host, read) in events {
            if !self.running(player) {
                break;
            }
            shown.extend(self.soc_event(player, &name, host, read));
        }
        shown
    }

    fn soc_event(
        &mut self,
        player: PlayerId,
        name: &str,
        host: HostId,
        read: bool,
    ) -> Vec<Message> {
        let id = self.hosts[host.0].id.clone();
        let line = Message::new(Kind::Telegraph)
            .text("[SOC] Analyst ")
            .span(Style::Player, name);
        if !read {
            return vec![
                line.text(" turns to the logs of ")
                    .span(Style::Host, id)
                    .text("."),
            ];
        }
        let line = line.text(" reads the logs of ").span(Style::Host, id);
        let Some(run) = self
            .players
            .get_mut(&player)
            .and_then(|state| state.run.as_mut())
        else {
            return Vec::new();
        };
        let logged = run.logs.iter().position(|&(known, _)| known == host);
        let found = match logged {
            Some(index) if run.mask == 0 => run.logs.remove(index).1,
            _ => 0,
        };
        if found == 0 {
            return vec![line.text(" and finds nothing.")];
        }
        let gain = FOUND_TRACE + found;
        let before = run.trace;
        let after = (before + gain).min(MAX_TRACE);
        let mut shown = vec![line.text(format!(
            " and finds your trail. Trace: {before}% -> {after}%."
        ))];
        shown.extend(self.raise_trace(player, gain));
        shown
    }
}

#[cfg(test)]
mod tests {
    use super::Habit;
    use crate::testing::{enter_job, hired_at, plain, walk};
    use crate::{PlayerId, World};

    const NOON: u64 = 20_000 * 86_400 + 12 * 3_600;
    const NIGHT: u64 = 20_000 * 86_400 + 2 * 3_600;

    fn staff(world: &World, player: PlayerId) -> Vec<String> {
        let run = world
            .players
            .get(&player)
            .and_then(|state| state.run.as_ref());
        run.map_or_default(|run| {
            run.analysts
                .iter()
                .map(|analyst| analyst.name.clone())
                .collect()
        })
    }

    fn versions(world: &World, player: PlayerId) -> Vec<u32> {
        let run = world
            .players
            .get(&player)
            .and_then(|state| state.run.as_ref());
        run.map_or_default(|run| run.ice.iter().map(|ice| ice.version).collect())
    }

    fn watched(at: u64) -> (World, PlayerId, String, String) {
        let (mut world, player) = hired_at(at);
        enter_job(&mut world, player);
        let here = world.hosts[world.here(player).0].id.clone();
        let Some(state) = world.players.get_mut(&player) else {
            return (world, player, String::new(), here);
        };
        state.rig.cycles = 20;
        let Some(run) = state.run.as_mut() else {
            return (world, player, String::new(), here);
        };
        run.ice.clear();
        for (index, analyst) in run.analysts.iter_mut().enumerate() {
            analyst.habit = Habit::Strict;
            analyst.wait = if index == 0 { 1 } else { 100 };
        }
        let name = run.analysts[0].name.clone();
        (world, player, name, here)
    }

    #[test]
    fn the_day_shift_has_two_analysts_and_the_night_shift_has_one_and_stronger_ice() {
        let (mut day, player) = hired_at(NOON);
        enter_job(&mut day, player);
        let (mut night, other) = hired_at(NIGHT);
        enter_job(&mut night, other);
        assert_eq!(staff(&day, player).len(), 2);
        assert_eq!(staff(&night, other).len(), 1);
        let stronger: Vec<u32> = versions(&day, player)
            .iter()
            .map(|version| version + 1)
            .collect();
        assert_eq!(versions(&night, other), stronger);
    }

    #[test]
    fn each_alert_level_adds_an_analyst() {
        let (mut world, player) = hired_at(NOON);
        let corp = world.corps.iter().position(|corp| corp.id == "acme");
        world.raise_alert(corp.unwrap_or_default(), 60);
        enter_job(&mut world, player);
        assert_eq!(staff(&world, player).len(), 4);
    }

    #[test]
    fn an_analyst_turns_to_a_log_and_then_finds_your_trail() {
        let (mut world, player, name, here) = watched(NOON);
        world.command(player, "scan");
        assert_eq!(
            plain(&world.tick()),
            [format!("[SOC] Analyst {name} turns to the logs of {here}.")]
        );
        assert_eq!(plain(&world.tick()), Vec::<String>::new());
        assert_eq!(
            plain(&world.tick()),
            [format!(
                "[SOC] Analyst {name} reads the logs of {here} and finds your trail. Trace: 2% -> 9%."
            )]
        );
    }

    #[test]
    fn scrub_cleans_the_log_and_mask_hides_you() {
        let (mut world, player, name, here) = watched(NOON);
        for line in ["scan", "scrub"] {
            world.command(player, line);
        }
        for _ in 0..2 {
            world.tick();
        }
        let nothing = format!("[SOC] Analyst {name} reads the logs of {here} and finds nothing.");
        assert_eq!(plain(&world.tick()), std::slice::from_ref(&nothing));
        let (mut world, player, _, _) = watched(NOON);
        if let Some(run) = world
            .players
            .get_mut(&player)
            .and_then(|state| state.run.as_mut())
        {
            run.mask = 5;
        }
        world.command(player, "scan");
        for _ in 0..2 {
            world.tick();
        }
        assert_eq!(plain(&world.tick()), [nothing]);
    }

    #[test]
    fn scan_names_the_analysts_on_shift_and_a_hand_made_network_has_no_soc() {
        let (mut world, player) = hired_at(NOON);
        enter_job(&mut world, player);
        let shown = plain(&world.command(player, "scan"));
        let names: Vec<String> = staff(&world, player);
        let line = shown.iter().find(|line| line.starts_with("SOC on shift: "));
        let line = line.cloned().unwrap_or_default();
        assert!(
            names.iter().all(|name| line.contains(name.as_str())),
            "{shown:?}"
        );
        assert!(line.ends_with(").") && !names.is_empty(), "{line}");
        let lab = walk(&["go back-room", "go gate", "scan"]).0;
        assert!(!lab.iter().any(|line| line.contains("SOC")), "{lab:?}");
        let (mut world, player) = crate::testing::joined();
        for line in ["go back-room", "go gate"] {
            world.command(player, line);
        }
        assert_eq!(staff(&world, player), Vec::<String>::new());
    }
}
