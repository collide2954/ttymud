use ttymud_text::{Message, Style};

use super::ram::named_error;
use super::rig::Rig;
use super::{Player, PlayerId, Step, World, lines, listed};
use crate::commands::{self, Program, TOP_VERSION, Verb};
use crate::content::File;

const QUIET_PRICE: u32 = 400;
const USAGE: &str = "Usage: compile [program] [quiet|plain].";

type Change = fn(&mut Player, Verb, &str) -> Result<Message, Message>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Build {
    pub(super) version: u32,
    pub(super) quiet: bool,
}

impl Default for Build {
    fn default() -> Self {
        Self {
            version: 1,
            quiet: false,
        }
    }
}

fn power_at(verb: Verb, version: u32) -> u32 {
    verb.power()
        .map_or(0, |power| power.base + power.step * (version - 1))
}

fn price(version: u32) -> u32 {
    if version == 2 { 1_000 } else { 3_000 }
}

pub(super) fn effect(verb: Verb, version: u32) -> String {
    let power = power_at(verb, version);
    match verb {
        Verb::Crack => format!("It does {power} damage."),
        Verb::Scrub => format!("It lowers a trace by {power}%."),
        Verb::Patch => format!("It repairs {power} integrity."),
        Verb::Firewall => format!("It stops {power} attacks."),
        Verb::Mask => format!("It hides your noise for {power} ticks."),
        Verb::Decoy => format!("It draws the ICE to you for {power} ticks."),
        _ => String::new(),
    }
}

impl Rig {
    pub(super) fn build(&self, verb: Verb) -> Build {
        let found = self.builds.iter().find(|&&(known, _)| known == verb);
        found.map_or_default(|&(_, build)| build)
    }

    fn set_build(&mut self, verb: Verb, build: Build) {
        self.builds.retain(|&(known, _)| known != verb);
        if build != Build::default() {
            self.builds.push((verb, build));
        }
    }

    pub(super) fn power(&self, verb: Verb) -> u32 {
        power_at(verb, self.build(verb).version)
    }

    pub(super) fn program(&self, verb: Verb) -> Option<Program> {
        let mut program = verb.program()?;
        if self.build(verb).quiet {
            program.noise = program.noise.saturating_sub(1);
            program.cycles += 1;
        }
        Some(program)
    }

    pub(super) fn label(&self, verb: Verb) -> String {
        let build = self.build(verb);
        let mut label = commands::name_of(verb).to_owned();
        if build.version > 1 {
            label = format!("{label} v{}", build.version);
        }
        if build.quiet {
            label.push_str(" (quiet)");
        }
        label
    }
}

impl World {
    pub(super) fn power(&self, player: PlayerId, verb: Verb) -> u32 {
        let state = self.players.get(&player);
        state.map_or(0, |state| state.rig.power(verb))
    }

    pub(super) fn study(&mut self, player: PlayerId, files: &[File]) -> Vec<Message> {
        let home = self.home.map(|home| self.hosts[home.0].id.clone());
        let Some(state) = self.players.get_mut(&player) else {
            return Vec::new();
        };
        let mut shown = Vec::new();
        for blueprint in files.iter().filter_map(File::teaches) {
            if state.blueprints.contains(&blueprint) {
                continue;
            }
            state.blueprints.push(blueprint);
            let (verb, version) = blueprint;
            let learned = Message::game("You learn the blueprint for ")
                .span(Style::Command, commands::name_of(verb))
                .text(format!(" v{version}."));
            shown.push(match &home {
                Some(home) => learned
                    .text(" Compile it at ")
                    .span(Style::Host, home)
                    .text("."),
                None => learned,
            });
        }
        shown
    }

    pub(super) fn compile(&mut self, player: PlayerId, args: &[String]) -> Result<Step, Message> {
        self.at_home(player, "compile programs")?;
        let (word, change): (&String, Change) = match args {
            [] => return Ok(self.compile_options(player)),
            [word] => (word, Player::next_version),
            [word, mode] if mode.eq_ignore_ascii_case("quiet") => (word, Player::add_quiet),
            [word, mode] if mode.eq_ignore_ascii_case("plain") => (word, Player::remove_quiet),
            _ => return Err(Message::error(USAGE)),
        };
        let (verb, name) = self.program_named(player, "compile", std::slice::from_ref(word))?;
        let Some(state) = self.players.get_mut(&player) else {
            return Ok(Step::default());
        };
        Ok(Step::show(vec![change(state, verb, name)?]))
    }

