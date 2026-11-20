use std::collections::BTreeMap;

use ttymud_text::{Kind, Message, Style, sanitize};

use super::account::{Phase, good_name};
use super::{Output, PlayerId, World};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outgoing {
    Chat { handle: String, text: String },
}

#[derive(Debug, Default)]
pub(super) struct Longline {
    peers: Vec<String>,
    outgoing: Vec<Outgoing>,
    mutes: BTreeMap<String, u64>,
}

pub(super) fn is_remote(key: &str) -> bool {
    key.find('@').is_some_and(|at| at > 0)
}

fn remote_key(handle: &str, peer: &str) -> String {
    format!("{}@{peer}", handle.to_ascii_lowercase())
}

impl World {
    pub fn set_peers(&mut self, names: Vec<String>) {
        self.longline.peers = names;
    }

    #[must_use = "the server sends these to the linked cities"]
    pub fn outgoing(&mut self) -> Vec<Outgoing> {
        std::mem::take(&mut self.longline.outgoing)
    }

    #[must_use = "the server sends these to the players"]
    pub fn peer_chat(&mut self, peer: &str, handle: &str, text: &str) -> Vec<Output> {
        let text = sanitize(text);
        let text = text.trim();
        let known = self.longline.peers.iter().any(|name| name == peer);
        let Some(id) = self.linked_channel().filter(|_| known) else {
            return Vec::new();
        };
        let key = remote_key(handle, peer);
        if !good_name(handle) || text.is_empty() || self.remote_muted_until(&key) > self.now {
            return Vec::new();
        }
        let name = format!("{handle}@{peer}");
        let message = Message::new(Kind::Chat)
            .text(format!("[{id}] <"))
            .span(Style::Player, name.clone())
            .text("> ")
            .text(text);
        let listeners: Vec<PlayerId> = self
            .players
            .iter()
            .filter(|(_, state)| state.phase == Phase::Playing && state.channels.contains(&id))
            .map(|(&player, _)| player)
            .filter(|&player| self.hears_key(player, &key))
            .collect();
        self.remember_line(key, None, format!("#{id}: <{name}> {text}"));
        let outputs = listeners.into_iter().map(|player| Output {
            player,
            message: message.clone(),
        });
        outputs.collect()
    }

    pub(super) fn linked_channel(&self) -> Option<String> {
        let linked = self.channels.iter().find(|channel| channel.linked);
        linked.map(|channel| channel.id.clone())
    }

    pub(super) fn send_out(&mut self, handle: &str, text: &str) {
        if !self.longline.peers.is_empty() {
            self.longline.outgoing.push(Outgoing::Chat {
                handle: handle.to_owned(),
                text: text.to_owned(),
            });
        }
    }

    pub(super) fn remote_person(&self, word: &str) -> Result<(String, String), Message> {
        let (handle, peer) = word.split_once('@').unwrap_or((word, ""));
        let peer = peer.to_ascii_lowercase();
        if !self.longline.peers.contains(&peer) {
            return Err(Message::error(format!("No linked city named {peer}.")));
        }
        if !good_name(handle) {
            return Err(Message::error(format!("No handle named {word}.")));
        }
        Ok((remote_key(handle, &peer), format!("{handle}@{peer}")))
    }

    pub(super) fn remote_muted_until(&self, key: &str) -> u64 {
        self.longline.mutes.get(key).copied().unwrap_or_default()
    }

