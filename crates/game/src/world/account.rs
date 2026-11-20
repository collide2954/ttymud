use std::collections::BTreeMap;

use ttymud_text::{Kind, Message, Style};

use super::builds::Build;
use super::contract::Job;
use super::{Output, Player, PlayerId, World, address, listed, presence};
use crate::commands::{self, Verb};
use crate::content::File;
use crate::kits::{self, KITS, Kit};

pub(super) const RULE: &str = "A handle has 3 to 16 letters, digits, hyphens, or underscores.";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Phase {
    Naming,
    Kit(String),
    Playing,
}

#[derive(Debug, Clone, Default)]
pub(super) struct Account {
    pub(super) handle: Option<Saved>,
    pub(super) know: Vec<Verb>,
    pub(super) blueprints: Vec<(Verb, u32)>,
    pub(super) stash: Vec<File>,
    pub(super) social: Social,
}

#[derive(Debug, Clone, Default)]
pub(super) struct Social {
    pub(super) created: u64,
    pub(super) warned: bool,
    pub(super) muted: u64,
    pub(super) ignores: Vec<String>,
    pub(super) blocks: Vec<String>,
}

#[derive(Debug, Clone)]
pub(super) struct Saved {
    pub(super) name: String,
    pub(super) kit: &'static Kit,
    pub(super) heat: u32,
    pub(super) cooled: u64,
    pub(super) tipped: Option<u64>,
    pub(super) integrity: u32,
    pub(super) credits: u32,
    pub(super) ram: u32,
    pub(super) cpu: u32,
    pub(super) sold: Vec<String>,
    pub(super) rep: BTreeMap<String, u32>,
    pub(super) done: Vec<(u64, String, u64)>,
    pub(super) posting: Option<(String, u64, u64)>,
    pub(super) channels: Option<Vec<String>>,
    pub(super) files: Vec<File>,
    pub(super) loads: Vec<Verb>,
    pub(super) builds: Vec<(Verb, Build)>,
    pub(super) contract: Option<Job>,
}

impl World {
    pub fn connect(&mut self, account: &str) -> (PlayerId, Vec<Output>) {
        let known = self.accounts.get(account).cloned();
        let Some((saved, known)) = known.and_then(|known| Some((known.handle.clone()?, known)))
        else {
            return self.begin(account);
        };
        let (know, stash) = (known.know, known.stash);
        let home = self.home.unwrap_or(self.start);
        let mut state = Player::new(&saved.name, home);
        state.account = Some(account.to_owned());
        state.social = known.social;
        state.kit = Some(saved.kit);
        state.heat = saved.heat;
        state.cooled = saved.cooled;
        state.tipped = saved.tipped;
        state.rig.integrity = saved.integrity;
        state.credits = saved.credits;
        state.rig.ram = saved.ram;
        state.rig.cpu = saved.cpu;
        state.rig.cycles = saved.cpu;
        state.sold = saved.sold;
        state.rep = saved.rep;
        state.done = saved.done;
        state.channels = match saved.channels {
            Some(channels) => channels
                .into_iter()
                .filter(|id| self.channels.iter().any(|channel| &channel.id == id))
                .collect(),
            None => self.auto_channels(),
        };
        state.rig.files = saved.files;
        state.rig.loads = saved.loads;
        state.rig.builds = saved.builds;
        state.contract = saved.contract.or_else(|| {
            let (fixer, day, slot) = saved.posting.as_ref()?;
            self.resume_posting(fixer, *day, *slot).map(Job::new)
        });
        state.know = know;
        learn(&mut state.know, saved.kit.knows);
        state.blueprints = known.blueprints;
        state.stash = stash;
        let player = self.add(state);
        let name = &saved.name;
        self.notify(home, player, &presence::named(name, " connects."));
        let mut shown = vec![
            Message::game("Welcome back, ")
                .span(Style::Player, name)
                .text("."),
        ];
        if let Some((before, after)) = self.cool(player) {
            shown.push(Message::game(format!(
                "Your heat cooled down while you were away: {before} -> {after}."
            )));
        }
        shown.extend(self.scene(home, player));
        (player, self.deliver(player, shown))
    }

    fn begin(&mut self, account: &str) -> (PlayerId, Vec<Output>) {
        let mut state = Player::new("", self.start);
        state.account = Some(account.to_owned());
        state.phase = Phase::Naming;
        if let Some(known) = self.accounts.get(account) {
            state.know.clone_from(&known.know);
            state.blueprints.clone_from(&known.blueprints);
            state.stash.clone_from(&known.stash);
            state.social.clone_from(&known.social);
        } else {
            state.know.clear();
            state.social.created = self.now;
        }
        let player = self.add(state);
        let city = &self.city;
        let shown = vec![
            Message::game(format!(
                "You are new in {city}. Choose a handle: the name that other players see."
            )),
            Message::game(RULE),
        ];
        (player, address(player, shown))
    }

