use ttymud_text::{Message, Style};

use super::jobs::DAY;
use super::{PlayerId, Step, World};
use crate::content::File;
use crate::target::Level;

impl World {
    pub(super) fn exploit(&mut self, player: PlayerId) -> Result<Step, Message> {
        let here = self.here(player);
        let next = match self.access(player) {
            Level::Guest => Level::User,
            Level::User => Level::Admin,
            Level::Admin => return Err(Message::error("You already have admin access here.")),
        };
        self.refuse_patched(player)?;
        let Some(run) = self
            .players
            .get_mut(&player)
            .and_then(|state| state.run.as_mut())
        else {
            return Ok(Step::default());
        };
        run.shells.retain(|&(host, _)| host != here);
        run.shells.push((here, next));
        let shell = Message::game("exploit opens a shell on ")
            .span(Style::Host, &self.hosts[here.0].id)
            .text(format!(". Your access here: {}.", next.word()));
        let mut shown = vec![shell];
        if let Some((corp, software)) = self.exposed(player) {
            shown.extend(self.expose(corp, &software));
        }
        Ok(Step::show(shown))
    }

    pub(super) fn access(&self, player: PlayerId) -> Level {
        let here = self.here(player);
        let Some(run) = self
            .players
            .get(&player)
            .and_then(|state| state.run.as_ref())
        else {
            return Level::Admin;
        };
        run.shells
            .iter()
            .filter(|(host, _)| *host == here)
            .map(|&(_, level)| level)
            .fold(run.access, Level::max)
    }

    pub(super) fn needs(&self, player: PlayerId, file: &File) -> Option<String> {
        (file.lock > self.access(player)).then(|| format!(" (needs {})", file.lock.word()))
    }

    pub(super) fn locked(&self, player: PlayerId, file: &File) -> Option<Message> {
        let access = self.access(player);
        (file.lock > access).then(|| {
            let (name, lock, access) = (&file.name, file.lock.word(), access.word());
            Message::error(format!(
                "{name} needs {lock} access. Your access here: {access}."
            ))
        })
    }

    pub(super) fn learn(&mut self, player: PlayerId, files: &[File]) -> Option<Message> {
        let day = self.now / DAY;
        let run = self.players.get_mut(&player)?.run.as_mut()?;
        let network = &self.networks[run.network.0];
        let found: Vec<&(String, Level)> = files
            .iter()
            .filter_map(|file| file.key.as_ref())
            .filter_map(|key| network.keys.iter().find(|(known, _)| known == key))
            .collect();
        let reset = |key: &str| {
            let mut resets = network.resets.iter();
            resets.any(|(known, when)| known == key && *when == day)
        };
        let (stale, fresh): (Vec<_>, Vec<_>) = found.into_iter().partition(|(key, _)| reset(key));
        let Some(best) = fresh.iter().map(|&&(_, level)| level).max() else {
            return (!stale.is_empty()).then(|| {
                Message::game("These credentials do not work. Somebody reset the password.")
            });
        };
        if best <= run.access {
            return None;
        }
        run.access = best;
        run.passwords
            .extend(fresh.iter().map(|(key, _)| key.clone()));
        let (name, level) = (&network.name, best.word());
        Some(Message::game(format!(
            "You learn the credentials. Your access in {name}: {level}."
        )))
    }
}

#[cfg(test)]
mod tests {
    use crate::target::Level;
    use crate::testing::{plain, world};
    use crate::world::HostId;
    use crate::{PlayerId, World};

    fn inside() -> (World, PlayerId) {
        let mut world = world();
        world.set_time(20_000 * 86_400 + 100);
        let (player, _) = world.join("ghost");
        for line in ["go bazaar", "accept 1", "go relay", "go back-room"] {
            world.command(player, line);
        }
        let entry = world.networks.last().map(|network| network.entries[0]);
        if let Some(entry) = entry {
            let id = world.hosts[entry.0].id.clone();
            world.command(player, &format!("go {id}"));
        }
        (world, player)
    }

    fn holder(world: &World, found: impl Fn(&crate::content::File) -> bool) -> (HostId, String) {
        let network = world.networks.len() - 1;
        let index = (0..world.hosts.len())
            .find(|&index| {
                let host = &world.hosts[index];
                host.network.is_some_and(|id| id.0 == network) && host.files.iter().any(&found)
            })
            .unwrap_or(0);
        let name = world.hosts[index]
            .files
            .iter()
            .find(|file| found(file))
            .map_or_default(|file| file.name.clone());
        (HostId(index), name)
    }

