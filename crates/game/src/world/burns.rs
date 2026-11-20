use ttymud_text::{Message, Style};

use super::account::{Phase, RULE};
use super::rig::Rig;
use super::{PlayerId, World, presence};
use crate::content::File;
use crate::date::date;

const BURN_LIST: &str = "burn-list.txt";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Burn {
    pub(super) name: String,
    pub(super) when: u64,
}

impl World {
    pub fn set_time(&mut self, now: u64) {
        self.now = now;
    }

    pub(super) fn is_burned(&self, name: &str) -> bool {
        self.burns
            .iter()
            .any(|burn| burn.name.eq_ignore_ascii_case(name))
    }

    pub(super) fn burn(&mut self, player: PlayerId) -> Vec<Message> {
        let (here, start, now) = (self.here(player), self.start, self.now);
        let Some(state) = self.players.get_mut(&player) else {
            return Vec::new();
        };
        let name = std::mem::take(&mut state.name);
        state.phase = Phase::Naming;
        state.rig = Rig::new();
        state.heat = 0;
        state.credits = 0;
        state.rep.clear();
        state.run = None;
        state.end_contract();
        state.kit = None;
        state.chain = vec![start];
        let (know, stash) = (state.know.clone(), state.stash.clone());
        if let Some(account) = state.account.clone() {
            let entry = self.accounts.entry(account).or_default();
            entry.handle = None;
            entry.know = know;
            entry.stash = stash;
        }
        self.burns.push(Burn {
            name: name.clone(),
            when: now,
        });
        self.burns_dirty = true;
        self.touch(player);
        self.post_burns();
        self.quit_crew(player, &name, "burns and leaves");
        self.publish(format!(
            "The handle {name} burns. The police seize its rig."
        ));
        let taken = presence::named(&name, " burns. The police take the rig away.");
        self.notify(here, player, &taken);
        vec![
            Message::game("Your heat is 5. The police know who you are."),
            Message::game("Your handle ")
                .span(Style::Player, &name)
                .text(" burns. The police seize your rig and its files."),
            Message::game("Your account keeps its know-how and its stash."),
            Message::game("Choose a new handle: the name that other players see."),
            Message::game(RULE),
        ]
    }

    pub(super) fn post_burns(&mut self) {
        let Some(host) = self.burns_host else {
            return;
        };
        let mut lines: Vec<String> = self
            .burns
            .iter()
            .map(|burn| format!("{} burned on {}.", burn.name, date(burn.when)))
            .collect();
        if lines.is_empty() {
            lines.push("Nobody is on the burn list yet.".to_owned());
        }
        self.post_file(host, File::new(BURN_LIST, lines));
    }
}

#[cfg(test)]
mod tests {
    use crate::testing::{heard, plain, walk, world};
    use crate::{Content, PlayerId, World};

    fn hot(kit: &str) -> (World, PlayerId) {
        let mut world = world();
        world.set_time(1_700_000_000);
        let (player, _) = world.connect("a");
        world.command(player, "nyx");
        world.command(player, kit);
        for line in ["go back-room", "go gate", "go core"] {
            world.command(player, line);
        }
        if let Some(state) = world.players.get_mut(&player) {
            state.heat = 4;
            state.cooled = 1_700_000_000;
            if let Some(run) = state.run.as_mut() {
                run.trace = 98;
            }
        }
        (world, player)
    }

    fn burned() -> (World, PlayerId) {
        let (mut world, player) = hot("ghost");
        world.command(player, "scan");
        (world, player)
    }

    #[test]
    fn the_burn_list_is_a_file_on_its_host() {
        assert_eq!(
            walk(&["go bazaar", "read burn-list.txt"]).0,
            ["Nobody is on the burn list yet."]
        );
    }

    #[test]
    fn heat_five_burns_the_handle() {
        let (mut world, player) = hot("ghost");
        let shown = plain(&world.command(player, "scan"));
        assert_eq!(
            shown[2..],
            [
                "Your heat: 4 -> 5.",
                "Your heat is 5. The police know who you are.",
                "Your handle nyx burns. The police seize your rig and its files.",
                "Your account keeps its know-how and its stash.",
                "Choose a new handle: the name that other players see.",
                "A handle has 3 to 16 letters, digits, hyphens, or underscores.",
            ]
        );
        assert_eq!(world.prompt(player).as_deref(), Some("handle: "));
        assert_eq!(world.handle(player), None);
    }

    #[test]
    fn a_burn_at_home_is_private() {
        let (mut world, player) = hot("ghost");
        let (ghost, _) = world.join("ghost");
        world.command(ghost, "go back-room");
        assert_eq!(
            heard(&world.command(player, "scan"), ghost),
            Vec::<String>::new()
        );
    }

    #[test]
    fn others_on_a_shared_hub_see_a_burn() {
        let city = "[city c]\nname = C\nstart = hub\n[host hub]\nname = Hub\ndesc = A hub.\n\
                    [network n]\nname = N\n[host gate]\nname = Gate\ndesc = A gate.\n\
                    network = n\nroute = hub\n";
        let mut world = World::new(Content::parse(city).unwrap(), 0);
        let (ghost, _) = world.join("ghost");
        let (player, _) = world.join("nyx");
        world.command(player, "go gate");
        if let Some(state) = world.players.get_mut(&player) {
            state.heat = 4;
            if let Some(run) = state.run.as_mut() {
                run.trace = 98;
            }
        }
        assert_eq!(
            heard(&world.command(player, "scan"), ghost),
            ["nyx arrives.", "nyx burns. The police take the rig away."]
        );
    }

    #[test]
    fn the_burn_list_names_the_burned_handle() {
        let (mut world, player) = burned();
        for line in ["zer0", "breaker", "go bazaar"] {
            world.command(player, line);
        }
        assert_eq!(
            plain(&world.command(player, "read burn-list.txt")),
            ["nyx burned on 2023-11-14."]
        );
        assert_eq!(
            plain(&world.command(player, "news")),
            ["2023-11-14 22:13 The handle nyx burns. The police seize its rig."]
        );
    }

    #[test]
    fn a_burned_name_cannot_be_used_again() {
        let (mut world, player) = burned();
        assert_eq!(
            plain(&world.command(player, "NYX")),
            ["The handle NYX burned. Nobody can use it again."]
        );
    }

    #[test]
    fn a_new_handle_keeps_the_know_how_of_the_account() {
        let (mut world, player) = burned();
        for line in ["zer0", "face", "go back-room", "unload patch"] {
            world.command(player, line);
        }
        assert_eq!(
            plain(&world.command(player, "load mask")),
            ["You load mask. RAM: 8/8 GB."]
        );
    }

    #[test]
    fn a_burn_takes_the_credits_and_the_reputation() {
        let (mut world, player) = hot("ghost");
        if let Some(state) = world.players.get_mut(&player) {
            state.credits = 500;
            state.rep.insert("pell".to_owned(), 3);
        }
        for line in ["scan", "zer0", "breaker", "go relay", "go bazaar"] {
            world.command(player, line);
        }
        assert_eq!(
            plain(&world.command(player, "status | grep credits")),
            ["Credits: 0 cr"]
        );
        assert_eq!(
            plain(&world.command(player, "jobs | grep reputation:")),
            ["Your reputation: Pell 0"]
        );
    }

    #[test]
    fn a_burn_ends_the_handle_of_the_account() {
        let (mut world, player) = burned();
        world.leave(player);
        let (player, _) = world.connect("a");
        assert_eq!(world.prompt(player).as_deref(), Some("handle: "));
    }
}