    fn compile_options(&self, player: PlayerId) -> Step {
        let Some(state) = self.players.get(&player) else {
            return Step::default();
        };
        let owned: Vec<(Verb, u32)> = commands::versioned()
            .flat_map(|verb| (2..=TOP_VERSION).map(move |version| (verb, version)))
            .filter(|blueprint| state.blueprints.contains(blueprint))
            .collect();
        let label =
            |&(verb, version): &(Verb, u32)| format!("{} v{version}", commands::name_of(verb));
        let mut shown = Vec::new();
        if owned.is_empty() {
            shown.push(Message::game(
                "You have no blueprints. Find them in targets, or buy them at a shop.",
            ));
        } else {
            let names: Vec<String> = owned.iter().map(label).collect();
            let message = Message::game("Blueprints: ");
            shown.push(listed(
                message,
                Style::Command,
                names.iter().map(String::as_str),
            ));
            let next: Vec<String> = owned
                .iter()
                .filter(|&&(verb, version)| {
                    state.know.contains(&verb) && state.rig.build(verb).version + 1 == version
                })
                .map(|blueprint| format!("{} for {} cr", label(blueprint), price(blueprint.1)))
                .collect();
            shown.push(if next.is_empty() {
                Message::game("You cannot compile a new version now.")
            } else {
                Message::game(format!("You can compile: {}.", next.join(", ")))
            });
        }
        shown.push(
            Message::game("Type ")
                .span(Style::Command, "compile <program> quiet")
                .text(format!(" to add the quiet mod for {QUIET_PRICE} cr, or "))
                .span(Style::Command, "compile <program> plain")
                .text(" to remove it."),
        );
        lines(shown)
    }
}

impl Player {
    fn next_version(&mut self, verb: Verb, name: &str) -> Result<Message, Message> {
        if verb.power().is_none() {
            return Err(named_error(name, " has no versions."));
        }
        let mut build = self.rig.build(verb);
        if build.version == TOP_VERSION {
            return Err(named_error(
                name,
                &format!(" is already at v{TOP_VERSION}."),
            ));
        }
        build.version += 1;
        let (version, price, credits) = (build.version, price(build.version), self.credits);
        if !self.blueprints.contains(&(verb, version)) {
            return Err(Message::error("You need the blueprint for ")
                .span(Style::Command, name)
                .text(format!(" v{version}.")));
        }
        if credits < price {
            let text = format!(" v{version} costs {price} cr to compile. You have {credits} cr.");
            return Err(named_error(name, &text));
        }
        self.credits -= price;
        self.rig.set_build(verb, build);
        let effect = effect(verb, version);
        Ok(Message::game("You compile ")
            .span(Style::Command, name)
            .text(format!(" v{version} for {price} cr. {effect}")))
    }

    fn add_quiet(&mut self, verb: Verb, name: &str) -> Result<Message, Message> {
        let mut build = self.rig.build(verb);
        if build.quiet {
            return Err(named_error(name, " is already quiet."));
        }
        if verb.program().is_none_or(|program| program.noise == 0) {
            return Err(named_error(name, " makes no noise."));
        }
        let credits = self.credits;
        if credits < QUIET_PRICE {
            return Err(Message::error(format!(
                "The quiet mod costs {QUIET_PRICE} cr. You have {credits} cr."
            )));
        }
        self.credits -= QUIET_PRICE;
        build.quiet = true;
        self.rig.set_build(verb, build);
        Ok(Message::game("You add the quiet mod to ")
            .span(Style::Command, name)
            .text(format!(
                " for {QUIET_PRICE} cr. It makes 1 less noise and needs 1 more cycle."
            )))
    }

    fn remove_quiet(&mut self, verb: Verb, name: &str) -> Result<Message, Message> {
        let mut build = self.rig.build(verb);
        if !build.quiet {
            return Err(named_error(name, " has no mod."));
        }
        build.quiet = false;
        self.rig.set_build(verb, build);
        Ok(Message::game("You take the quiet mod off ")
            .span(Style::Command, name)
            .text("."))
    }
}

#[cfg(test)]
mod tests {
    use super::Build;
    use crate::commands::Verb;
    use crate::content::File;
    use crate::testing::{joined, plain};
    use crate::{PlayerId, World};

    fn built(builds: &[(Verb, Build)]) -> (World, PlayerId) {
        let (mut world, player) = joined();
        if let Some(state) = world.players.get_mut(&player) {
            state.rig.builds = builds.to_vec();
            state
                .rig
                .loads
                .extend([Verb::Firewall, Verb::Mask, Verb::Decoy]);
        }
        for line in ["go back-room", "go gate"] {
            world.command(player, line);
        }
        if let Some(state) = world.players.get_mut(&player) {
            state.rig.cycles = 20;
        }
        (world, player)
    }

    fn version(version: u32) -> Build {
        Build {
            version,
            quiet: false,
        }
    }

