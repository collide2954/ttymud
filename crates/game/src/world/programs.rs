use ttymud_text::{Kind, Message, Style};

use super::{PlayerId, Step, World};
use crate::commands::Item;

impl World {
    pub(super) fn scan(&mut self, player: PlayerId) -> Result<Step, Message> {
        let here = self.here(player);
        let run = self
            .players
            .get_mut(&player)
            .and_then(|state| state.run.as_mut())
            .ok_or_else(|| Message::error("scan works only in a target network."))?;
        let mut step = Step::default();
        for ice in run.ice.iter_mut().filter(|ice| ice.host == here) {
            ice.known = true;
            let state = format!(
                ": {}, integrity {}/{}",
                ice.title(),
                ice.integrity,
                ice.max()
            );
            step.shown.push(
                Message::new(Kind::Game)
                    .span(Style::Ice, &ice.name)
                    .text(state),
            );
            step.items.push(Item::Ice(ice.name.clone()));
        }
        if step.shown.is_empty() {
            step.shown.push(Message::game("scan finds no ICE here."));
        }
        Ok(step)
    }
}

#[cfg(test)]
mod tests {
    use crate::testing::{run, walk};

    const GATE: [&str; 2] = ["go back-room", "go gate"];
    const UNKNOWN: &str = "ICE: 2 unknown. Type scan to see them.";

    fn at_gate(lines: &[&str]) -> Vec<String> {
        walk(&[&GATE[..], lines].concat()).0
    }

    fn last(lines: &[String]) -> Option<&str> {
        lines.last().map(String::as_str)
    }

    #[test]
    fn look_counts_the_ice_that_you_do_not_know() {
        assert_eq!(last(&at_gate(&["look"])), Some(UNKNOWN));
    }

    #[test]
    fn scan_shows_the_ice_on_this_host() {
        assert_eq!(
            at_gate(&["scan"]),
            [
                "sentry: Sentry v1, integrity 20/20",
                "warden: Warden v1, integrity 40/40",
            ]
        );
    }

    #[test]
    fn look_names_the_ice_after_a_scan() {
        assert_eq!(
            last(&at_gate(&["scan", "look"])),
            Some("ICE: sentry, warden")
        );
    }

    #[test]
    fn scan_pipes_the_ice() {
        assert_eq!(at_gate(&["scan | grep war"]), ["warden"]);
    }

    #[test]
    fn scan_without_ice_says_so() {
        assert_eq!(at_gate(&["go core", "scan"]), ["scan finds no ICE here."]);
    }

    #[test]
    fn scan_works_only_in_a_target_network() {
        assert_eq!(run("scan"), ["scan works only in a target network."]);
    }

    #[test]
    fn a_new_run_starts_with_unknown_ice() {
        assert_eq!(last(&at_gate(&["scan", "back", "go gate"])), Some(UNKNOWN));
    }
}
