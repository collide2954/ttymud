use ttymud_text::{Message, Style};

use super::instance::InstanceId;
use super::{HostId, NetworkId, PlayerId, World, listed};
use crate::content::{File, Zone};
use crate::target::Level;

const CONSENT: u64 = 60;
const MAX_DROPS: usize = 100;

impl World {
    pub(super) fn zone(&self, network: NetworkId) -> Zone {
        self.networks[network.0].zone
    }

    pub(super) fn consent(&mut self, player: PlayerId, target: HostId) -> Result<(), Message> {
        let Some(network) = self.hosts[target.0].network else {
            return Ok(());
        };
        if self.zone(network) != Zone::Darknet {
            return Ok(());
        }
        let now = self.now;
        let Some(state) = self.players.get_mut(&player) else {
            return Ok(());
        };
        if state.run.is_some() {
            return Ok(());
        }
        let agreed = state
            .consent
            .is_some_and(|(host, when)| host == target && now.saturating_sub(when) <= CONSENT);
        if agreed {
            state.consent = None;
            return Ok(());
        }
        state.consent = Some((target, now));
        let name = &self.networks[network.0].name;
        let id = &self.hosts[target.0].id;
        Err(Message::error(format!(
            "{name} is a darknet. Other handles there can crash your rig, take your files, and tip off the police. Type "
        ))
        .span(Style::Command, format!("go {id}"))
        .text(" again to agree."))
    }

    pub(super) fn darknet_copy(&self, network: NetworkId) -> Option<InstanceId> {
        let found = self
            .instances
            .iter()
            .find(|(_, instance)| instance.network == network);
        found.map(|(&id, _)| id)
    }

    pub(super) fn darknet_crash(&mut self, player: PlayerId) -> Option<Vec<Message>> {
        let home = self.home?;
        let network = self.players.get(&player)?.run.as_ref()?.network;
        if self.zone(network) != Zone::Darknet {
            return None;
        }
        let here = self.here(player);
        let state = self.players.get_mut(&player)?;
        let (mut dropped, kept): (Vec<File>, Vec<File>) = std::mem::take(&mut state.rig.files)
            .into_iter()
            .partition(|file| file.value > 0 || file.teaches().is_some());
        state.rig.files = kept;
        let names: Vec<String> = dropped.iter().map(|file| file.name.clone()).collect();
        let seen = Message::game("The rig of ")
            .span(Style::Player, state.name.clone())
            .text(" crashes");
        self.notify(here, player, &drops(seen, &names));
        let start = self.start;
        let state = self.players.get_mut(&player)?;
        let closed = state.run.take().map(|run| run.instance);
        state.chain = if home == start {
            vec![home]
        } else {
            vec![start, home]
        };
        if let Some(id) = closed {
            self.close_instance(id);
        }
        for file in &mut dropped {
            file.lock = Level::Guest;
        }
        self.drop_files(here, dropped);
        let crash = Message::game("Your rig crashes");
        let crash = if names.is_empty() {
            crash.text(".")
        } else {
            let on = format!(" and drops these files on {}: ", self.hosts[here.0].id);
            listed(
                crash.text(on),
                Style::File,
                names.iter().map(String::as_str),
            )
            .text(".")
        };
        let wake = Message::game("You wake up at ")
            .span(Style::Host, &self.hosts[home.0].id)
            .text(" with 0 integrity. Type ")
            .span(Style::Command, "repair")
            .text(" to fix your rig.");
        let mut shown = vec![crash, wake];
        shown.extend(self.scene(home, player));
        Some(shown)
    }

    pub(super) fn drop_files(&mut self, host: HostId, files: Vec<File>) {
        self.drops
            .extend(files.into_iter().map(|file| (host, file)));
        let excess = self.drops.len().saturating_sub(MAX_DROPS);
        self.drops.drain(..excess);
    }
}

fn drops(message: Message, names: &[String]) -> Message {
    if names.is_empty() {
        return message.text(".");
    }
    listed(
        message.text(" and drops "),
        Style::File,
        names.iter().map(String::as_str),
    )
    .text(".")
}

#[cfg(test)]
mod tests {
    use crate::commands::Verb;
    use crate::content::File;
    use crate::target::Level;
    use crate::testing::{dark, heard, inside, plain};
    use crate::{PlayerId, World};

    const ASK: &str = "The Undercity is a darknet. Other handles there can crash your rig, take your files, and tip off the police. Type go alley again to agree.";
    const DROPS: &str = "payroll.db, crack-v3.blueprint";

    fn carry(world: &mut World, player: PlayerId) {
        let mut payroll = File::new("payroll.db", vec!["Pay.".to_owned()]);
        payroll.value = 40;
        payroll.lock = Level::Admin;
        if let Some(state) = world.players.get_mut(&player) {
            let memo = File::new("memo.txt", vec!["Hi.".to_owned()]);
            state.rig.files = vec![memo, payroll, File::blueprint(Verb::Crack, 3)];
        }
    }

    fn crashed() -> (World, PlayerId, PlayerId, Vec<crate::Output>) {
        let mut world = dark();
        let nyx = inside(&mut world, "nyx");
        let zer0 = inside(&mut world, "zer0");
        carry(&mut world, nyx);
        if let Some(state) = world.players.get_mut(&nyx) {
            state.rig.integrity = 10;
        }
        let outputs = world.command(zer0, "crack nyx");
        (world, nyx, zer0, outputs)
    }

