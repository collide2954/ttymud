use ttymud_text::{Kind, Message, Style};

use super::instance::{InstanceId, tell};
use super::{HostId, Output, PlayerId, Step, World, ticks};
use crate::target::{RoleBridge, Vault};

const SYNC_EVERY: u32 = 20;
const SYNC_OPEN: u32 = 3;
const SYNC_WARN: u32 = 2;

impl World {
    fn run_vault(&self, player: PlayerId) -> Option<Vault> {
        let network = self.players.get(&player)?.run.as_ref()?.network;
        self.networks[network.0].vault.clone()
    }

    pub(super) fn tick_vault(&mut self, id: InstanceId) -> Vec<Output> {
        let Some(instance) = self.instances.get_mut(&id) else {
            return Vec::new();
        };
        let Some(vault) = &self.networks[instance.network.0].vault else {
            return Vec::new();
        };
        let (laptop, safe) = (HostId(vault.laptop), HostId(vault.host));
        let line = |rest: &str| {
            Message::new(Kind::Telegraph)
                .span(Style::Host, &self.hosts[laptop.0].id)
                .text(rest.to_owned())
        };
        let to_vault = |message: Message, rest: &str| {
            message
                .span(Style::Host, &self.hosts[safe.0].id)
                .text(rest.to_owned())
        };
        instance.clock = (instance.clock + 1) % SYNC_EVERY;
        let opens = SYNC_EVERY - SYNC_OPEN;
        let (message, hosts) = match instance.clock {
            0 => {
                instance.bridges.retain(|&bridge| bridge != (laptop, safe));
                let ends = line(" ends its sync. The route to ");
                (to_vault(ends, " closes."), vec![laptop, safe])
            }
            clock if clock == opens - SYNC_WARN => (
                line(&format!(
                    " starts its sync with the vault ({}).",
                    ticks(SYNC_WARN)
                )),
                vec![laptop],
            ),
            clock if clock == opens => {
                instance.bridges.push((laptop, safe));
                let syncs = line(" syncs with the vault. The route to ");
                let open = format!(" is open for {}.", ticks(SYNC_OPEN));
                (to_vault(syncs, &open), vec![laptop])
            }
            _ => return Vec::new(),
        };
        let present: Vec<PlayerId> = self
            .members(id)
            .iter()
            .filter(|member| hosts.contains(&member.host))
            .map(|member| member.player)
            .collect();
        tell(&present, &message)
    }

    pub(super) fn trick_employee(
        &mut self,
        player: PlayerId,
        word: &str,
    ) -> Option<Result<Step, Message>> {
        let vault = self.run_vault(player)?;
        let RoleBridge::Employee { host, name } = &vault.bridge else {
            return None;
        };
        if !name.eq_ignore_ascii_case(word) {
            return None;
        }
        let (desk, safe) = (HostId(*host), HostId(vault.host));
        if self.here(player) != desk {
            return Some(Err(Message::error(format!(
                "{name} does not work on this host."
            ))));
        }
        let instance = self.instance_mut(player)?;
        if instance.bridges.contains(&(desk, safe)) {
            return Some(Err(Message::error(format!(
                "{name} already put your drive into the vault."
            ))));
        }
        instance.bridges.push((desk, safe));
        let message = Message::game(format!(
            "You call {name} as the help desk. {name} puts your USB drive into the vault. The route from "
        ))
        .span(Style::Host, &self.hosts[desk.0].id)
        .text(" to ")
        .span(Style::Host, &self.hosts[safe.0].id)
        .text(" is open.");
        Some(Ok(Step::show(vec![message])))
    }

    pub(super) fn ride_modem(&mut self, player: PlayerId) -> Option<Message> {
        let vault = self.run_vault(player)?;
        let RoleBridge::Modem { host } = vault.bridge else {
            return None;
        };
        let (line, safe) = (HostId(host), HostId(vault.host));
        if self.here(player) != line {
            return None;
        }
        let instance = self.instance_mut(player)?;
        if instance.bridges.contains(&(line, safe)) {
            return None;
        }
        instance.bridges.push((line, safe));
        Some(
            Message::game("Your proxy finds the old modem line. The route from ")
                .span(Style::Host, &self.hosts[line.0].id)
                .text(" to ")
                .span(Style::Host, &self.hosts[safe.0].id)
                .text(" is open."),
        )
    }