    pub(super) fn create(&mut self, player: PlayerId, line: &str) -> Vec<Message> {
        let phase = self.players.get(&player).map(|state| state.phase.clone());
        match phase {
            Some(Phase::Naming) => {
                if let Err(message) = self.check_handle(line) {
                    return vec![message];
                }
                if let Some(state) = self.players.get_mut(&player) {
                    state.phase = Phase::Kit(line.to_owned());
                }
                menu()
            }
            Some(Phase::Kit(name)) => match kits::find(line) {
                Some(kit) => self.finish(player, &name, kit),
                None => vec![Message::error("Type breaker, ghost, phreak, or face.")],
            },
            _ => Vec::new(),
        }
    }

    fn check_handle(&self, name: &str) -> Result<(), Message> {
        if !good_name(name) {
            return Err(Message::error(RULE));
        }
        if name.to_ascii_lowercase().starts_with("anon-") {
            return Err(Message::error(
                "Names that start with anon- are for players without a handle.",
            ));
        }
        if self.is_burned(name) {
            return Err(Message::error(format!(
                "The handle {name} burned. Nobody can use it again."
            )));
        }
        let saved = self
            .accounts
            .values()
            .filter_map(|account| account.handle.as_ref())
            .any(|handle| handle.name.eq_ignore_ascii_case(name));
        let online = self
            .players
            .values()
            .any(|state| state.phase == Phase::Playing && state.name.eq_ignore_ascii_case(name));
        if saved || online {
            return Err(Message::error(format!("The handle {name} is taken.")));
        }
        Ok(())
    }

    fn finish(&mut self, player: PlayerId, name: &str, kit: &'static Kit) -> Vec<Message> {
        let (start, offer, channels) = (self.start, self.offer, self.auto_channels());
        let Some(state) = self.players.get_mut(&player) else {
            return Vec::new();
        };
        name.clone_into(&mut state.name);
        state.kit = Some(kit);
        state.phase = Phase::Playing;
        state.channels = channels;
        state.rig.loads = kit.loads.to_vec();
        learn(&mut state.know, &kit.loads);
        learn(&mut state.know, kit.knows);
        state.contract = offer.map(Job::new);
        state.chain = vec![start];
        self.keep(player);
        self.notify(start, player, &presence::named(name, " connects."));
        let programs = kit.loads.iter().map(|&verb| commands::name_of(verb));
        let intro = Message::game("You are ")
            .span(Style::Player, name)
            .text(format!(", a {}. Programs in RAM: ", kit.id));
        let mut shown = vec![listed(intro, Style::Command, programs)];
        shown.extend(self.scene(start, player));
        shown.extend(self.offer(player));
        shown
    }

    pub(super) fn keep(&mut self, player: PlayerId) {
        let Some(state) = self.players.get(&player) else {
            return;
        };
        let (Some(account), Some(kit), Phase::Playing) = (&state.account, state.kit, &state.phase)
        else {
            return;
        };
        let (integrity, files) = match self.saved_rig(player) {
            Some(fighter) => (fighter.integrity, &fighter.files),
            None => (state.rig.integrity, &state.rig.files),
        };
        let saved = Saved {
            name: state.name.clone(),
            kit,
            heat: state.heat,
            cooled: state.cooled,
            tipped: state.tipped,
            integrity,
            credits: state.credits,
            ram: state.rig.ram,
            cpu: state.rig.cpu,
            sold: state.sold.clone(),
            rep: state.rep.clone(),
            done: state.done.clone(),
            posting: None,
            channels: Some(state.channels.clone()),
            files: files.clone(),
            loads: state.rig.loads.clone(),
            builds: state.rig.builds.clone(),
            contract: state.contract,
        };
        let (know, blueprints, stash, social) = (
            state.know.clone(),
            state.blueprints.clone(),
            state.stash.clone(),
            state.social.clone(),
        );
        let entry = self.accounts.entry(account.clone()).or_default();
        entry.handle = Some(saved);
        entry.know = know;
        entry.blueprints = blueprints;
        entry.stash = stash;
        entry.social = social;
    }

    #[must_use]
    pub fn account_count(&self) -> usize {
        self.accounts.len()
    }