    #[test]
    fn a_crash_in_a_darknet_drops_the_valuable_files_and_wakes_you_at_home() {
        let (mut world, nyx, zer0, outputs) = crashed();
        assert_eq!(
            heard(&outputs, zer0),
            [
                "crack hits nyx. Integrity: 10 -> 0.".to_owned(),
                format!("The rig of nyx crashes and drops {DROPS}."),
            ]
        );
        assert_eq!(
            heard(&outputs, nyx)[..4],
            [
                "zer0 cracks your rig. Integrity: 10 -> 0.".to_owned(),
                format!("Your rig crashes and drops these files on alley: {DROPS}."),
                "You wake up at home with 0 integrity. Type repair to fix your rig.".to_owned(),
                "Home".to_owned(),
            ]
        );
        assert_eq!(world.prompt(nyx).as_deref(), Some("nyx@home $ "));
        assert_eq!(plain(&world.command(nyx, "inventory")), ["memo.txt"]);
        assert_eq!(plain(&world.command(nyx, "back"))[0], "Hub");
        assert_eq!(
            plain(&world.command(zer0, "ls")),
            ["hub/", "den/", "payroll.db", "crack-v3.blueprint"]
        );
    }

    #[test]
    fn a_dropped_file_goes_to_the_first_handle_that_downloads_it() {
        let (mut world, _, zer0, _) = crashed();
        assert_eq!(
            plain(&world.command(zer0, "get payroll.db")),
            ["Downloaded payroll.db."]
        );
        let ghost = inside(&mut world, "ghost");
        assert_eq!(
            plain(&world.command(ghost, "get payroll.db")),
            ["No file named payroll.db. Files: crack-v3.blueprint"]
        );
        assert_eq!(world.complete(ghost, "get ").names, ["crack-v3.blueprint"]);
    }

    #[test]
    fn a_darknet_keeps_the_newest_100_dropped_files() {
        let mut world = dark();
        let alley = world.start;
        let files = (0..101).map(|index| File::new(format!("f{index}.db"), Vec::new()));
        world.drop_files(alley, files.collect());
        assert_eq!(world.drops.len(), 100);
        assert_eq!(world.drops[0].1.name, "f1.db");
    }

    #[test]
    fn jackout_leaves_a_darknet_with_all_the_files() {
        let mut world = dark();
        let nyx = inside(&mut world, "nyx");
        carry(&mut world, nyx);
        assert_eq!(
            plain(&world.command(nyx, "jackout"))[..2],
            [
                "You jack out of The Undercity. The shock costs your rig 20 integrity.",
                "Hub"
            ]
        );
        assert_eq!(
            plain(&world.command(nyx, "inventory")),
            ["memo.txt", "payroll.db", "crack-v3.blueprint"]
        );
    }

    #[test]
    fn a_darknet_asks_for_a_second_go_on_each_entry() {
        let mut world = dark();
        let (nyx, _) = world.join("nyx");
        assert_eq!(plain(&world.command(nyx, "go alley")), [ASK]);
        assert_eq!(world.prompt(nyx).as_deref(), Some("nyx@hub $ "));
        let shown = plain(&world.command(nyx, "go alley"));
        assert_eq!(
            shown[0],
            "You enter the darknet The Undercity. Other handles can attack you here."
        );
        assert_eq!(shown[1], "Alley");
        assert_eq!(
            plain(&world.command(nyx, "go den"))[0],
            "Den",
            "a move inside the darknet does not ask"
        );
        world.command(nyx, "back");
        assert_eq!(
            plain(&world.command(nyx, "back"))[0],
            "You leave the darknet The Undercity."
        );
        assert_eq!(plain(&world.command(nyx, "go alley")), [ASK]);
    }

    #[test]
    fn the_consent_lasts_one_minute() {
        let mut world = dark();
        let (nyx, _) = world.join("nyx");
        world.command(nyx, "go alley");
        world.set_time(61);
        assert_eq!(plain(&world.command(nyx, "go alley")), [ASK]);
        world.set_time(120);
        assert_eq!(world.prompt(nyx).as_deref(), Some("nyx@hub $ "));
        world.command(nyx, "go alley");
        assert_eq!(
            world.prompt(nyx).as_deref(),
            Some("[integ 100] nyx@alley $ ")
        );
    }

    #[test]
    fn all_handles_in_a_darknet_share_one_copy() {
        let mut world = dark();
        let nyx = inside(&mut world, "nyx");
        let (zer0, _) = world.join("zer0");
        world.command(zer0, "go alley");
        let outputs = world.command(zer0, "go alley");
        assert_eq!(heard(&outputs, nyx), ["zer0 arrives."]);
        assert!(
            heard(&outputs, zer0).contains(&"Here: nyx".to_owned()),
            "{outputs:?}"
        );
        assert_eq!(world.instance_id(nyx), world.instance_id(zer0));
        assert_eq!(world.instances.len(), 1);
        assert_eq!(world.mates(nyx), Vec::new(), "strangers are not crewmates");
    }

    #[test]
    fn noise_in_a_darknet_makes_no_trace() {
        let mut world = dark();
        let nyx = inside(&mut world, "nyx");
        world.command(nyx, "scan");
        let trace = world
            .players
            .get(&nyx)
            .and_then(|state| state.run.as_ref())
            .map(|run| run.trace);
        assert_eq!(trace, Some(0));
    }
}
