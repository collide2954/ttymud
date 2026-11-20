use std::collections::BTreeMap;

use ttymud_text::{Kind, Message, Style, sanitize};

use super::account::{Phase, good_name};
use super::news::Headline;
use super::{Output, PlayerId, Step, World, lines};

const MAX_CITY: usize = 40;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outgoing {
    Chat { handle: String, text: String },
    News { when: u64, text: String },
}

#[derive(Debug, Default)]
pub(super) struct Longline {
    pub(super) peers: Vec<Peer>,
    outgoing: Vec<Outgoing>,
    mutes: BTreeMap<String, u64>,
}

#[derive(Debug)]
pub(super) struct Peer {
    pub(super) name: String,
    city: Option<String>,
    pub(super) headlines: Vec<Headline>,
}

pub(super) fn is_remote(key: &str) -> bool {
    key.find('@').is_some_and(|at| at > 0)
}

fn remote_key(handle: &str, peer: &str) -> String {
    format!("{}@{peer}", handle.to_ascii_lowercase())
}

impl World {
    pub fn set_peers(&mut self, names: Vec<String>) {
        self.longline.peers = names
            .into_iter()
            .map(|name| Peer {
                name,
                city: None,
                headlines: Vec::new(),
            })
            .collect();
    }

    #[must_use = "the server sends these to the players"]
    pub fn peer_up(&mut self, name: &str, city: &str) -> Vec<Output> {
        let city: String = sanitize(city).trim().chars().take(MAX_CITY).collect();
        let Some(peer) = self.peer_mut(name) else {
            return Vec::new();
        };
        peer.city = Some(city.clone());
        peer.headlines.clear();
        self.tell_linked(&format!("The link to {name} ({city}) is up."))
    }

    #[must_use = "the server sends these to the players"]
    pub fn peer_down(&mut self, name: &str) -> Vec<Output> {
        match self.peer_mut(name).filter(|peer| peer.city.is_some()) {
            Some(peer) => {
                peer.city = None;
                self.tell_linked(&format!("The link to {name} is down."))
            }
            None => Vec::new(),
        }
    }

    #[must_use = "the server sends these to a new linked city"]
    pub fn latest_news(&self) -> Vec<Outgoing> {
        let latest = self.latest_headlines().into_iter();
        latest
            .map(|headline| Outgoing::News {
                when: headline.when,
                text: headline.text,
            })
            .collect()
    }

    #[must_use = "the server sends these to the linked cities"]
    pub fn outgoing(&mut self) -> Vec<Outgoing> {
        std::mem::take(&mut self.longline.outgoing)
    }