    pub fn delete_account(&mut self, word: &str) -> Result<(String, String), String> {
        let id = match self.saved_handle(word) {
            Some((id, _)) => id,
            None if self.accounts.contains_key(word) => word.to_owned(),
            None => return Err(format!("No handle named {word}.")),
        };
        let handle = self.accounts.remove(&id).and_then(|account| account.handle);
        self.dirty.remove(&id);
        self.moderation.forget_moderator(&id);
        let name = handle.map_or_else(|| id.clone(), |saved| saved.name);
        self.forget_member(&name);
        Ok((id, name))
    }

    #[must_use]
    pub fn handle(&self, player: PlayerId) -> Option<&str> {
        let state = self.players.get(&player)?;
        (state.phase == Phase::Playing).then_some(state.name.as_str())
    }
}

fn learn(know: &mut Vec<Verb>, verbs: &[Verb]) {
    for &verb in verbs {
        if !know.contains(&verb) {
            know.push(verb);
        }
    }
}

pub(super) fn good_name(name: &str) -> bool {
    (3..=16).contains(&name.len())
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

fn menu() -> Vec<Message> {
    let mut shown = vec![Message::game("Choose a starter kit:")];
    shown.extend(KITS.iter().map(|kit| {
        let line = Message::new(Kind::Game)
            .span(Style::Command, kit.id)
            .text(format!(": {} Programs: ", kit.summary));
        let programs = kit.loads.iter().map(|&verb| commands::name_of(verb));
        let line = listed(line, Style::Command, programs);
        if kit.knows.is_empty() {
            return line;
        }
        let extra = kit.knows.iter().map(|&verb| commands::name_of(verb));
        listed(line.text(". It also knows "), Style::Command, extra).text(".")
    }));
    shown
}

#[cfg(test)]
mod tests {
    use crate::testing::{heard, plain, world};
    use crate::{PlayerId, World};

    fn hacker(world: &mut World, account: &str, name: &str, kit: &str) -> PlayerId {
        let (player, _) = world.connect(account);
        world.command(player, name);
        world.command(player, kit);
        player
    }

    #[test]
    fn delete_account_removes_the_account_and_its_moderator_and_crew_places() {
        let mut world = world();
        world
            .restore("moderators.ini", "[moderator a]\nhandle = nyx\n")
            .unwrap();
        let nyx = hacker(&mut world, "a", "nyx", "ghost");
        let zer0 = hacker(&mut world, "b", "zer0", "ghost");
        world.command(nyx, "crew create Ghostwire");
        world.command(nyx, "crew invite zer0");
        world.command(zer0, "crew join ghostwire");
        let _ = world.leave(nyx);
        let _ = world.leave(zer0);
        let _ = world.saves();
        assert_eq!(
            world.delete_account("NYX"),
            Ok(("a".to_owned(), "nyx".to_owned()))
        );
        assert_eq!(world.account_count(), 1);
        assert_eq!(world.moderators(), Vec::<String>::new());
        let crews = world
            .saves()
            .into_iter()
            .find(|(path, _)| path == "crews.ini");
        let crews = crews.map_or_default(|(_, text)| text);
        assert!(crews.contains("leader = zer0\n"), "{crews}");
        assert!(!crews.contains("nyx"), "{crews}");
        assert_eq!(
            world.delete_account("nyx"),
            Err("No handle named nyx.".to_owned())
        );
        assert_eq!(
            world.delete_account("b"),
            Ok(("b".to_owned(), "zer0".to_owned()))
        );
        assert_eq!(world.account_count(), 0);
    }

    #[test]
    fn a_new_account_chooses_a_handle_and_a_kit() {
        let mut world = world();
        let (player, outputs) = world.connect("a");
        assert_eq!(
            plain(&outputs),
            [
                "You are new in Test City. Choose a handle: the name that other players see.",
                "A handle has 3 to 16 letters, digits, hyphens, or underscores.",
            ]
        );
        assert_eq!(world.prompt(player).as_deref(), Some("handle: "));
        let shown = plain(&world.command(player, "nyx"));
        assert_eq!(shown[0], "Choose a starter kit:");
        assert_eq!(
            shown[2],
            "ghost: Stays hidden from sentries. Programs: scan, probe, crack, scrub, mask"
        );
        assert_eq!(world.prompt(player).as_deref(), Some("kit: "));
        let shown = plain(&world.command(player, "ghost"));
        assert_eq!(
            shown[..2],
            [
                "You are nyx, a ghost. Programs in RAM: scan, probe, crack, scrub, mask",
                "The Relay",
            ]
        );
        assert_eq!(world.prompt(player).as_deref(), Some("nyx@relay $ "));
        assert_eq!(world.handle(player), Some("nyx"));
    }

    #[test]
    fn a_handle_needs_a_good_name() {
        let mut world = world();
        let (player, _) = world.connect("a");
        for bad in ["ab", "a-name-that-is-long", "n y x", "nyx!"] {
            assert_eq!(
                plain(&world.command(player, bad)),
                ["A handle has 3 to 16 letters, digits, hyphens, or underscores."],
                "{bad}"
            );
        }
        assert_eq!(
            plain(&world.command(player, "anon-1234")),
            ["Names that start with anon- are for players without a handle."]
        );
        assert_eq!(world.prompt(player).as_deref(), Some("handle: "));
    }

    #[test]
    fn a_handle_name_is_unique_without_case() {
        let mut world = world();
        hacker(&mut world, "a", "nyx", "ghost");
        let (player, _) = world.connect("b");
        assert_eq!(
            plain(&world.command(player, "NYX")),
            ["The handle NYX is taken."]
        );
    }

    #[test]
    fn a_kit_takes_a_unique_prefix() {
        let mut world = world();
        let (player, _) = world.connect("a");
        world.command(player, "nyx");
        assert_eq!(
            plain(&world.command(player, "wizard")),
            ["Type breaker, ghost, phreak, or face."]
        );
        assert_eq!(
            plain(&world.command(player, "PH"))[0],
            "You are nyx, a phreak. Programs in RAM: scan, probe, crack, scrub, proxy"
        );
    }

    #[test]
    fn the_kit_sets_the_ram_and_the_know_how() {
        let mut world = world();
        let player = hacker(&mut world, "a", "nyx", "face");
        world.command(player, "go back-room");
        assert_eq!(
            plain(&world.command(player, "load mask")),
            ["No program named mask. Programs: scan, probe, crack, patch, pretext"]
        );
    }

    #[test]
    fn a_breaker_also_knows_decoy_and_so_does_an_older_breaker() {
        let mut world = world();
        let player = hacker(&mut world, "a", "nyx", "breaker");
        world.command(player, "go back-room");
        world.command(player, "unload patch");
        assert_eq!(
            plain(&world.command(player, "load decoy")),
            ["You load decoy. RAM: 8/8 GB."]
        );
        let text = "[account b]\nknow = scan\n[handle zer0]\nkit = breaker\nheat = 0\ncooled = 0\nintegrity = 100\n";
        world.restore("accounts/b.ini", text).unwrap();
        let (older, _) = world.connect("b");
        let knows = world
            .players
            .get(&older)
            .is_some_and(|state| state.know.contains(&crate::commands::Verb::Decoy));
        assert!(knows);
    }

    #[test]
    fn others_see_a_new_handle_only_when_it_is_ready() {
        let mut world = world();
        let (ghost, _) = world.join("ghost");
        let (player, outputs) = world.connect("a");
        assert_eq!(heard(&outputs, ghost), Vec::<String>::new());
        assert_eq!(
            heard(&world.command(player, "nyx"), ghost),
            Vec::<String>::new()
        );
        assert_eq!(
            heard(&world.command(player, "ghost"), ghost),
            ["nyx connects."]
        );
    }

    #[test]
    fn a_disconnect_during_a_run_counts_as_a_jack_out() {
        let mut world = world();
        let player = hacker(&mut world, "a", "nyx", "ghost");
        for line in ["go back-room", "go gate"] {
            world.command(player, line);
        }
        if let Some(run) = world
            .players
            .get_mut(&player)
            .and_then(|state| state.run.as_mut())
        {
            run.trace = 80;
        }
        world.leave(player);
        let (player, _) = world.connect("a");
        let status = plain(&world.command(player, "status"));
        assert_eq!(
            status[..3],
            ["Heat: 1/5", "Credits: 0 cr", "Integrity: 80/100"]
        );
    }

    #[test]
    fn a_returning_account_starts_at_home_with_its_handle() {
        let mut world = world();
        let player = hacker(&mut world, "a", "nyx", "breaker");
        world.command(player, "get motd.txt");
        world.leave(player);
        let (player, outputs) = world.connect("a");
        assert_eq!(
            plain(&outputs)[..2],
            ["Welcome back, nyx.", "The Back Room"]
        );
        assert_eq!(world.handle(player), Some("nyx"));
        assert_eq!(plain(&world.command(player, "inventory")), ["motd.txt"]);
    }
}