    pub(super) fn employee_here(&self, player: PlayerId) -> Option<&str> {
        let network = self.players.get(&player)?.run.as_ref()?.network;
        let vault = self.networks[network.0].vault.as_ref()?;
        match &vault.bridge {
            RoleBridge::Employee { host, name } if *host == self.here(player).0 => {
                Some(name.as_str())
            }
            _ => None,
        }
    }

    pub(super) fn vault_lines(&self, player: PlayerId) -> Vec<Message> {
        let here = self.here(player);
        let Some(instance) = self.instance(player) else {
            return Vec::new();
        };
        let Some(vault) = &self.networks[instance.network.0].vault else {
            return Vec::new();
        };
        let mut shown = Vec::new();
        let safe = &self.hosts[vault.host].id;
        if here.0 == vault.laptop {
            let open = instance
                .bridges
                .contains(&(HostId(vault.laptop), HostId(vault.host)));
            let when = if open {
                " is open now.".to_owned()
            } else {
                let left = (SYNC_EVERY - SYNC_OPEN + SYNC_EVERY - instance.clock) % SYNC_EVERY;
                format!(" opens in {}.", ticks(left))
            };
            shown.push(
                Message::game(format!(
                    "This laptop syncs with the vault every {SYNC_EVERY} ticks. The route to "
                ))
                .span(Style::Host, safe)
                .text(when),
            );
        }
        match &vault.bridge {
            RoleBridge::Employee { host, name } if *host == here.0 => shown.push(
                Message::game("An IT admin works on this host: ")
                    .span(Style::Player, name)
                    .text(". A pretext can get a USB drive into the vault."),
            ),
            RoleBridge::Modem { host } if *host == here.0 => shown.push(Message::game(
                "An old modem line hums on this host. A proxy can ride it into the vault.",
            )),
            _ => {}
        }
        shown
    }

    pub(super) fn vault_hop(&self, player: PlayerId, from: HostId, to: HostId) -> bool {
        let Some(vault) = self.run_vault(player) else {
            return false;
        };
        let safe = HostId(vault.host);
        (from == safe || to == safe) && !self.routes(from, player).contains(&to)
    }
}

#[cfg(test)]
mod tests {
    use crate::PlayerId;
    use crate::commands::Verb;
    use crate::target::{RoleBridge, Vault, generate_heist};
    use crate::testing::{plain, world};
    use crate::world::contract::{Contract, Job, Task};
    use crate::world::{HostId, World};

    fn seed_with(employee: bool) -> u64 {
        let world = world();
        (0..50)
            .find(|&seed| {
                let heist = generate_heist(&world.corps[0], &world.templates, seed);
                let bridge = heist
                    .and_then(|heist| heist.vault)
                    .map(|vault| vault.bridge);
                matches!(bridge, Some(RoleBridge::Employee { .. })) == employee
            })
            .unwrap_or_default()
    }

    fn heist(seed: u64) -> (World, PlayerId, Vault) {
        let mut world = world();
        let target = generate_heist(&world.corps[0], &world.templates, seed).unwrap();
        let network = world.add_target(&target, 0);
        world.contracts.push(Contract {
            id: "heist".to_owned(),
            fixer: 0,
            network,
            goal: target.goal.clone(),
            task: Task::Steal,
            offer: Vec::new(),
            steps: Vec::new(),
            pay: 0,
            posting: None,
        });
        let (player, _) = world.join("ghost");
        let job = Job::new(world.contracts.len() - 1);
        if let Some(state) = world.players.get_mut(&player) {
            state.contract = Some(job);
            state.rig.cycles = 20;
            state.rig.loads.extend([Verb::Pretext, Verb::Proxy]);
        }
        world.command(player, "go back-room");
        let entry = world.networks[network.0].entries[0];
        let id = world.hosts[entry.0].id.clone();
        world.command(player, &format!("go {id}"));
        if let Some(instance) = world.instance_mut(player) {
            instance.ice.clear();
            instance.analysts.clear();
        }
        let vault = world.networks[network.0].vault.clone().unwrap();
        (world, player, vault)
    }