    fn said(world: &mut World, player: PlayerId, line: &str) -> Vec<String> {
        plain(&world.command(player, line))
    }

    fn vitals(world: &World, player: PlayerId) -> (u32, u32) {
        let state = world.players.get(&player);
        let trace = state
            .and_then(|state| state.run.as_ref())
            .map_or(0, |run| run.trace);
        (state.map_or(0, |state| state.rig.cycles), trace)
    }

    #[test]
    fn a_higher_version_makes_the_main_effect_of_a_program_stronger() {
        let (mut world, player) = built(&[
            (Verb::Crack, version(3)),
            (Verb::Scrub, version(2)),
            (Verb::Patch, version(3)),
            (Verb::Firewall, version(2)),
            (Verb::Mask, version(3)),
            (Verb::Decoy, version(2)),
        ]);
        said(&mut world, player, "scan");
        assert_eq!(
            said(&mut world, player, "crack warden"),
            ["crack hits warden. Integrity: 40 -> 15."]
        );
        if let Some(state) = world.players.get_mut(&player) {
            state.rig.integrity = 50;
            if let Some(run) = state.run.as_mut() {
                run.trace = 30;
            }
        }
        assert_eq!(
            said(&mut world, player, "scrub"),
            ["scrub deletes your log lines. Trace: 30% -> 10%."]
        );
        assert_eq!(
            said(&mut world, player, "patch"),
            ["patch repairs your rig. Integrity: 50 -> 80."]
        );
        assert_eq!(
            said(&mut world, player, "firewall"),
            ["firewall is up. It stops the next 2 attacks on your integrity."]
        );
        assert_eq!(
            said(&mut world, player, "mask")[0],
            "mask hides your noise for 7 ticks."
        );
        assert_eq!(
            said(&mut world, player, "decoy")[0],
            "decoy draws the ICE on this host to you for 12 ticks."
        );
    }

    #[test]
    fn the_quiet_mod_trades_one_noise_for_one_cycle() {
        let mut costs = Vec::new();
        for quiet in [false, true] {
            let crack = Build { version: 1, quiet };
            let (mut world, player) = built(&[(Verb::Crack, crack)]);
            said(&mut world, player, "scan");
            let before = vitals(&world, player);
            said(&mut world, player, "crack warden");
            let after = vitals(&world, player);
            costs.push((before.0 - after.0, after.1 - before.1));
        }
        assert_eq!(costs, [(3, 6), (4, 4)]);
    }

    #[test]
    fn status_shows_the_versions_and_the_mods() {
        let crack = Build {
            version: 2,
            quiet: true,
        };
        let (mut world, player) = built(&[(Verb::Crack, crack)]);
        assert_eq!(
            said(&mut world, player, "status | grep programs"),
            ["Programs: scan, probe, crack v2 (quiet), scrub, patch, firewall, decoy, mask"]
        );
    }

    fn at_home(credits: u32, blueprints: &[(Verb, u32)]) -> (World, PlayerId) {
        let (mut world, player) = joined();
        if let Some(state) = world.players.get_mut(&player) {
            state.credits = credits;
            state.blueprints = blueprints.to_vec();
        }
        world.command(player, "go back-room");
        (world, player)
    }

    #[test]
    fn compile_needs_the_home_host_and_a_blueprint() {
        let (mut world, player) = joined();
        assert_eq!(
            said(&mut world, player, "compile crack"),
            ["You can compile programs only at back-room."]
        );
        let (mut world, player) = at_home(5_000, &[]);
        assert_eq!(
            said(&mut world, player, "compile"),
            [
                "You have no blueprints. Find them in targets, or buy them at a shop.",
                "Type compile <program> quiet to add the quiet mod for 400 cr, or compile <program> plain to remove it."
            ]
        );
        assert_eq!(
            said(&mut world, player, "compile crack"),
            ["You need the blueprint for crack v2."]
        );
        assert_eq!(
            said(&mut world, player, "compile scan"),
            ["scan has no versions."]
        );
    }

    #[test]
    fn compile_makes_the_next_version_for_credits() {
        let (mut world, player) = at_home(4_500, &[(Verb::Crack, 2), (Verb::Crack, 3)]);
        assert_eq!(
            said(&mut world, player, "compile")[..2],
            [
                "Blueprints: crack v2, crack v3",
                "You can compile: crack v2 for 1000 cr."
            ]
        );
        assert_eq!(
            said(&mut world, player, "compile crack"),
            ["You compile crack v2 for 1000 cr. It does 20 damage."]
        );
        assert_eq!(
            said(&mut world, player, "compile crack"),
            ["You compile crack v3 for 3000 cr. It does 25 damage."]
        );
        assert_eq!(
            said(&mut world, player, "compile crack"),
            ["crack is already at v3."]
        );
        assert_eq!(
            said(&mut world, player, "compile")[1],
            "You cannot compile a new version now."
        );
        assert_eq!(
            said(&mut world, player, "status | grep credits"),
            ["Credits: 500 cr"]
        );
        let (mut world, player) = at_home(500, &[(Verb::Mask, 2)]);
        assert_eq!(
            said(&mut world, player, "compile mask"),
            ["mask v2 costs 1000 cr to compile. You have 500 cr."]
        );
    }

