use ttymud_text::{Message, Style};

use super::{Host, HostId, NetworkId, PlayerId, World, listed};
use crate::ice::IceKind;

#[derive(Debug)]
pub(super) struct Ice {
    pub(super) host: HostId,
    pub(super) name: String,
    pub(super) kind: IceKind,
    pub(super) version: u32,
    pub(super) integrity: u32,
    pub(super) known: bool,
}

impl Ice {
    pub(super) fn max(&self) -> u32 {
        self.kind.integrity(self.version)
    }

    pub(super) fn title(&self) -> String {
        format!("{} v{}", self.kind.title(), self.version)
    }
}

pub(super) fn deploy(hosts: &[Host], network: NetworkId) -> Vec<Ice> {
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
                known: false,
            })
        })
        .collect()
}

impl World {
    pub(super) fn ice_line(&self, host: HostId, viewer: PlayerId) -> Option<Message> {
        let run = self.players.get(&viewer)?.run.as_ref()?;
        let here: Vec<&Ice> = run.ice.iter().filter(|ice| ice.host == host).collect();
        if here.is_empty() {
            return None;
        }
        let known: Vec<&str> = here
            .iter()
            .filter(|ice| ice.known)
            .map(|ice| ice.name.as_str())
            .collect();
        let unknown = here.len() - known.len();
        let mut message = Message::game("ICE: ");
        if !known.is_empty() {
            message = listed(message, Style::Ice, known);
            if unknown > 0 {
                message = message.text(", ");
            }
        }
        if unknown > 0 {
            message = message
                .text(format!("{unknown} unknown. Type "))
                .span(Style::Command, "scan")
                .text(" to see them.");
        }
        Some(message)
    }
}