    pub(super) fn set_remote_muted(&mut self, key: &str, until: u64) {
        if until > self.now {
            self.longline.mutes.insert(key.to_owned(), until);
        } else {
            self.longline.mutes.remove(key);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Outgoing;
    use crate::testing::{hacker, heard, plain, world};
    use crate::{PlayerId, World};

    fn linked() -> (World, PlayerId, PlayerId) {
        let mut world = world();
        world
            .restore("moderators.ini", "[moderator a]\nhandle = nyx\n")
            .unwrap();
        world.set_peers(vec!["night-owl".to_owned()]);
        world.set_time(60);
        let nyx = hacker(&mut world, "a", "nyx");
        let zer0 = hacker(&mut world, "b", "zer0");
        world.command(nyx, "join longline");
        (world, nyx, zer0)
    }

    fn chat(handle: &str, text: &str) -> Outgoing {
        Outgoing::Chat {
            handle: handle.to_owned(),
            text: text.to_owned(),
        }
    }

    #[test]
    fn the_linked_channel_sends_its_messages_to_the_linked_cities() {
        let (mut world, nyx, zer0) = linked();
        let outputs = world.command(nyx, "#longline hi all");
        assert_eq!(heard(&outputs, nyx), ["[longline] <nyx> hi all"]);
        assert_eq!(heard(&outputs, zer0), Vec::<String>::new());
        assert_eq!(world.outgoing(), [chat("nyx", "hi all")]);
        assert_eq!(world.outgoing(), []);
        world.command(nyx, "#city local only");
        assert_eq!(world.outgoing(), []);
    }

    #[test]
    fn without_links_the_linked_channel_stays_in_the_city() {
        let mut world = world();
        let nyx = hacker(&mut world, "a", "nyx");
        world.command(nyx, "join longline");
        world.command(nyx, "#longline anyone?");
        assert_eq!(world.outgoing(), []);
    }

    #[test]
    fn a_message_from_a_linked_city_reaches_the_members_of_the_linked_channel() {
        let (mut world, nyx, zer0) = linked();
        let outputs = world.peer_chat("night-owl", "Vex", "hello \u{1b}[31mostra");
        assert_eq!(
            heard(&outputs, nyx),
            ["[longline] <Vex@night-owl> hello [31mostra"]
        );
        assert_eq!(heard(&outputs, zer0), Vec::<String>::new());
    }

    #[test]
    fn bad_messages_from_linked_cities_go_nowhere() {
        let (mut world, _, _) = linked();
        let bad = [
            ("dockside", "Vex", "hello"),
            ("night-owl", "x", "hello"),
            ("night-owl", "two words", "hello"),
            ("night-owl", "Vex", " \u{1b}\u{7} "),
        ];
        for (peer, handle, text) in bad {
            assert_eq!(world.peer_chat(peer, handle, text), [], "{peer} {handle}");
        }
    }

    #[test]
    fn ignore_and_block_take_a_handle_of_a_linked_city() {
        let (mut world, nyx, _) = linked();
        let mut said = |line: &str| plain(&world.command(nyx, line));
        assert_eq!(
            said("ignore vex@night-owl"),
            ["You ignore vex@night-owl. You no longer see their messages."]
        );
        assert_eq!(
            said("ignore"),
            ["You ignore: vex@night-owl", "You block nobody."]
        );
        assert_eq!(
            said("ignore vex@dockside"),
            ["No linked city named dockside."]
        );
        assert_eq!(said("block x@night-owl"), ["No handle named x@night-owl."]);
        assert_eq!(world.peer_chat("night-owl", "Vex", "hi"), []);
        world.command(nyx, "unignore VEX@night-owl");
        let outputs = world.peer_chat("night-owl", "Vex", "hi");
        assert_eq!(heard(&outputs, nyx), ["[longline] <Vex@night-owl> hi"]);
    }

    #[test]
    fn a_moderator_can_mute_but_not_kick_or_ban_a_handle_of_a_linked_city() {
        let (mut world, nyx, _) = linked();
        assert_eq!(
            plain(&world.command(nyx, "mute vex@night-owl 10")),
            ["You mute vex@night-owl for 10 minutes."]
        );
        assert_eq!(world.peer_chat("night-owl", "Vex", "hi"), []);
        for line in ["kick vex@night-owl", "ban vex@night-owl"] {
            assert_eq!(
                plain(&world.command(nyx, line)),
                ["vex@night-owl is in a linked city. You can only mute a handle of a linked city."],
                "{line}"
            );
        }
        assert_eq!(
            plain(&world.command(nyx, "unmute vex@night-owl")),
            ["You unmute vex@night-owl."]
        );
        assert_eq!(world.peer_chat("night-owl", "Vex", "hi").len(), 1);
        world.command(nyx, "mute vex@night-owl 1");
        world.set_time(121);
        assert_eq!(world.peer_chat("night-owl", "Vex", "hi").len(), 1);
    }

    #[test]
    fn a_report_keeps_the_chat_of_a_handle_of_a_linked_city() {
        let (mut world, nyx, zer0) = linked();
        world.command(zer0, "join longline");
        let _ = world.peer_chat("night-owl", "Vex", "buy cheap ice");
        let outputs = world.command(zer0, "report vex@night-owl spam");
        assert_eq!(
            heard(&outputs, zer0),
            [
                "You report vex@night-owl. The moderators get your report and the chat of vex@night-owl from the last 15 minutes."
            ]
        );
        let report = plain(&world.command(nyx, "reports 1"));
        assert!(
            report
                .contains(&"1970-01-01 00:01 #longline: <Vex@night-owl> buy cheap ice".to_owned()),
            "{report:?}"
        );
    }

    #[test]
    fn private_messages_stay_in_the_city() {
        let (mut world, nyx, _) = linked();
        assert_eq!(
            plain(&world.command(nyx, "tell vex@night-owl hi")),
            ["Private messages stay in this city."]
        );
    }
}
