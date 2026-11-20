use ttymud_text::{Kind, Message, Style};

use super::instance::{InstanceId, tell};
use super::run::MAX_TRACE;
use super::{HostId, NetworkId, Output, PlayerId, Step, World, address};
use crate::random::{Random, mix};
use crate::target::Level;

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

    pub(super) fn word(self) -> &'static str {
        match self {
            Self::Careful => "careful",
            Self::Strict => "strict",
            Self::Lazy => "lazy",
        }
    }

    pub(super) fn other(self, lie: usize) -> Self {
        let index = HABITS.iter().position(|&habit| habit == self);
        HABITS[(index.unwrap_or(0) + 1 + lie % 2) % 3]
    }
}

#[derive(Debug, Clone)]
pub(super) struct Analyst {
    pub(super) name: String,
    pub(super) habit: Habit,
    pub(super) wait: u32,
    pub(super) target: Option<HostId>,
    pub(super) fooled: bool,
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
        let extra = usize::try_from(self.alert(corp).steps()).unwrap_or(0);
        self.shift(corp, day, extra)
            .into_iter()
            .zip(0..)
            .map(|((name, habit), index)| Analyst {
                name,
                habit,
                wait: habit.every() + 2 * index,
                target: None,
                fooled: false,
            })
            .collect()
    }

    pub(super) fn on_shift(&self, corp: usize, day: bool) -> Vec<(String, Habit)> {
        self.shift(corp, day, 0)
    }

    fn shift(&self, corp: usize, day: bool, extra: usize) -> Vec<(String, Habit)> {
        let mut people = pool(self.seed, corp);
        if !day {
            people.reverse();
        }
        people.truncate(extra + if day { DAY_STAFF } else { NIGHT_STAFF });
        people
    }

    pub(super) fn analyst_names(&self, player: PlayerId) -> Vec<&str> {
        self.instance(player)
            .into_iter()
            .flat_map(|instance| &instance.analysts)
            .map(|analyst| analyst.name.as_str())
            .collect()
    }

    pub(super) fn fool(&mut self, player: PlayerId, word: &str) -> Option<Result<Step, Message>> {
        let instance = self.instance_mut(player)?;
        let analyst = instance
            .analysts
            .iter_mut()
            .find(|analyst| analyst.name.eq_ignore_ascii_case(word))?;
        let named = |rest: &str| {
            Message::error("")
                .span(Style::Player, &analyst.name)
                .text(rest.to_owned())
        };
        if analyst.habit == Habit::Strict {
            return Some(Err(named(" is strict and does not believe you.")));
        }
        if analyst.fooled {
            return Some(Err(named(" already believes you.")));
        }
        (analyst.fooled, analyst.target) = (true, None);
        let fooled = Message::game("You call the SOC as the help desk. ")
            .span(Style::Player, &analyst.name)
            .text(" believes you and stops watching this run.");
        Some(Ok(Step::show(vec![fooled])))
    }

    pub(super) fn night_bonus(&self, network: NetworkId) -> u32 {
        u32::from(self.networks[network.0].corp.is_some() && !day_shift(self.now))
    }

    pub(super) fn soc_line(&self, player: PlayerId) -> Option<Message> {
        let instance = self.instance(player)?;
        let (first, rest) = instance.analysts.split_first()?;
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

    pub(super) fn tick_soc(&mut self, id: InstanceId) -> Vec<Output> {
        let members = self.members(id);
        let suspect = members.iter().rev().max_by_key(|member| member.trace);
        let Some(here) = suspect.map(|member| member.host) else {
            return Vec::new();
        };
        let Some(instance) = self.instances.get_mut(&id) else {
            return Vec::new();
        };
        let network = instance.network;
        let hosts: Vec<HostId> = (0..self.hosts.len())
            .filter(|&index| self.hosts[index].network == Some(network))
            .map(HostId)
            .collect();
        let mut totals: Vec<(HostId, u32)> = Vec::new();
        for &(host, _, noise) in &instance.logs {
            match totals.iter_mut().find(|(known, _)| *known == host) {
                Some((_, total)) => *total += noise,
                None => totals.push((host, noise)),
            }
        }
        let noisiest = totals
            .iter()
            .max_by_key(|&&(_, noise)| noise)
            .map(|&(host, _)| host);
        let mut events = Vec::new();
        for analyst in instance
            .analysts
            .iter_mut()
            .filter(|analyst| !analyst.fooled)
        {
            analyst.wait = analyst.wait.saturating_sub(1);
            if analyst.wait > 0 {
                continue;
            }
            if let Some(host) = analyst.target.take() {
                analyst.wait = analyst.habit.every();
                events.push((analyst.name.clone(), analyst.habit, host, true));
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
            events.push((analyst.name.clone(), analyst.habit, host, false));
        }
        let mut outputs = Vec::new();
        for (name, habit, host, read) in events {
            if self.members(id).is_empty() {
                break;
            }
            outputs.extend(self.soc_event(id, (&name, habit), host, read));
        }
        outputs
    }

    fn soc_event(
        &mut self,
        id: InstanceId,
        (name, habit): (&str, Habit),
        host: HostId,
        read: bool,
    ) -> Vec<Output> {
        let members = self.members(id);
        let everyone: Vec<PlayerId> = members.iter().map(|member| member.player).collect();
        let place = self.hosts[host.0].id.clone();
        let line = || {
            Message::new(Kind::Telegraph)
                .text("[SOC] Analyst ")
                .span(Style::Player, name)
        };
        if !read {
            let turns = line()
                .text(" turns to the logs of ")
                .span(Style::Host, &place)
                .text(".");
            return tell(&everyone, &turns);
        }
        let reads = || line().text(" reads the logs of ").span(Style::Host, &place);
        let mut found = Vec::new();
        for member in &members {
            let masked = self
                .players
                .get(&member.player)
                .and_then(|state| state.run.as_ref())
                .is_some_and(|run| run.mask > 0);
            let Some(instance) = self.instances.get_mut(&id) else {
                break;
            };
            let mine =
                |&(at, owner, _): &(HostId, PlayerId, u32)| at == host && owner == member.player;
            let noise: u32 = instance
                .logs
                .iter()
                .filter(|&entry| mine(entry))
                .map(|&(_, _, noise)| noise)
                .sum();
            if masked || noise == 0 {
                continue;
            }
            instance.logs.retain(|entry| !mine(entry));
            found.push((member.player, member.name.clone(), noise));
        }
        if found.is_empty() {
            return tell(&everyone, &reads().text(" and finds nothing."));
        }
        let mut outputs = Vec::new();
        for (player, who, noise) in found {
            let gain = FOUND_TRACE + noise;
            let others: Vec<PlayerId> = everyone
                .iter()
                .copied()
                .filter(|&other| other != player)
                .collect();
            let trail = reads().text(format!(" and finds the trail of {who}."));
            outputs.extend(tell(&others, &trail));
            let mut shown = Vec::new();
            if let Some(run) = self
                .players
                .get_mut(&player)
                .and_then(|state| state.run.as_mut())
            {
                let before = run.trace;
                let after = (before + gain).min(MAX_TRACE);
                shown.push(reads().text(format!(
                    " and finds your trail. Trace: {before}% -> {after}%."
                )));
                if habit != Habit::Lazy && run.access > Level::Guest {
                    run.access = Level::Guest;
                    let network = &self.networks[run.network.0].name;
                    shown.push(line().text(format!(
                        " locks your stolen account. Your access in {network}: guest."
                    )));
                }
            }
            shown.extend(self.raise_trace(player, gain));
            outputs.extend(address(player, shown));
        }
        outputs
    }
}

#[cfg(test)]
mod tests {
    use super::Habit;
    use crate::commands::Verb;
    use crate::target::Level;
    use crate::testing::{enter_job, hired_at, plain, walk};
    use crate::{PlayerId, World};

    const NOON: u64 = 20_000 * 86_400 + 12 * 3_600;
    const NIGHT: u64 = 20_000 * 86_400 + 2 * 3_600;

    fn staff(world: &World, player: PlayerId) -> Vec<String> {
        world.instance(player).map_or_default(|instance| {
            instance
                .analysts
                .iter()
                .map(|analyst| analyst.name.clone())
                .collect()
        })
    }

    fn versions(world: &World, player: PlayerId) -> Vec<u32> {
        world
            .instance(player)
            .map_or_default(|instance| instance.ice.iter().map(|ice| ice.version).collect())
    }

    fn watched(at: u64) -> (World, PlayerId, String, String) {
        let (mut world, player) = hired_at(at);
        enter_job(&mut world, player);
        let here = world.hosts[world.here(player).0].id.clone();
        if let Some(state) = world.players.get_mut(&player) {
            state.rig.cycles = 20;
        }
        let Some(instance) = world.instance_mut(player) else {
            return (world, player, String::new(), here);
        };
        instance.ice.clear();
        for (index, analyst) in instance.analysts.iter_mut().enumerate() {
            analyst.habit = Habit::Strict;
            analyst.wait = if index == 0 { 1 } else { 100 };
        }
        let name = instance.analysts[0].name.clone();
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

    fn set_up(world: &mut World, player: PlayerId, habits: &[Habit], access: Level) {
        let Some(state) = world.players.get_mut(&player) else {
            return;
        };
        state.rig.loads.push(Verb::Pretext);
        state.rig.cycles = 20;
        if let Some(run) = state.run.as_mut() {
            run.access = access;
        }
        let Some(instance) = world.instance_mut(player) else {
            return;
        };
        for (analyst, &habit) in instance.analysts.iter_mut().zip(habits) {
            analyst.habit = habit;
        }
    }

    #[test]
    fn a_careful_or_strict_analyst_locks_your_stolen_account() {
        let (mut world, player, name, here) = watched(NOON);
        set_up(&mut world, player, &[Habit::Strict], Level::User);
        world.command(player, "scan");
        world.tick();
        world.tick();
        assert_eq!(
            plain(&world.tick()),
            [
                format!(
                    "[SOC] Analyst {name} reads the logs of {here} and finds your trail. Trace: 2% -> 9%."
                ),
                format!(
                    "[SOC] Analyst {name} locks your stolen account. Your access in Acme Corp: guest."
                ),
            ]
        );
    }

    #[test]
    fn a_lazy_analyst_does_not_lock_your_account() {
        let (mut world, player, name, here) = watched(NOON);
        set_up(&mut world, player, &[Habit::Lazy], Level::User);
        world.command(player, "scan");
        let host = world.here(player);
        if let Some(instance) = world.instance_mut(player) {
            instance.analysts[0].target = Some(host);
        }
        assert_eq!(
            plain(&world.tick()),
            [format!(
                "[SOC] Analyst {name} reads the logs of {here} and finds your trail. Trace: 2% -> 9%."
            )]
        );
        let access = world
            .players
            .get(&player)
            .and_then(|state| state.run.as_ref());
        assert_eq!(access.map(|run| run.access), Some(Level::User));
    }

    #[test]
    fn pretext_fools_a_careful_analyst_but_not_a_strict_one() {
        let (mut world, player) = hired_at(NOON);
        enter_job(&mut world, player);
        set_up(
            &mut world,
            player,
            &[Habit::Careful, Habit::Strict],
            Level::Guest,
        );
        let names = staff(&world, player);
        let (careful, strict) = (names[0].clone(), names[1].clone());
        assert_eq!(
            world.complete(player, "pretext ").names,
            [careful.clone(), strict.clone()]
        );
        assert_eq!(
            plain(&world.command(player, &format!("pretext {}", careful.to_uppercase())))[0],
            format!(
                "You call the SOC as the help desk. {careful} believes you and stops watching this run."
            )
        );
        assert_eq!(
            plain(&world.command(player, &format!("pretext {strict}"))),
            [format!("{strict} is strict and does not believe you.")]
        );
        if let Some(instance) = world.instance_mut(player) {
            instance.ice.clear();
            instance.analysts[0].wait = 1;
            instance.analysts[1].wait = 100;
        }
        assert_eq!(plain(&world.tick()), Vec::<String>::new());
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
