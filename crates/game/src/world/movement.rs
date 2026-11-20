use ttymud_text::Message;

use super::{HostId, PlayerId, Step, Thing, World, choose, usage};
use crate::commands::Item;
use crate::contract::Event;

impl World {
    pub(super) fn go(
        &mut self,
        player: PlayerId,
        args: &[String],
        input: Option<Vec<Item>>,
    ) -> Result<Step, Message> {
        let place = match (args, input) {
            ([place], None) if place == ".." => return self.back(player),
            ([place], None) => place.clone(),
            ([], Some(items)) => piped_route(items)?,
            ([], None) => {
                let routes = self.route_names(self.here(player), player);
                return Err(usage("go <place>", Thing::Route, &routes));
            }
            (_, Some(_)) => {
                return Err(Message::error("go takes a place or a pipe, not both."));
            }
            (_, None) => return Err(Message::error("Usage: go <place>.")),
        };
        let from = self.here(player);
        let target = self.route(player, &place)?;
        self.may_enter(player, target)?;
        self.guard(player, target)?;
        self.move_to(player, target);
        Ok(self.arrive(player, from, target))
    }

    pub(super) fn back(&mut self, player: PlayerId) -> Result<Step, Message> {
        let from = self.here(player);
        let behind = self.players.get(&player).and_then(|state| {
            let index = state.chain.len().checked_sub(2)?;
            state.chain.get(index).copied()
        });
        if let Some(behind) = behind
            && self.vault_hop(player, from, behind)
        {
            let id = &self.hosts[behind.0].id;
            return Err(Message::error(format!(
                "There is no route back to {id} now."
            )));
        }
        match self.players.get_mut(&player) {
            Some(state) if state.chain.len() > 1 => {
                state.chain.pop();
            }
            _ => return Err(Message::error("There is no hop behind you.")),
        }
        let here = self.here(player);
        Ok(self.arrive(player, from, here))
    }

    fn arrive(&mut self, player: PlayerId, from: HostId, to: HostId) -> Step {
        self.depart(player, from, to);
        let mut shown = self.cross(player, from, to);
        self.appear(player, to);
        if self.home == Some(to) {
            self.advance(player, Event::Home);
        }
        shown.extend(self.scene(to, player));
        Step::show(shown)
    }

    fn route(&self, player: PlayerId, place: &str) -> Result<HostId, Message> {
        let here = self.here(player);
        let routes = self.routes(here, player);
        let names = self.route_names(here, player);
        let name = choose(Thing::Route, place, &names)?;
        let index = names.iter().position(|&known| known == name);
        Ok(index.map_or(here, |index| routes[index]))
    }

    fn move_to(&mut self, player: PlayerId, target: HostId) {
        if let Some(state) = self.players.get_mut(&player) {
            match state.chain.iter().position(|&hop| hop == target) {
                Some(index) => state.chain.truncate(index + 1),
                None => state.chain.push(target),
            }
        }
    }
}

fn piped_route(items: Vec<Item>) -> Result<String, Message> {
    let mut routes: Vec<String> = items
        .into_iter()
        .filter_map(|item| match item {
            Item::Route(id) => Some(id),
            Item::File(_) | Item::Ice { .. } | Item::Line(_) => None,
        })
        .collect();
    if routes.len() == 1 {
        return Ok(routes.remove(0));
    }
    let count = routes.len();
    Err(Message::error(format!(
        "go needs one route, but the input has {count}."
    )))
}

#[cfg(test)]
mod tests {
    use crate::testing::walk;

    #[test]
    fn go_moves_along_a_route_and_shows_the_host() {
        let (shown, prompt) = walk(&["go market"]);
        assert_eq!(
            shown,
            [
                "The Night Market",
                "Stalls sell cracked firmware.",
                "Routes: relay, bazaar",
                "Files: prices.db",
                "There is a shop here. Type shop to see what it sells.",
            ]
        );
        assert_eq!(prompt.as_deref(), Some("ghost@market $ "));
    }

    #[test]
    fn cd_connect_and_ssh_also_move() {
        for verb in ["cd", "connect", "ssh"] {
            let (_, prompt) = walk(&[&format!("{verb} market")]);
            assert_eq!(prompt.as_deref(), Some("ghost@market $ "), "{verb}");
        }
    }

    #[test]
    fn a_unique_prefix_is_enough() {
        assert_eq!(walk(&["go m"]).1.as_deref(), Some("ghost@market $ "));
        assert_eq!(walk(&["go back"]).1.as_deref(), Some("ghost@back-room $ "));
    }

    #[test]
    fn an_ambiguous_prefix_fails() {
        assert_eq!(walk(&["go ba"]).0, ["ba could mean back-room or bazaar."]);
    }

    #[test]
    fn a_misspelled_place_gets_a_suggestion() {
        assert_eq!(
            walk(&["go markte"]).0,
            ["No route to markte. Did you mean market?"]
        );
    }

    #[test]
    fn an_unknown_place_lists_the_routes() {
        assert_eq!(
            walk(&["go vault"]).0,
            ["No route to vault. Routes: market, back-room, bazaar"]
        );
    }

    #[test]
    fn go_needs_a_place() {
        assert_eq!(
            walk(&["go"]).0,
            ["Usage: go <place>. Routes: market, back-room, bazaar"]
        );
    }

    #[test]
    fn back_returns_one_hop() {
        let (shown, prompt) = walk(&["go market", "back"]);
        assert_eq!(shown[0], "The Relay");
        assert_eq!(prompt.as_deref(), Some("ghost@relay $ "));
    }

    #[test]
    fn exit_and_cd_dot_dot_also_go_back() {
        for line in ["exit", "cd .."] {
            let (_, prompt) = walk(&["go market", line]);
            assert_eq!(prompt.as_deref(), Some("ghost@relay $ "), "{line}");
        }
    }

    #[test]
    fn back_at_the_start_fails() {
        assert_eq!(walk(&["back"]).0, ["There is no hop behind you."]);
    }

    #[test]
    fn returning_to_an_earlier_hop_shortens_the_path() {
        let (shown, _) = walk(&["go market", "go bazaar", "go relay", "back"]);
        assert_eq!(shown, ["There is no hop behind you."]);
    }

    #[test]
    fn go_takes_a_piped_route() {
        let (shown, _) = walk(&["ls | grep baz | go"]);
        assert_eq!(shown[0], "The Bazaar");
    }

    #[test]
    fn go_needs_exactly_one_piped_route() {
        assert_eq!(
            walk(&["ls | grep ba | go"]).0,
            ["go needs one route, but the input has 2."]
        );
        assert_eq!(
            walk(&["ls | grep txt | go"]).0,
            ["go needs one route, but the input has 0."]
        );
    }

    #[test]
    fn go_takes_a_place_or_a_pipe_but_not_both() {
        assert_eq!(
            walk(&["ls | grep baz | go market"]).0,
            ["go takes a place or a pipe, not both."]
        );
    }

    #[test]
    fn movement_gives_no_output_to_a_pipe() {
        assert_eq!(
            walk(&["go market | ls"]).0,
            ["go gives no output to a pipe."]
        );
    }
}