    #[must_use = "the server sends these to the players"]
    pub fn peer_chat(&mut self, peer: &str, handle: &str, text: &str) -> Vec<Output> {
        let text = sanitize(text);
        let text = text.trim();
        let known = self.peer(peer).is_some();
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

    pub(super) fn peer(&self, name: &str) -> Option<&Peer> {
        self.longline.peers.iter().find(|peer| peer.name == name)
    }

    fn peer_mut(&mut self, name: &str) -> Option<&mut Peer> {
        self.longline
            .peers
            .iter_mut()
            .find(|peer| peer.name == name)
    }

    pub(super) fn find_peer(&self, word: &str) -> Result<&Peer, Message> {
        let name = word.to_ascii_lowercase();
        self.peer(&name)
            .ok_or_else(|| Message::error(format!("No linked city named {name}.")))
    }

    fn tell_linked(&self, text: &str) -> Vec<Output> {
        let Some(id) = self.linked_channel() else {
            return Vec::new();
        };
        let message = Message::new(Kind::Chat).text(format!("[{id}] {text}"));
        let members = self
            .players
            .iter()
            .filter(|(_, state)| state.phase == Phase::Playing && state.channels.contains(&id));
        members
            .map(|(&player, _)| Output {
                player,
                message: message.clone(),
            })
            .collect()
    }

    pub(super) fn send_out(&mut self, outgoing: Outgoing) {
        if !self.longline.peers.is_empty() {
            self.longline.outgoing.push(outgoing);
        }
    }

    pub(super) fn cities(&self) -> Step {
        let peers = &self.longline.peers;
        if peers.is_empty() {
            return lines(vec![Message::game(
                "This city has no links to other cities.",
            )]);
        }
        let mut shown: Vec<Message> = peers
            .iter()
            .map(|peer| {
                let line = Message::game("").span(Style::Host, peer.name.as_str());
                match &peer.city {
                    Some(city) => line.text(format!(": {city}. The link is up.")),
                    None => line.text(": The link is down."),
                }
            })
            .collect();
        if let Some(id) = self.linked_channel() {
            shown.push(
                Message::game("Type ")
                    .span(Style::Command, format!("#{id} <text>"))
                    .text(" to talk with the linked cities. Type ")
                    .span(Style::Command, "news <city>")
                    .text(" to read their news."),
            );
        }
        lines(shown)
    }

    pub(super) fn remote_person(&self, word: &str) -> Result<(String, String), Message> {
        let (handle, peer) = word.split_once('@').unwrap_or((word, ""));
        let peer = self.find_peer(peer)?.name.clone();
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

    fn news(when: u64, text: &str) -> Outgoing {
        Outgoing::News {
            when,
            text: text.to_owned(),
        }
    }

    #[test]
    fn headlines_go_out_to_the_linked_cities() {
        let (mut world, _, _) = linked();
        world.publish("Acme Corp patches a hole in Gatekeep VPN.".to_owned());
        assert_eq!(
            world.outgoing(),
            [news(60, "Acme Corp patches a hole in Gatekeep VPN.")]
        );
    }

    #[test]
    fn a_new_link_gets_the_latest_headlines_and_the_channel_hears_of_it() {
        let (mut world, nyx, zer0) = linked();
        for number in 1..=12 {
            world.publish(format!("Headline {number}."));
        }
        let _ = world.outgoing();
        let expected: Vec<Outgoing> = (3..=12)
            .map(|number| news(60, &format!("Headline {number}.")))
            .collect();
        assert_eq!(world.latest_news(), expected);
        let outputs = world.peer_up("night-owl", "Port \u{1b}Ostra");
        assert_eq!(
            heard(&outputs, nyx),
            ["[longline] The link to night-owl (Port Ostra) is up."]
        );
        assert_eq!(heard(&outputs, zer0), Vec::<String>::new());
        assert_eq!(
            heard(&world.peer_down("night-owl"), nyx),
            ["[longline] The link to night-owl is down."]
        );
        assert_eq!(world.peer_down("night-owl"), []);
        assert_eq!(world.peer_up("dockside", "Dockside"), []);
    }

    #[test]
    fn cities_shows_the_links_and_their_state() {
        assert_eq!(
            crate::testing::run("cities"),
            ["This city has no links to other cities."]
        );
        let (mut world, nyx, _) = linked();
        world.set_peers(vec!["night-owl".to_owned(), "dockside".to_owned()]);
        let _ = world.peer_up("night-owl", "Port Ostra");
        assert_eq!(
            plain(&world.command(nyx, "cities")),
            [
                "night-owl: Port Ostra. The link is up.",
                "dockside: The link is down.",
                "Type #longline <text> to talk with the linked cities. Type news <city> to read their news.",
            ]
        );
        assert!(plain(&world.command(nyx, "?"))[0].contains("cities"));
        assert!(!crate::testing::run("?")[0].contains("cities"));
    }

    #[test]
    fn news_shows_the_headlines_of_a_linked_city() {
        let (mut world, nyx, _) = linked();
        world.set_peers(vec!["night-owl".to_owned(), "dockside".to_owned()]);
        let _ = world.peer_up("night-owl", "Port Ostra");
        world.peer_news(
            "night-owl",
            30,
            "Kestrel Health loses patients.db in a breach.",
        );
        world.peer_news("night-owl", 9_999, "A headline \u{1b}[2Jfrom the future.");
        world.peer_news("night-owl", 40, " \u{7} ");
        let mut said = |line: &str| plain(&world.command(nyx, line));
        assert_eq!(
            said("news NIGHT-OWL"),
            [
                "1970-01-01 00:01 A headline [2Jfrom the future.",
                "1970-01-01 00:00 Kestrel Health loses patients.db in a breach.",
            ]
        );
        assert_eq!(said("news dockside"), ["No news from dockside yet."]);
        assert_eq!(said("news nowhere"), ["No linked city named nowhere."]);
        assert_eq!(said("news a b"), ["Usage: news [city]."]);
        for number in 1..=12 {
            world.peer_news("night-owl", 50, &format!("Item {number}."));
        }
        let shown = plain(&world.command(nyx, "news night-owl"));
        assert_eq!(shown.len(), 10);
        assert_eq!(shown[0], "1970-01-01 00:00 Item 12.");
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