    fn visit(world: &mut World, player: PlayerId, host: HostId) {
        if let Some(state) = world.players.get_mut(&player) {
            state.chain.push(host);
        }
    }

    #[test]
    fn lan_data_needs_user_access() {
        let (mut world, player) = inside();
        let (host, file) = holder(&world, |file| file.lock == Level::User && file.value > 0);
        visit(&mut world, player, host);
        let refused = format!("{file} needs user access. Your access here: guest.");
        assert_eq!(
            plain(&world.command(player, &format!("read {file}"))),
            std::slice::from_ref(&refused)
        );
        assert_eq!(
            plain(&world.command(player, &format!("download {file}"))),
            [refused]
        );
    }

    #[test]
    fn credentials_raise_the_access_in_the_network() {
        let (mut world, player) = inside();
        let (edge, creds) = holder(&world, |file| {
            file.key.is_some() && file.lock == Level::Guest
        });
        visit(&mut world, player, edge);
        let shown = plain(&world.command(player, &format!("read {creds}")));
        assert_eq!(
            shown.last().map(String::as_str),
            Some("You learn the credentials. Your access in Acme Corp: user.")
        );
        let (host, file) = holder(&world, |file| file.lock == Level::User && file.value > 0);
        visit(&mut world, player, host);
        assert_eq!(
            plain(&world.command(player, &format!("read {file}"))).len(),
            1
        );
    }

    #[test]
    fn credentials_on_the_rig_work_in_the_next_run() {
        let (mut world, player) = inside();
        let (edge, creds) = holder(&world, |file| {
            file.key.is_some() && file.lock == Level::Guest
        });
        visit(&mut world, player, edge);
        world.command(player, &format!("download {creds}"));
        world.command(player, "jackout");
        let entry = world.networks.last().map(|network| network.entries[0]);
        if let Some(entry) = entry {
            let id = world.hosts[entry.0].id.clone();
            world.command(player, &format!("go {id}"));
        }
        let shown = plain(&world.command(player, &format!("read {creds}")));
        assert_eq!(
            shown.last().map(String::as_str),
            Some("You learn the credentials. Your access in Acme Corp: user.")
        );
    }

    fn armed() -> (World, PlayerId) {
        let mut world = world();
        world.set_time(20_000 * 86_400 + 100);
        let (player, _) = world.join("ghost");
        let lines = [
            "go bazaar",
            "accept 1",
            "go relay",
            "go back-room",
            "unload patch",
        ];
        for line in lines.iter().chain(&["load exploit"]) {
            world.command(player, line);
        }
        let entry = world.networks.last().map(|network| network.entries[0]);
        if let Some(entry) = entry {
            let id = world.hosts[entry.0].id.clone();
            world.command(player, &format!("go {id}"));
        }
        (world, player)
    }

    #[test]
    fn exploit_raises_the_access_on_this_host_only() {
        let (mut world, player) = armed();
        let (host, file) = holder(&world, |file| file.lock == Level::User && file.value > 0);
        visit(&mut world, player, host);
        let id = world.hosts[host.0].id.clone();
        let shown = plain(&world.command(player, "exploit"));
        assert_eq!(
            shown[0],
            format!("exploit opens a shell on {id}. Your access here: user.")
        );
        assert_eq!(
            plain(&world.command(player, &format!("read {file}"))).len(),
            1
        );
        world.command(player, "back");
        assert_eq!(
            plain(&world.command(player, "status | grep access")),
            ["Access here: guest"]
        );
    }

    #[test]
    fn exploit_stops_at_admin() {
        let (mut world, player) = armed();
        for _ in 0..2 {
            for _ in 0..4 {
                world.tick();
            }
            world.command(player, "exploit");
        }
        for _ in 0..4 {
            world.tick();
        }
        assert_eq!(
            plain(&world.command(player, "exploit")),
            ["You already have admin access here."]
        );
    }

    #[test]
    fn hosts_mark_the_files_that_need_more_access() {
        let (mut world, player) = inside();
        let (host, file) = holder(&world, |file| file.lock == Level::User && file.value > 0);
        visit(&mut world, player, host);
        let scene = plain(&world.command(player, "look"));
        let files = scene
            .iter()
            .find(|line| line.starts_with("Files: "))
            .cloned()
            .unwrap_or_default();
        assert!(files.contains(&format!("{file} (needs user)")), "{files}");
        let listed = plain(&world.command(player, "ls"));
        assert!(
            listed.contains(&format!("{file} (needs user)")),
            "{listed:?}"
        );
        assert_eq!(
            plain(&world.command(player, "status | grep access")),
            ["Access here: guest"]
        );
    }
}
