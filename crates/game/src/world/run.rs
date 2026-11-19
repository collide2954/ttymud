use ttymud_text::Message;

use super::{HostId, NetworkId, PlayerId, World};

#[derive(Debug)]
pub(super) struct Run {
    pub(super) network: NetworkId,
    pub(super) trace: u32,
}

impl World {
    pub(super) fn cross(&mut self, player: PlayerId, from: HostId, to: HostId) -> Option<Message> {
        let state = self.players.get_mut(&player)?;
        match (self.hosts[from.0].network, self.hosts[to.0].network) {
            (None, Some(network)) => {
                state.run = Some(Run { network, trace: 0 });
                let name = &self.networks[network.0].name;
                Some(Message::game(format!(
                    "You connect to the network of {name}."
                )))
            }
            (Some(network), None) => {
                state.run = None;
                let name = &self.networks[network.0].name;
                Some(Message::game(format!(
                    "You leave the network of {name}. Clean exit."
                )))
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::testing::walk;

    #[test]
    fn entering_a_network_starts_a_run() {
        let (shown, prompt) = walk(&["go back-room", "go gate"]);
        assert_eq!(
            shown[..2],
            ["You connect to the network of Test Lab.", "Lab Gate"]
        );
        assert_eq!(
            prompt.as_deref(),
            Some("[trace 0% integ 100] ghost@gate $ ")
        );
    }

    #[test]
    fn status_shows_the_trace_during_a_run() {
        let (shown, _) = walk(&["go back-room", "go gate", "status"]);
        assert_eq!(shown[0], "Trace: 0% in Test Lab");
        assert_eq!(walk(&["status"]).0[0], "Integrity: 100/100");
    }

    #[test]
    fn the_run_goes_on_inside_the_network() {
        let (shown, prompt) = walk(&["go back-room", "go gate", "go core"]);
        assert_eq!(shown[0], "Lab Core");
        assert_eq!(
            prompt.as_deref(),
            Some("[trace 0% integ 100] ghost@core $ ")
        );
    }

    #[test]
    fn leaving_through_the_entry_is_a_clean_exit() {
        let (shown, prompt) = walk(&["go back-room", "go gate", "back"]);
        assert_eq!(
            shown[..2],
            [
                "You leave the network of Test Lab. Clean exit.",
                "The Back Room"
            ]
        );
        assert_eq!(prompt.as_deref(), Some("ghost@back-room $ "));
    }

    #[test]
    fn going_to_the_hub_by_name_is_a_clean_exit_too() {
        let lines = ["go back-room", "go gate", "go core", "go gate", "go back"];
        let (shown, _) = walk(&lines);
        assert_eq!(shown[0], "You leave the network of Test Lab. Clean exit.");
    }
}
