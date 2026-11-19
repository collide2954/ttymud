use ttymud_text::{Kind, Message, Style};

use super::{HostId, Output, PlayerId, World};

impl World {
    pub(super) fn others(
        &self,
        host: HostId,
        except: PlayerId,
    ) -> impl Iterator<Item = (PlayerId, &str)> {
        self.players
            .iter()
            .filter(move |&(&id, state)| id != except && state.chain.last() == Some(&host))
            .map(|(&id, state)| (id, state.name.as_str()))
    }

    pub(super) fn notify(&mut self, host: HostId, except: PlayerId, message: &Message) {
        let targets: Vec<PlayerId> = self.others(host, except).map(|(id, _)| id).collect();
        self.outbox.extend(targets.into_iter().map(|player| Output {
            player,
            message: message.clone(),
        }));
    }

    pub(super) fn relocate(&mut self, player: PlayerId, from: HostId, to: HostId) {
        let Some(name) = self.players.get(&player).map(|state| state.name.clone()) else {
            return;
        };
        let destination = self.hosts[to.0].id.clone();
        let leaves = named(&name, &format!(" leaves for {destination}."));
        self.notify(from, player, &leaves);
        self.notify(to, player, &named(&name, " arrives."));
    }
}

pub(super) fn named(name: &str, rest: &str) -> Message {
    Message::new(Kind::Game)
        .span(Style::Player, name)
        .text(rest)
}

#[cfg(test)]
mod tests {
    use ttymud_text::Style;

    use crate::testing::{plain, world};
    use crate::{Output, PlayerId, World};

    fn to(outputs: &[Output], player: PlayerId) -> Vec<String> {
        outputs
            .iter()
            .filter(|output| output.player == player)
            .map(|output| output.message.plain())
            .collect()
    }

    fn two() -> (World, PlayerId, PlayerId) {
        let mut world = world();
        let (ghost, _) = world.join("ghost");
        let (nyx, _) = world.join("nyx");
        (world, ghost, nyx)
    }

    #[test]
    fn others_see_a_player_connect() {
        let mut world = world();
        let (ghost, _) = world.join("ghost");
        let (_, outputs) = world.join("nyx");
        assert_eq!(to(&outputs, ghost), ["nyx connects."]);
    }

    #[test]
    fn look_shows_who_else_is_here() {
        let (mut world, ghost, _) = two();
        let shown = plain(&world.command(ghost, "look"));
        assert_eq!(shown.last().map(String::as_str), Some("Here: nyx"));
    }

    #[test]
    fn players_see_others_leave_and_arrive() {
        let (mut world, ghost, nyx) = two();
        assert_eq!(
            to(&world.command(nyx, "go market"), ghost),
            ["nyx leaves for market."]
        );
        assert_eq!(
            to(&world.command(ghost, "go market"), nyx),
            ["ghost arrives."]
        );
    }

    #[test]
    fn moving_back_is_seen_too() {
        let (mut world, ghost, nyx) = two();
        world.command(nyx, "go market");
        assert_eq!(to(&world.command(nyx, "back"), ghost), ["nyx arrives."]);
    }

    #[test]
    fn players_elsewhere_see_nothing() {
        let (mut world, ghost, nyx) = two();
        world.command(ghost, "go back-room");
        assert_eq!(
            to(&world.command(nyx, "go market"), ghost),
            Vec::<String>::new()
        );
    }

    #[test]
    fn others_see_a_player_disconnect() {
        let (mut world, ghost, nyx) = two();
        assert_eq!(to(&world.leave(nyx), ghost), ["nyx disconnects."]);
    }

    #[test]
    fn a_player_who_left_gets_nothing() {
        let (mut world, _, nyx) = two();
        world.leave(nyx);
        assert_eq!(world.command(nyx, "look"), Vec::<Output>::new());
        assert_eq!(world.prompt(nyx), None);
    }

    #[test]
    fn player_names_have_the_player_style() {
        let mut world = world();
        let (ghost, _) = world.join("ghost");
        let (_, outputs) = world.join("nyx");
        let notice = outputs.iter().find(|output| output.player == ghost);
        assert_eq!(
            notice.map(|output| output.message.spans[0].style),
            Some(Style::Player)
        );
    }
}
