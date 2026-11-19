use ttymud_text::{Kind, Message, Style};

use super::run::MAX_TRACE;
use super::{Host, HostId, NetworkId, PlayerId, World};
use crate::ice::{Action, IceKind};

const ALERT_TRACE: u32 = 10;

#[derive(Debug)]
pub(super) struct Ice {
    pub(super) host: HostId,
    pub(super) name: String,
    pub(super) kind: IceKind,
    pub(super) version: u32,
    pub(super) integrity: u32,
    pub(super) guards: Option<HostId>,
    pub(super) known: bool,
    pub(super) awake: bool,
    pub(super) intent: Option<Intent>,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct Intent {
    action: Action,
    ticks: u32,
}

impl Ice {
    pub(super) fn broken(&self) -> bool {
        self.integrity == 0
    }

    fn named(&self) -> Message {
        let message = Message::new(Kind::Telegraph);
        if self.known {
            message.span(Style::Ice, &self.name)
        } else {
            message.text("Unknown ICE")
        }
    }

    pub(super) fn compile(&mut self, action: Action) -> Message {
        let ticks = action.ticks();
        self.intent = Some(Intent { action, ticks });
        let unit = if ticks == 1 { "tick" } else { "ticks" };
        self.named()
            .text(" starts to compile ")
            .span(Style::Danger, action.name())
            .text(format!(" ({ticks} {unit})."))
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
                awake: false,
                intent: None,
            })
        })
        .collect()
}

impl World {
    pub(super) fn tick_ice(&mut self, player: PlayerId) -> Vec<Message> {
        let here = self.here(player);
        let Some(run) = self
            .players
            .get_mut(&player)
            .and_then(|state| state.run.as_mut())
        else {
            return Vec::new();
        };
        let mut shown = Vec::new();
        let mut fired = Vec::new();
        for (index, ice) in run.ice.iter_mut().enumerate() {
            if ice.broken() {
                continue;
            }
            if ice.kind == IceKind::Hound && ice.awake && ice.host != here {
                ice.host = here;
                shown.push(ice.named().text(" follows you."));
            }
            let Some(intent) = &mut ice.intent else {
                continue;
            };
            intent.ticks = intent.ticks.saturating_sub(1);
            if intent.ticks == 0 {
                fired.push((index, intent.action));
                ice.intent = None;
            }
        }
        for (index, action) in fired {
            if !self.running(player) {
                break;
            }
            shown.extend(self.fire(player, index, action));
        }
        shown
    }

    fn running(&self, player: PlayerId) -> bool {
        self.players
            .get(&player)
            .is_some_and(|state| state.run.is_some())
    }

    fn fire(&mut self, player: PlayerId, index: usize, action: Action) -> Vec<Message> {
        let here = self.here(player);
        let Some(run) = self
            .players
            .get_mut(&player)
            .and_then(|state| state.run.as_mut())
        else {
            return Vec::new();
        };
        let source = run.ice[index].named();
        match action {
            Action::Alert => {
                let alert = source
                    .text(" sends an ")
                    .span(Style::Danger, "ALERT")
                    .text(".");
                let mut woken = Vec::new();
                for ice in &mut run.ice {
                    if ice.kind == IceKind::Hound && !ice.awake && !ice.broken() {
                        ice.awake = true;
                        ice.host = here;
                        woken.push(ice.named().text(" wakes up and hunts you."));
                    }
                }
                if !woken.is_empty() {
                    return std::iter::once(alert).chain(woken).collect();
                }
                let before = run.trace;
                let after = (before + ALERT_TRACE).min(MAX_TRACE);
                let mut shown = vec![alert.text(format!(" Trace: {before}% -> {after}%."))];
                shown.extend(self.raise_trace(player, ALERT_TRACE));
                shown
            }
        }
    }

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

    #[test]
    fn noise_near_a_sentry_starts_an_alert() {
        let shown = at_gate(&["scan"]);
        assert_eq!(
            shown.last().map(String::as_str),
            Some("sentry starts to compile ALERT (3 ticks).")
        );
    }

    #[test]
    fn an_alert_wakes_the_hounds() {
        assert_eq!(at_gate(&["scan", "~2"]), Vec::<String>::new());
        assert_eq!(
            at_gate(&["scan", "~3"]),
            [
                "sentry sends an ALERT.",
                "Unknown ICE wakes up and hunts you."
            ]
        );
        assert_eq!(
            at_gate(&["scan", "~3", "look"]).last().map(String::as_str),
            Some("ICE: sentry, warden, 1 unknown. Type scan to see them.")
        );
    }

    #[test]
    fn a_hunting_hound_follows_you() {
        assert_eq!(
            at_gate(&["scan", "~3", "go core", "~"]),
            ["Unknown ICE follows you."]
        );
    }

    #[test]
    fn a_second_alert_raises_the_trace() {
        let shown = at_gate(&["scan", "~3", "scan", "~3"]);
        assert_eq!(shown, ["sentry sends an ALERT. Trace: 8% -> 18%."]);
    }
}
