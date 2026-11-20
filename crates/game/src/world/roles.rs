use ttymud_text::{Kind, Message, Style};

use super::run::Run;
use super::{PlayerId, Step, World};
use crate::commands::Item;
use crate::ice::IceKind;

const MASK_TICKS: u32 = 5;

impl World {
    fn run_mut(&mut self, player: PlayerId) -> Option<&mut Run> {
        self.players.get_mut(&player)?.run.as_mut()
    }

    pub(super) fn firewall(&mut self, player: PlayerId) -> Result<Step, Message> {
        let Some(run) = self.run_mut(player) else {
            return Ok(Step::default());
        };
        if run.firewall {
            return Err(Message::error("Your firewall is already up."));
        }
        run.firewall = true;
        Ok(Step::show(vec![Message::game(
            "firewall is up. It stops the next attack on your integrity.",
        )]))
    }

    pub(super) fn mask(&mut self, player: PlayerId) -> Step {
        let Some(run) = self.run_mut(player) else {
            return Step::default();
        };
        run.mask = MASK_TICKS;
        Step::show(vec![Message::game(format!(
            "mask hides your noise for {MASK_TICKS} ticks."
        ))])
    }

    pub(super) fn proxy(&mut self, player: PlayerId) -> Result<Step, Message> {
        let Some(run) = self.run_mut(player) else {
            return Ok(Step::default());
        };
        if run.proxy {
            return Err(Message::error("Your link already goes through a proxy."));
        }
        run.proxy = true;
        Ok(Step::show(vec![Message::game(
            "Your link goes through a proxy. ICE attacks on your trace do half damage in this run.",
        )]))
    }

    pub(super) fn pretext(
        &mut self,
        player: PlayerId,
        args: &[String],
        input: Option<Vec<Item>>,
    ) -> Result<Step, Message> {
        if let ([word], None) = (args, &input)
            && let Some(fooled) = self.fool(player, word)
        {
            return fooled;
        }
        let targets = self.targets(player, "pretext", args, input)?;
        let [(name, _)] = targets.as_slice() else {
            return Err(Message::error("pretext takes one warden at a time."));
        };
        let here = self.here(player);
        let Some(ice) = self.instance_mut(player).and_then(|instance| {
            instance
                .ice
                .iter_mut()
                .find(|ice| ice.host == here && &ice.name == name)
        }) else {
            return Ok(Step::default());
        };
        let named = |rest: &str| {
            Message::new(Kind::Error)
                .span(Style::Ice, name)
                .text(rest.to_owned())
        };
        if ice.kind != IceKind::Warden {
            return Err(Message::error("pretext works only on a warden."));
        }
        if ice.broken() {
            return Err(named(" is already broken."));
        }
        if ice.fooled {
            return Err(named(" already lets you pass."));
        }
        ice.fooled = true;
        let guarded = ice.guards;
        let mut message = Message::game("You call the help desk and talk your way past ")
            .span(Style::Ice, name)
            .text(".");
        if let Some(target) = guarded {
            message = message
                .text(" The route to ")
                .span(Style::Host, &self.hosts[target.0].id)
                .text(" is open for you.");
        }
        Ok(Step::show(vec![message]))
    }
}

#[cfg(test)]
mod tests {
    use crate::ice::{Action, IceKind};
    use crate::testing::{joined, plain, step, walk};

    fn with(program: &str, lines: &[&str]) -> (Vec<String>, Option<String>) {
        let load = format!("load {program}");
        let start = ["go back-room", "unload patch", load.as_str()];
        walk(&[&start[..], lines].concat())
    }

    #[test]
    fn a_firewall_stops_the_next_attack_on_integrity() {
        let lines = [
            "go gate",
            "go core",
            "go archive",
            "firewall",
            "~",
            "scan",
            "~3",
            "status | grep integrity",
        ];
        let (shown, _) = with("firewall", &lines);
        assert_eq!(shown, ["Integrity: 100/100"]);
        let (shown, _) = with("firewall", &lines[..7]);
        assert!(
            shown
                .iter()
                .any(|line| line.ends_with("Your firewall stops it.")),
            "{shown:?}"
        );
    }

    #[test]
    fn only_one_firewall_can_be_up() {
        let (shown, _) = with("firewall", &["go gate", "firewall", "~3", "firewall"]);
        assert_eq!(shown, ["Your firewall is already up."]);
    }

    #[test]
    fn a_mask_hides_noise_from_sentries_and_halves_it() {
        let (shown, prompt) = with("mask", &["go gate", "mask", "~2", "scan"]);
        assert!(
            !shown.iter().any(|line| line.contains("ALERT")),
            "{shown:?}"
        );
        assert_eq!(
            prompt.as_deref(),
            Some("[trace 1% integ 100] ghost@gate $ ")
        );
    }

    #[test]
    fn a_mask_drops_after_five_ticks() {
        let (shown, _) = with("mask", &["go gate", "mask", "~5"]);
        assert_eq!(shown, ["Your mask drops."]);
    }

    #[test]
    fn a_proxy_halves_ice_attacks_on_the_trace() {
        let (mut world, player) = joined();
        let lines = [
            "go back-room",
            "unload patch",
            "load proxy",
            "go gate",
            "proxy",
        ];
        for line in lines {
            world.command(player, line);
        }
        if let Some(run) = world
            .players
            .get_mut(&player)
            .and_then(|state| state.run.as_mut())
        {
            run.trace = 0;
        }
        if let Some(instance) = world.instance_mut(player) {
            let gate = instance.ice[0].host;
            for ice in &mut instance.ice {
                ice.intent = None;
                if ice.kind == IceKind::Hound {
                    ice.awake = true;
                    ice.host = gate;
                    ice.target = Some(player);
                    let _ = ice.compile(Action::TraceBurst);
                }
            }
        }
        let shown = plain(&step(&mut world, player, "~2"));
        assert_eq!(shown[0], "Unknown ICE fires TRACE BURST. Trace: 0% -> 4%.");
    }

    #[test]
    fn pretext_talks_past_a_warden() {
        let lines = ["go gate", "scan", "~2", "pretext warden"];
        let (shown, _) = with("pretext", &lines);
        assert_eq!(
            shown[0],
            "You call the help desk and talk your way past warden. The route to vault is open for you."
        );
        let lines = [&lines[..], &["go vault"]].concat();
        assert_eq!(with("pretext", &lines).0[0], "Lab Vault");
    }

    #[test]
    fn pretext_works_only_on_a_warden() {
        let (shown, _) = with("pretext", &["go gate", "scan", "~2", "pretext sentry"]);
        assert_eq!(shown[0], "pretext works only on a warden.");
    }
}
