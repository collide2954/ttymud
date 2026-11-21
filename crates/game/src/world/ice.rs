use ttymud_text::{Kind, Message, Style};

use super::alert::MAX_VERSION;
use super::instance::{InstanceId, decoy_on, host_of, name_of, tell, witnesses};
use super::run::MAX_TRACE;
use super::{Host, HostId, NetworkId, Output, PlayerId, World, address};
use crate::commands;
use crate::ice::{Action, Effect, IceKind};

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
    pub(super) fooled: bool,
    pub(super) intent: Option<Intent>,
    pub(super) target: Option<PlayerId>,
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
        self.compile_in(action, action.ticks())
    }

    pub(super) fn compile_in(&mut self, action: Action, ticks: u32) -> Message {
        self.intent = Some(Intent { action, ticks });
        self.telegraph(None)
    }

    pub(super) fn telegraph(&self, against: Option<&str>) -> Message {
        let Some(Intent { action, ticks }) = self.intent else {
            return self.named();
        };
        let unit = if ticks == 1 { "tick" } else { "ticks" };
        let mut message = self
            .named()
            .text(" starts to compile ")
            .span(Style::Danger, action.name());
        if let Some(name) = against {
            message = message.text(" against ").span(Style::Player, name);
        }
        message.text(format!(" ({ticks} {unit})."))
    }

    pub(super) fn wake(&mut self) -> Message {
        self.awake = true;
        self.named().text(" wakes up.")
    }

    pub(super) fn lose(&mut self) -> Message {
        self.intent = None;
        self.target = None;
        self.named().text(" loses you.")
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

pub(super) fn deploy(hosts: &[Host], network: NetworkId, bonus: u32) -> Vec<Ice> {
    let find = |id: &str| {
        let position = |inside: bool| {
            hosts
                .iter()
                .position(|host| host.id == id && (!inside || host.network == Some(network)))
        };
        position(true).or_else(|| position(false)).map(HostId)
    };
    hosts
        .iter()
        .enumerate()
        .filter(|(_, host)| host.network == Some(network))
        .flat_map(|(index, host)| {
            host.ice.iter().map(move |def| {
                let version = (def.version + bonus).min(MAX_VERSION);
                Ice {
                    host: HostId(index),
                    name: def.name.clone(),
                    kind: def.kind,
                    version,
                    integrity: def.kind.integrity(version),
                    guards: def.guards.as_deref().and_then(find),
                    known: false,
                    awake: false,
                    fooled: false,
                    intent: None,
                    target: None,
                }
            })
        })
        .collect()
}

impl World {
    pub(super) fn tick_run(&mut self, player: PlayerId) -> Vec<Message> {
        let Some(run) = self
            .players
            .get_mut(&player)
            .and_then(|state| state.run.as_mut())
        else {
            return Vec::new();
        };
        let mut shown = Vec::new();
        for (left, ends) in [
            (&mut run.mask, "Your mask drops."),
            (&mut run.decoy, "Your decoy fades."),
        ] {
            if *left > 0 {
                *left -= 1;
                if *left == 0 {
                    shown.push(Message::game(ends));
                }
            }
        }
        if let Some((verb, left)) = run.frozen {
            if left > 1 {
                run.frozen = Some((verb, left - 1));
            } else {
                run.frozen = None;
                let name = commands::name_of(verb);
                shown.push(Message::game(format!("Your {name} works again.")));
            }
        }
        shown
    }

    pub(super) fn tick_ice(&mut self, id: InstanceId) -> Vec<Output> {
        let members = self.members(id);
        let Some(instance) = self.instances.get_mut(&id) else {
            return Vec::new();
        };
        let mut outputs = Vec::new();
        let mut fired = Vec::new();
        for (index, ice) in instance.ice.iter_mut().enumerate() {
            if ice.broken() {
                continue;
            }
            if ice
                .target
                .is_some_and(|target| host_of(&members, target).is_none())
            {
                ice.target = None;
            }
            if let Some(decoy) = decoy_on(&members, ice.host) {
                ice.target = Some(decoy);
            }
            let at = ice.target.and_then(|target| host_of(&members, target));
            if ice.kind == IceKind::Hound
                && ice.awake
                && let (Some(prey), Some(at)) = (ice.target, at)
                && at != ice.host
            {
                ice.host = at;
                outputs.push(Output {
                    player: prey,
                    message: ice.named().text(" follows you."),
                });
            }
            if ice.kind.local() && ice.intent.is_some() && at != Some(ice.host) {
                ice.intent = None;
                if let Some(target) = ice.target.take() {
                    let message = ice.named().text(" loses your signal.");
                    outputs.push(Output {
                        player: target,
                        message,
                    });
                }
                continue;
            }
            if let Some(intent) = &mut ice.intent {
                intent.ticks = intent.ticks.saturating_sub(1);
                if intent.ticks == 0 {
                    fired.push((index, intent.action));
                    ice.intent = None;
                }
                continue;
            }
            if ice.kind.local() && ice.awake && at != Some(ice.host) {
                let near = members.iter().find(|member| member.host == ice.host);
                ice.target = near.map(|member| member.player);
            }
            let actions = ice.kind.actions();
            let target = ice
                .target
                .filter(|&target| host_of(&members, target) == Some(ice.host));
            if ice.awake
                && !actions.is_empty()
                && let Some(target) = target
            {
                let action = actions[self.random.below(actions.len())];
                let told = ice.compile(action);
                outputs.push(Output {
                    player: target,
                    message: told,
                });
                let seen = ice.telegraph(Some(name_of(&members, target)));
                outputs.extend(tell(&witnesses(&members, ice.host, target), &seen));
            }
        }
        for (index, action) in fired {
            outputs.extend(self.fire(id, index, action));
        }
        outputs
    }

    fn fire(&mut self, id: InstanceId, index: usize, action: Action) -> Vec<Output> {
        let members = self.members(id);
        let Some(instance) = self.instances.get_mut(&id) else {
            return Vec::new();
        };
        let Some((source, host, Some(player))) = instance
            .ice
            .get(index)
            .map(|ice| (ice.named(), ice.host, ice.target))
        else {
            return Vec::new();
        };
        let Some(at) = host_of(&members, player) else {
            return Vec::new();
        };
        let name = name_of(&members, player).to_owned();
        let fired = |rest: String| {
            source
                .clone()
                .text(" fires ")
                .span(Style::Danger, action.name())
                .text(rest)
        };
        let (shown, seen) = match action.effect() {
            Effect::Alert => {
                let sends = |rest: String| {
                    source
                        .clone()
                        .text(" sends an ")
                        .span(Style::Danger, action.name())
                        .text(rest)
                };
                let mut woken = Vec::new();
                for ice in &mut instance.ice {
                    if ice.kind == IceKind::Hound && !ice.awake && !ice.broken() {
                        (ice.awake, ice.host, ice.target) = (true, at, Some(player));
                        woken.push(ice.named().text(" wakes up and hunts you."));
                    }
                }
                let shown = if woken.is_empty() {
                    self.trace_hit(player, sends(".".to_owned()), ALERT_TRACE)
                } else {
                    std::iter::once(sends(".".to_owned()))
                        .chain(woken)
                        .collect()
                };
                (shown, sends(format!(" about {name}.")))
            }
            Effect::Trace(amount) => (
                self.trace_hit(player, fired(".".to_owned()), amount),
                fired(format!(" at {name}.")),
            ),
            Effect::Damage(amount) => {
                let shield = self
                    .players
                    .get_mut(&player)
                    .and_then(|state| state.run.as_mut())
                    .filter(|run| run.firewall > 0);
                let stopped = shield.is_some();
                if let Some(run) = shield {
                    run.firewall -= 1;
                }
                let shown = if stopped {
                    vec![fired(". Your firewall stops it.".to_owned())]
                } else {
                    self.damage_hit(player, fired(".".to_owned()), amount)
                };
                (shown, fired(format!(" at {name}.")))
            }
            Effect::Drain => {
                let rig = self.players.get_mut(&player).map(|state| &mut state.rig);
                let before = rig.map_or(0, |rig| std::mem::take(&mut rig.cycles));
                let shown = fired(format!(". Cycles: {before} -> 0."));
                (vec![shown], fired(format!(" at {name}.")))
            }
            Effect::Freeze(ticks) => {
                let shown = match self.freeze(player, ticks) {
                    Some(program) => fired(format!(". Your {program} freezes for {ticks} ticks.")),
                    None => fired(". It finds no program to bite.".to_owned()),
                };
                (vec![shown], fired(format!(" at {name}.")))
            }
        };
        let mut outputs = address(player, shown);
        outputs.extend(tell(&witnesses(&members, host, player), &seen));
        outputs
    }

    fn freeze(&mut self, player: PlayerId, ticks: u32) -> Option<&'static str> {
        let state = self.players.get_mut(&player)?;
        let first = state.rig.loads.first().copied();
        let run = state.run.as_mut()?;
        let verb = run.last.or(first)?;
        run.frozen = Some((verb, ticks));
        Some(commands::name_of(verb))
    }

    fn damage_hit(&mut self, player: PlayerId, line: Message, amount: u32) -> Vec<Message> {
        let before = self
            .players
            .get(&player)
            .map_or(0, |state| state.rig.integrity);
        let after = before.saturating_sub(amount);
        let mut shown = vec![line.text(format!(" Integrity: {before} -> {after}."))];
        shown.extend(self.damage(player, amount));
        shown
    }

    fn trace_hit(&mut self, player: PlayerId, line: Message, amount: u32) -> Vec<Message> {
        let Some(run) = self
            .players
            .get(&player)
            .and_then(|state| state.run.as_ref())
        else {
            return vec![line];
        };
        let amount = if run.proxy { amount / 2 } else { amount };
        let before = run.trace;
        let after = (before + amount).min(MAX_TRACE);
        let mut shown = vec![line.text(format!(" Trace: {before}% -> {after}%."))];
        shown.extend(self.raise_trace(player, amount));
        shown
    }

    pub(super) fn guard(&self, player: PlayerId, target: HostId) -> Result<(), Message> {
        let here = self.here(player);
        let Some(instance) = self.instance(player) else {
            return Ok(());
        };
        if instance.cuts.contains(&(here, target)) {
            return Err(Message::error("The SOC cut the route to ")
                .span(Style::Host, &self.hosts[target.0].id)
                .text("."));
        }
        let warden = instance.ice.iter().find(|ice| {
            ice.host == here && ice.guards == Some(target) && !ice.broken() && !ice.fooled
        });
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
        let instance = self.instance(viewer)?;
        let here: Vec<&Ice> = instance.ice.iter().filter(|ice| ice.host == host).collect();
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
    use super::deploy;
    use crate::content::IceDef;
    use crate::ice::IceKind;
    use crate::testing::{joined, plain, step, walk};
    use crate::world::{Host, HostId, NetworkId};

    const ARCHIVE: [&str; 4] = ["go back-room", "go gate", "go core", "go archive"];

    fn at_gate(lines: &[&str]) -> Vec<String> {
        walk(&[&["go back-room", "go gate"][..], lines].concat()).0
    }

    fn at_archive(lines: &[&str]) -> Vec<String> {
        walk(&[&ARCHIVE[..], lines].concat()).0
    }

    #[test]
    fn noise_wakes_a_scorch() {
        let shown = at_archive(&["scan"]);
        assert_eq!(shown.last().map(String::as_str), Some("scorch wakes up."));
        assert_eq!(
            at_archive(&["get tapes.db"]),
            ["Downloaded tapes.db.", "Unknown ICE wakes up."]
        );
    }

    #[test]
    fn an_awake_scorch_attacks_the_rig() {
        let shown = at_archive(&["scan", "~"]);
        let [line] = shown.as_slice() else {
            panic!("{shown:?}");
        };
        let burn = line == "scorch starts to compile BURN (2 ticks).";
        let spike = line == "scorch starts to compile SPIKE (1 tick).";
        assert!(burn || spike, "{line}");
        let status = at_archive(&["scan", "~3", "status | grep integrity"]);
        assert_ne!(status, ["Integrity: 100/100"]);
    }

    #[test]
    fn leaving_the_host_dodges_a_scorch() {
        assert_eq!(
            at_archive(&["scan", "~", "go core", "~"]),
            ["scorch loses your signal."]
        );
    }

    const PIT: &str = "[city lab]\nname = Lab\nstart = hub\n[host hub]\nname = Hub\ndesc = A hub.\n\
                       [network net]\nname = Net\n[host gate]\nname = Gate\ndesc = A gate.\n\
                       network = net\nroute = hub\nroute = core\n[host core]\nname = Core\n\
                       desc = A core.\nnetwork = net\n[ice gate/tarpit]\ntype = tarpit\n\
                       [ice gate/lockjaw]\ntype = lockjaw\n";

    fn in_the_pit(lines: &[&str]) -> Vec<String> {
        let content = crate::Content::parse(PIT).unwrap();
        let mut world = crate::World::new(content, 0);
        let (player, _) = world.join("ghost");
        let mut last = Vec::new();
        for line in [&["go gate"][..], lines].concat() {
            last = plain(&step(&mut world, player, line));
        }
        last
    }

    #[test]
    fn noise_wakes_a_tarpit_and_a_lockjaw_and_they_telegraph() {
        assert_eq!(
            in_the_pit(&["scan"])[2..],
            ["tarpit wakes up.", "lockjaw wakes up."]
        );
        assert_eq!(
            in_the_pit(&["scan", "~"]),
            [
                "tarpit starts to compile TAR (2 ticks).",
                "lockjaw starts to compile BITE (2 ticks)."
            ]
        );
    }

    #[test]
    fn tar_takes_the_cycles_and_bite_freezes_the_last_program() {
        let fired = ["scan", "~2", "~"];
        assert_eq!(
            in_the_pit(&fired),
            [
                "tarpit fires TAR. Cycles: 4 -> 0.",
                "lockjaw fires BITE. Your scan freezes for 6 ticks."
            ]
        );
        assert_eq!(
            in_the_pit(&[&fired[..], &["scan"]].concat()),
            ["Your scan is frozen for 6 more ticks."]
        );
        assert_eq!(
            in_the_pit(&[&fired[..], &["go core", "~5", "scan"]].concat()),
            ["Your scan is frozen for 1 more tick."]
        );
        assert_eq!(
            in_the_pit(&[&fired[..], &["go core", "~6"]].concat()),
            ["Your scan works again."]
        );
    }

    #[test]
    fn leaving_the_host_stops_a_tarpit_and_a_lockjaw() {
        assert_eq!(
            in_the_pit(&["scan", "~", "go core", "~"]),
            ["tarpit loses your signal.", "lockjaw loses your signal."]
        );
    }

    #[test]
    fn a_rig_without_integrity_crashes() {
        let (mut world, player) = joined();
        for line in ARCHIVE {
            world.command(player, line);
        }
        world.command(player, "get tapes.db");
        if let Some(state) = world.players.get_mut(&player) {
            state.rig.integrity = 5;
        }
        let shown = plain(&step(&mut world, player, "~3"));
        let crash = "Your rig crashes, and you lose the loot of this run.";
        assert!(shown.contains(&crash.to_owned()), "{shown:?}");
        assert_eq!(world.prompt(player).as_deref(), Some("ghost@back-room $ "));
        assert_eq!(
            plain(&world.command(player, "inventory")),
            ["Your rig has no files."]
        );
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
    fn a_warden_guards_a_host_of_its_own_network() {
        let host = |id: &str, network: usize, ice: Vec<IceDef>| Host {
            id: id.to_owned(),
            name: String::new(),
            description: String::new(),
            network: Some(NetworkId(network)),
            routes: Vec::new(),
            files: Vec::new(),
            ice,
            security: None,
            software: None,
        };
        let warden = IceDef {
            name: "warden".to_owned(),
            kind: IceKind::Warden,
            version: 1,
            guards: Some("db".to_owned()),
        };
        let hosts = [
            host("db", 0, vec![]),
            host("lan", 1, vec![warden]),
            host("db", 1, vec![]),
        ];
        assert_eq!(deploy(&hosts, NetworkId(1), 0)[0].guards, Some(HostId(2)));
    }

    #[test]
    fn a_warden_blocks_only_its_own_route() {
        assert_eq!(at_gate(&["go core"])[0], "Lab Core");
    }

    #[test]
    fn noise_near_a_sentry_starts_an_alert() {
        let shown = at_gate(&["~5", "scan"]);
        assert_eq!(
            shown.last().map(String::as_str),
            Some("sentry starts to compile ALERT (3 ticks).")
        );
    }

    #[test]
    fn an_alert_wakes_the_hounds() {
        assert_eq!(at_gate(&["scan", "~4"]), Vec::<String>::new());
        assert_eq!(
            at_gate(&["scan", "~5"]),
            [
                "sentry sends an ALERT.",
                "Unknown ICE wakes up and hunts you."
            ]
        );
        assert_eq!(
            at_gate(&["scan", "~5", "look"]).last().map(String::as_str),
            Some("ICE: sentry, warden, 1 unknown. Type scan to see them.")
        );
    }

    #[test]
    fn a_hunting_hound_follows_you() {
        assert_eq!(
            at_gate(&["scan", "~5", "go core", "~"])[0],
            "Unknown ICE follows you."
        );
    }

    #[test]
    fn a_second_alert_raises_the_trace() {
        let shown = at_gate(&["scan", "~5", "scan", "~3"]);
        assert!(
            shown.contains(&"sentry sends an ALERT. Trace: 8% -> 18%.".to_owned()),
            "{shown:?}"
        );
    }

    #[test]
    fn a_hunting_hound_telegraphs_its_attack() {
        let shown = at_gate(&["scan", "~5", "~"]);
        let [line] = shown.as_slice() else {
            panic!("{shown:?}");
        };
        let burst = line == "Unknown ICE starts to compile TRACE BURST (2 ticks).";
        let deep = line == "Unknown ICE starts to compile DEEP TRACE (4 ticks).";
        assert!(burst || deep, "{line}");
    }

    #[test]
    fn a_hunting_hound_raises_the_trace() {
        let (_, prompt) = walk(&["go back-room", "go gate", "scan", "~3", "~10"]);
        let trace: u32 = prompt
            .as_deref()
            .and_then(|prompt| prompt.strip_prefix("[trace "))
            .and_then(|rest| rest.split('%').next())
            .and_then(|number| number.parse().ok())
            .unwrap_or(0);
        assert!(trace > 4, "{prompt:?}");
    }
}
