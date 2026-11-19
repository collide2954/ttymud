use std::collections::BTreeMap;

use ttymud_text::{Kind, Message, Style};

use crate::content::{Content, File};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PlayerId(u64);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Output {
    pub player: PlayerId,
    pub message: Message,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct HostId(usize);

#[derive(Debug)]
struct Host {
    id: String,
    name: String,
    description: String,
    routes: Vec<HostId>,
    files: Vec<File>,
}

#[derive(Debug)]
struct Player {
    name: String,
    chain: Vec<HostId>,
}

#[derive(Debug)]
pub struct World {
    hosts: Vec<Host>,
    start: HostId,
    players: BTreeMap<PlayerId, Player>,
    next_player: u64,
}

impl World {
    #[must_use]
    pub fn new(content: Content) -> Self {
        let ids: Vec<String> = content.hosts.iter().map(|host| host.id.clone()).collect();
        let find = |id: &str| HostId(ids.iter().position(|known| known == id).unwrap_or(0));
        let mut hosts = Vec::with_capacity(content.hosts.len());
        let mut links = Vec::new();
        for (index, def) in content.hosts.into_iter().enumerate() {
            links.extend(
                def.routes
                    .iter()
                    .map(|target| (HostId(index), find(target))),
            );
            hosts.push(Host {
                id: def.id,
                name: def.name,
                description: def.description,
                routes: Vec::new(),
                files: def.files,
            });
        }
        for (from, to) in links {
            link(&mut hosts, from, to);
            link(&mut hosts, to, from);
        }
        Self {
            start: find(&content.start),
            hosts,
            players: BTreeMap::new(),
            next_player: 0,
        }
    }

    pub fn join(&mut self, name: &str) -> (PlayerId, Vec<Output>) {
        let player = PlayerId(self.next_player);
        self.next_player += 1;
        self.players.insert(
            player,
            Player {
                name: name.to_owned(),
                chain: vec![self.start],
            },
        );
        (player, address(player, self.scene(self.start)))
    }

    #[must_use]
    pub fn prompt(&self, player: PlayerId) -> Option<String> {
        let player = self.players.get(&player)?;
        let here = player.chain.last()?;
        Some(format!("{}@{} $ ", player.name, self.hosts[here.0].id))
    }

    fn scene(&self, id: HostId) -> Vec<Message> {
        let host = &self.hosts[id.0];
        let mut scene = vec![
            Message::new(Kind::Game).span(Style::Title, &host.name),
            Message::game(&host.description),
        ];
        let routes = host
            .routes
            .iter()
            .map(|route| self.hosts[route.0].id.as_str());
        scene.push(listed("Routes: ", Style::Host, routes));
        if !host.files.is_empty() {
            let files = host.files.iter().map(|file| file.name.as_str());
            scene.push(listed("Files: ", Style::File, files));
        }
        scene
    }
}

fn link(hosts: &mut [Host], from: HostId, to: HostId) {
    let routes = &mut hosts[from.0].routes;
    if !routes.contains(&to) {
        routes.push(to);
    }
}

fn address(player: PlayerId, messages: Vec<Message>) -> Vec<Output> {
    messages
        .into_iter()
        .map(|message| Output { player, message })
        .collect()
}

fn listed<'a>(label: &str, style: Style, names: impl Iterator<Item = &'a str>) -> Message {
    let mut message = Message::game(label);
    let mut empty = true;
    for name in names {
        if !empty {
            message = message.text(", ");
        }
        message = message.span(style, name);
        empty = false;
    }
    if empty { message.text("none") } else { message }
}

#[cfg(test)]
mod tests {
    use ttymud_text::Style;

    use super::PlayerId;
    use crate::testing::{plain, world};

    #[test]
    fn joining_shows_the_start_host() {
        let (_, outputs) = world().join("ghost");
        assert_eq!(
            plain(&outputs),
            [
                "The Relay",
                "A crowded public node.",
                "Routes: market, back-room, bazaar",
                "Files: motd.txt, rules.txt",
            ]
        );
    }

    #[test]
    fn outputs_go_to_the_player_who_joined() {
        let mut world = world();
        let (first, _) = world.join("ghost");
        let (second, outputs) = world.join("nyx");
        assert_ne!(first, second);
        assert!(outputs.iter().all(|output| output.player == second));
    }

    #[test]
    fn scene_styles_the_title_and_the_names() {
        let (_, outputs) = world().join("ghost");
        assert_eq!(outputs[0].message.spans[0].style, Style::Title);
        let route = &outputs[2].message.spans[1];
        assert_eq!((route.style, route.text.as_str()), (Style::Host, "market"));
        let file = &outputs[3].message.spans[1];
        assert_eq!((file.style, file.text.as_str()), (Style::File, "motd.txt"));
    }

    #[test]
    fn prompt_shows_the_player_and_the_host() {
        let mut world = world();
        let (player, _) = world.join("ghost");
        assert_eq!(world.prompt(player).as_deref(), Some("ghost@relay $ "));
    }

    #[test]
    fn unknown_players_have_no_prompt() {
        assert_eq!(world().prompt(PlayerId(99)), None);
    }
}
