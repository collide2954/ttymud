use ttymud_text::Message;

use super::jobs::DAY;
use super::{PlayerId, World};
use crate::heat::Security;

fn limits(budget: Security) -> (u32, u64) {
    match budget {
        Security::Low => (12, 2),
        Security::Medium => (8, 4),
        Security::High => (5, 7),
        Security::Top => (3, 14),
    }
}

impl World {
    pub(super) fn patched(&self, corp: usize, software: &str) -> bool {
        self.corp_states
            .get(corp)
            .and_then(|state| state.patched.get(software))
            .is_some_and(|&until| until > self.now)
    }

    pub(super) fn expose(&mut self, corp: usize, software: &str) -> Option<Message> {
        let (limit, days) = limits(self.corps.get(corp)?.budget);
        let state = self.corp_states.get_mut(corp)?;
        self.state_dirty = true;
        let used = state.used.entry(software.to_owned()).or_default();
        *used += 1;
        if *used < limit {
            return None;
        }
        state.used.remove(software);
        state
            .patched
            .insert(software.to_owned(), self.now + days * DAY);
        let name = &self.corps[corp].name;
        Some(Message::game(format!(
            "{name} finds the hole you used and patches {software}."
        )))
    }

    pub(super) fn exposed(&self, player: PlayerId) -> Option<(usize, String)> {
        let software = self.hosts[self.here(player).0].software.clone()?;
        Some((self.run_corp(player)?, software))
    }

    pub(super) fn refuse_patched(&self, player: PlayerId) -> Result<(), Message> {
        match self.exposed(player) {
            Some((corp, software)) if self.patched(corp, &software) => {
                let name = &self.corps[corp].name;
                Err(Message::error(format!(
                    "{name} patched the hole in {software}. exploit does not work on it until somebody finds a new hole."
                )))
            }
            _ => Ok(()),
        }
    }

    pub(super) fn software_line(&self, player: PlayerId) -> Option<Message> {
        let software = self.hosts[self.here(player).0].software.as_ref()?;
        let line = Message::game(format!("Software: {software}"));
        let patched = self
            .run_corp(player)
            .is_some_and(|corp| self.patched(corp, software));
        Some(if patched {
            line.text(" (patched)")
        } else {
            line
        })
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::DAY;
    use crate::commands::Verb;
    use crate::testing::{enter_job, hired_at, plain};
    use crate::{PlayerId, World};

    const NOON: u64 = 20_000 * 86_400 + 12 * 3_600;

    fn armed() -> (World, PlayerId, usize, String) {
        let (mut world, player) = hired_at(NOON);
        enter_job(&mut world, player);
        if let Some(state) = world.players.get_mut(&player) {
            state.rig.loads.push(Verb::Exploit);
            state.rig.cycles = 20;
            if let Some(run) = state.run.as_mut() {
                run.ice.clear();
                run.analysts.clear();
            }
        }
        let corp = world.run_corp(player).unwrap_or_default();
        let here = world.here(player);
        let software = world.hosts[here.0].software.clone().unwrap_or_default();
        (world, player, corp, software)
    }

    fn corp(world: &World, id: &str) -> usize {
        let found = world.corps.iter().position(|corp| corp.id == id);
        found.unwrap_or_default()
    }

    #[test]
    fn scan_shows_the_software_of_the_host() {
        let (mut world, player, _, software) = armed();
        assert_ne!(software, "");
        assert_eq!(
            plain(&world.command(player, "scan")),
            [
                "scan finds no ICE here.".to_owned(),
                format!("Software: {software}")
            ]
        );
    }

    #[test]
    fn a_corp_patches_software_after_many_exploits_until_a_new_hole_is_found() {
        let (mut world, player, corp, software) = armed();
        world.corp_states[corp].used.insert(software.clone(), 11);
        let id = world.hosts[world.here(player).0].id.clone();
        assert_eq!(
            plain(&world.command(player, "exploit")),
            [
                format!("exploit opens a shell on {id}. Your access here: user."),
                format!("Acme Corp finds the hole you used and patches {software}."),
            ]
        );
        assert_eq!(
            plain(&world.command(player, "exploit")),
            [format!(
                "Acme Corp patched the hole in {software}. exploit does not work on it until somebody finds a new hole."
            )]
        );
        assert_eq!(
            plain(&world.command(player, "scan"))[1],
            format!("Software: {software} (patched)")
        );
        world.set_time(NOON + 2 * DAY);
        assert_eq!(
            plain(&world.command(player, "exploit")),
            [format!(
                "exploit opens a shell on {id}. Your access here: admin."
            )]
        );
        assert_eq!(world.corp_states[corp].used.get(&software), Some(&1));
    }

    #[test]
    fn a_richer_corp_patches_after_fewer_uses_and_for_longer() {
        let (mut world, _) = hired_at(NOON);
        let globex = corp(&world, "globex");
        for _ in 0..7 {
            assert!(world.expose(globex, "Deskline OS").is_none());
        }
        assert!(world.expose(globex, "Deskline OS").is_some());
        world.set_time(NOON + 4 * DAY - 1);
        assert!(world.patched(globex, "Deskline OS"));
        assert!(!world.patched(globex, "Gatekeep VPN"));
        assert!(!world.patched(corp(&world, "acme"), "Deskline OS"));
        world.set_time(NOON + 4 * DAY);
        assert!(!world.patched(globex, "Deskline OS"));
    }

    #[test]
    fn a_host_without_software_is_never_patched() {
        let (mut world, player, corp, _) = armed();
        let here = world.here(player);
        world.hosts[here.0].software = None;
        world.command(player, "exploit");
        assert_eq!(world.corp_states[corp].used, BTreeMap::new());
        let shown = plain(&world.command(player, "scan"));
        assert_eq!(shown, ["scan finds no ICE here."]);
    }

    #[test]
    fn uses_and_patches_come_back_from_world_ini() {
        let (mut world, _) = hired_at(NOON);
        let acme = corp(&world, "acme");
        world.expose(acme, "Deskline OS");
        for _ in 0..12 {
            world.expose(acme, "Gatekeep VPN");
        }
        world.corp_states[acme]
            .patched
            .insert("Spindle Web".to_owned(), NOON);
        let saves = world.saves();
        let text = saves
            .iter()
            .find(|(path, _)| path == "world.ini")
            .map_or_default(|(_, text)| text.clone());
        let until = NOON + 2 * DAY;
        assert!(
            text.ends_with(&format!(
                "\n\n[corp acme]\nused = Deskline OS: 1\npatched = Gatekeep VPN: {until}\n"
            )),
            "{text}"
        );
        let mut fresh = crate::testing::world();
        for (path, text) in &saves {
            fresh.restore(path, text).unwrap();
        }
        fresh.set_time(NOON);
        assert!(fresh.patched(acme, "Gatekeep VPN"));
        assert_eq!(fresh.corp_states[acme].used.get("Deskline OS"), Some(&1));
        let errors: Vec<String> = fresh
            .restore(
                "world.ini",
                "[world test]\nseed = 7\n[corp acme]\nused = Deskline OS\n",
            )
            .unwrap_err()
            .iter()
            .map(ToString::to_string)
            .collect();
        assert_eq!(
            errors,
            ["line 4: bad used: Deskline OS (use a software name, a colon, and a number)"]
        );
    }
}