    fn visit(world: &mut World, player: PlayerId, host: usize) -> String {
        if let Some(state) = world.players.get_mut(&player) {
            state.chain.push(HostId(host));
        }
        world.hosts[host].id.clone()
    }

    #[test]
    fn the_laptop_opens_a_route_to_the_vault_for_three_ticks_every_twenty() {
        let (mut world, player, vault) = heist(1);
        let laptop = visit(&mut world, player, vault.laptop);
        let safe = world.hosts[vault.host].id.clone();
        let scan = plain(&world.command(player, "scan"));
        let line = format!(
            "This laptop syncs with the vault every 20 ticks. The route to {safe} opens in 17 ticks."
        );
        assert!(scan.contains(&line), "{scan:?}");
        assert!(
            !world
                .route_names(HostId(vault.laptop), player)
                .contains(&safe.as_str())
        );
        let mut told = Vec::new();
        for _ in 0..17 {
            told.extend(plain(&world.tick()));
        }
        assert!(
            told.contains(&format!(
                "{laptop} starts its sync with the vault (2 ticks)."
            )),
            "{told:?}"
        );
        assert!(told.contains(&format!(
            "{laptop} syncs with the vault. The route to {safe} is open for 3 ticks."
        )));
        assert_eq!(
            plain(&world.command(player, &format!("go {safe}")))[0],
            "Acme Corp Vault"
        );
        let mut told = Vec::new();
        for _ in 0..3 {
            told.extend(plain(&world.tick()));
        }
        assert_eq!(
            told,
            [format!(
                "{laptop} ends its sync. The route to {safe} closes."
            )]
        );
        assert_eq!(
            plain(&world.command(player, "back")),
            [format!("There is no route back to {laptop} now.")]
        );
    }

    #[test]
    fn a_face_talks_the_employee_into_a_usb_drive_for_the_vault() {
        let (mut world, player, vault) = heist(seed_with(true));
        let RoleBridge::Employee { host, name } = vault.bridge.clone() else {
            return;
        };
        let desk = visit(&mut world, player, host);
        let safe = world.hosts[vault.host].id.clone();
        let scan = plain(&world.command(player, "scan"));
        let line = format!(
            "An IT admin works on this host: {name}. A pretext can get a USB drive into the vault."
        );
        assert!(scan.contains(&line), "{scan:?}");
        assert_eq!(
            world.complete(player, "pretext ").names,
            std::slice::from_ref(&name)
        );
        assert_eq!(
            plain(&world.command(player, &format!("pretext {name}"))),
            [format!(
                "You call {name} as the help desk. {name} puts your USB drive into the vault. The route from {desk} to {safe} is open."
            )]
        );
        assert_eq!(
            plain(&world.command(player, &format!("pretext {name}"))),
            [format!("{name} already put your drive into the vault.")]
        );
        assert_eq!(
            plain(&world.command(player, &format!("go {safe}")))[0],
            "Acme Corp Vault"
        );
        assert_eq!(
            plain(&world.command(player, "back"))[0],
            world.hosts[host].name
        );
    }

    #[test]
    fn a_phreak_rides_the_old_modem_line_into_the_vault() {
        let (mut world, player, vault) = heist(seed_with(false));
        let RoleBridge::Modem { host } = vault.bridge else {
            return;
        };
        let line = visit(&mut world, player, host);
        let safe = world.hosts[vault.host].id.clone();
        let scan = plain(&world.command(player, "scan"));
        let hum = "An old modem line hums on this host. A proxy can ride it into the vault.";
        assert!(scan.contains(&hum.to_owned()), "{scan:?}");
        assert_eq!(
            plain(&world.command(player, "proxy")),
            [
                format!(
                    "Your proxy finds the old modem line. The route from {line} to {safe} is open."
                ),
                "Your link goes through a proxy. ICE attacks on your trace do half damage in this run."
                    .to_owned(),
            ]
        );
        assert_eq!(
            plain(&world.command(player, &format!("go {safe}")))[0],
            "Acme Corp Vault"
        );
    }
}
