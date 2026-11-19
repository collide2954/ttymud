use ttymud_text::{Kind, Message, Style};

use super::{Host, HostId, NetworkId, PlayerId, World};
use crate::ice::IceKind;

#[derive(Debug)]
pub(super) struct Ice {
    pub(super) host: HostId,
    pub(super) name: String,
    pub(super) kind: IceKind,
    pub(super) version: u32,
    pub(super) integrity: u32,
    pub(super) guards: Option<HostId>,
    pub(super) known: bool,
}

impl Ice {
    pub(super) fn broken(&self) -> bool {
        self.integrity == 0
    }

    pub(super) fn describe(&self) -> Message {
        let title = format!("{} v{}", self.kind.title(), self.version);
        let state = if self.broken() {
            format!(": {title}, broken")
        } else {
            let max = self.kind.integrity(self.version);
            format!(": {title}, integrity {}/{max}", self.integrity)
        };
        Message::new(Kind::Game)
            .span(Style::Ice, &self.name)
            .text(state)
    }
}

pub(super) fn deploy(hosts: &[Host], network: NetworkId) -> Vec<Ice> {
    let find = |id: &str| hosts.iter().position(|host| host.id == id).map(HostId);
    hosts
        .iter()
        .enumerate()
        .filter(|(_, host)| host.network == Some(network))
        .flat_map(|(index, host)| {
            host.ice.iter().map(move |def| Ice {
                host: HostId(index),
                name: def.name.clone(),
                kind: def.kind,
                version: def.version,
                integrity: def.kind.integrity(def.version),
                guards: def.guards.as_deref().and_then(find),
                known: false,
            })
        })
        .collect()
}

impl World {
    pub(super) fn guard(&self, player: PlayerId, target: HostId) -> Result<(), Message> {
        let here = self.here(player);
        let Some(run) = self
            .players
            .get(&player)
            .and_then(|state| state.run.as_ref())
        else {
            return Ok(());
        };
        let warden = run
            .ice
            .iter()
            .find(|ice| ice.host == here && ice.guards == Some(target) && ice.integrity > 0);
        let Some(warden) = warden else {
            return Ok(());
        };
        let message = if warden.known {
            Message::new(Kind::Error).span(Style::Ice, &warden.name)
        } else {
            Message::error("Unknown ICE")
        };
        Err(message
            .text(" blocks the route to ")
            .span(Style::Host, &self.hosts[target.0].id)
            .text("."))
    }

    pub(super) fn ice_line(&self, host: HostId, viewer: PlayerId) -> Option<Message> {
        let run = self.players.get(&viewer)?.run.as_ref()?;
        let here: Vec<&Ice> = run.ice.iter().filter(|ice| ice.host == host).collect();
        if here.is_empty() {
            return None;
        }
        let known: Vec<&Ice> = here.iter().copied().filter(|ice| ice.known).collect();
        let unknown = here.len() - known.len();
        let mut message = Message::game("ICE: ");
        for (index, ice) in known.iter().enumerate() {
            if index > 0 {
                message = message.text(", ");
            }
            message = message.span(Style::Ice, &ice.name);
            if ice.broken() {
                message = message.text(" (broken)");
            }
        }
        if unknown > 0 {
            if !known.is_empty() {
                message = message.text(", ");
            }
            message = message
                .text(format!("{unknown} unknown. Type "))
                .span(Style::Command, "scan")
                .text(" to see them.");
        }
        Some(message)
    }
}

#[cfg(test)]
mod tests {
    use crate::testing::walk;

    fn at_gate(lines: &[&str]) -> Vec<String> {
        walk(&[&["go back-room", "go gate"][..], lines].concat()).0
    }

    #[test]
    fn a_warden_blocks_the_route_that_it_guards() {
        assert_eq!(
            at_gate(&["go vault"]),
            ["Unknown ICE blocks the route to vault."]
        );
        assert_eq!(
            at_gate(&["scan", "go vault"]),
            ["warden blocks the route to vault."]
        );
    }

    #[test]
    fn a_warden_blocks_only_its_own_route() {
        assert_eq!(at_gate(&["go core"])[0], "Lab Core");
    }
}