    #[test]
    fn compile_adds_or_removes_the_quiet_mod() {
        let (mut world, player) = at_home(500, &[]);
        assert_eq!(
            said(&mut world, player, "compile crack quiet"),
            [
                "You add the quiet mod to crack for 400 cr. It makes 1 less noise and needs 1 more cycle."
            ]
        );
        assert_eq!(
            said(&mut world, player, "compile crack quiet"),
            ["crack is already quiet."]
        );
        assert_eq!(
            said(&mut world, player, "compile probe quiet"),
            ["The quiet mod costs 400 cr. You have 100 cr."]
        );
        assert_eq!(
            said(&mut world, player, "compile scrub quiet"),
            ["scrub makes no noise."]
        );
        assert_eq!(
            said(&mut world, player, "compile crack plain"),
            ["You take the quiet mod off crack."]
        );
        assert_eq!(
            said(&mut world, player, "compile crack plain"),
            ["crack has no mod."]
        );
        assert_eq!(
            said(&mut world, player, "compile crack loud"),
            ["Usage: compile [program] [quiet|plain]."]
        );
    }

    #[test]
    fn reading_a_blueprint_file_teaches_the_blueprint() {
        let (mut world, player) = at_home(0, &[]);
        if let Some(state) = world.players.get_mut(&player) {
            state.rig.files.push(File::blueprint(Verb::Crack, 2));
        }
        let notes = "The source code and build notes for crack v2.";
        assert_eq!(
            said(&mut world, player, "read crack-v2.blueprint"),
            [
                notes,
                "You learn the blueprint for crack v2. Compile it at back-room."
            ]
        );
        assert_eq!(said(&mut world, player, "read crack-v2.blueprint"), [notes]);
        assert_eq!(
            said(&mut world, player, "compile")[0],
            "Blueprints: crack v2"
        );
    }

    #[test]
    fn blueprints_stay_with_the_account_after_a_burn() {
        let mut world = crate::testing::world();
        let (player, _) = world.connect("a");
        for line in ["nyx", "breaker"] {
            world.command(player, line);
        }
        if let Some(state) = world.players.get_mut(&player) {
            state.blueprints = vec![(Verb::Crack, 2)];
        }
        world.burn(player);
        for line in ["zer0", "face", "go back-room"] {
            world.command(player, line);
        }
        assert_eq!(
            said(&mut world, player, "compile")[0],
            "Blueprints: crack v2"
        );
        world.leave(player);
        let saves = world.saves();
        let text = saves
            .iter()
            .find(|(path, _)| path == "accounts/a.ini")
            .map_or_default(|(_, text)| text.clone());
        assert!(text.contains("\nblueprint = crack 2\n"), "{text}");
        let mut fresh = crate::testing::world();
        for (path, text) in &saves {
            fresh.restore(path, text).unwrap();
        }
        let (player, _) = fresh.connect("a");
        assert_eq!(
            said(&mut fresh, player, "compile")[0],
            "Blueprints: crack v2"
        );
    }

    #[test]
    fn versions_and_mods_come_back_from_the_save() {
        let mut world = crate::testing::world();
        let (player, _) = world.connect("a");
        for line in ["nyx", "breaker"] {
            world.command(player, line);
        }
        if let Some(state) = world.players.get_mut(&player) {
            state.rig.builds = vec![
                (Verb::Crack, version(3)),
                (
                    Verb::Probe,
                    Build {
                        version: 1,
                        quiet: true,
                    },
                ),
            ];
        }
        world.leave(player);
        let saves = world.saves();
        let text = saves
            .iter()
            .find(|(path, _)| path == "accounts/a.ini")
            .map_or_default(|(_, text)| text.clone());
        assert!(
            text.contains("\nversion = crack 3\nquiet = probe\n"),
            "{text}"
        );
        let mut fresh = crate::testing::world();
        for (path, text) in &saves {
            fresh.restore(path, text).unwrap();
        }
        let (player, _) = fresh.connect("a");
        assert_eq!(
            said(&mut fresh, player, "status | grep programs"),
            ["Programs: scan, probe (quiet), crack v3, patch, firewall"]
        );
    }
}
