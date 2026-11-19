use ttymud_text::{Kind, Message, Style};

use super::{PlayerId, Step, Thing, World, choose, usage};
use crate::commands::{self, Verb};

impl World {
    pub(super) fn at_home(&self, player: PlayerId, action: &str) -> Result<(), Message> {
        let Some(home) = self.home else {
            return Err(Message::error("This city has no home host."));
        };
        if self.here(player) == home {
            return Ok(());
        }
        Err(Message::error(format!("You can {action} only at "))
            .span(Style::Host, &self.hosts[home.0].id)
            .text("."))
    }

    pub(super) fn load(&mut self, player: PlayerId, args: &[String]) -> Result<Step, Message> {
        self.at_home(player, "change your RAM")?;
        let (verb, name) = self.program_named(player, "load", args)?;
        let Some(rig) = self.players.get_mut(&player).map(|state| &mut state.rig) else {
            return Ok(Step::default());
        };
        if rig.loads.contains(&verb) {
            return Err(named_error(name, " is already in your RAM."));
        }
        let need = verb.program().map_or(0, |program| program.ram);
        let free = rig.ram.saturating_sub(rig.used());
        if need > free {
            return Err(named_error(
                name,
                &format!(" needs {need} GB, but only {free} GB are free. Unload a program first."),
            ));
        }
        rig.loads.push(verb);
        Ok(Step::show(vec![
            Message::game("You load ")
                .span(Style::Command, name)
                .text(format!(". {}.", rig.ram_line())),
        ]))
    }

    pub(super) fn unload(&mut self, player: PlayerId, args: &[String]) -> Result<Step, Message> {
        self.at_home(player, "change your RAM")?;
        let (verb, name) = self.program_named(player, "unload", args)?;
        let Some(rig) = self.players.get_mut(&player).map(|state| &mut state.rig) else {
            return Ok(Step::default());
        };
        if !rig.loads.contains(&verb) {
            return Err(named_error(name, " is not in your RAM."));
        }
        rig.loads.retain(|&loaded| loaded != verb);
        Ok(Step::show(vec![
            Message::game("You unload ")
                .span(Style::Command, name)
                .text(format!(". {}.", rig.ram_line())),
        ]))
    }

    pub(super) fn known_programs(&self, player: PlayerId) -> Vec<(Verb, &'static str)> {
        let Some(state) = self.players.get(&player) else {
            return Vec::new();
        };
        commands::programs()
            .filter(|(command, _)| state.know.contains(&command.verb))
            .map(|(command, _)| (command.verb, command.name()))
            .collect()
    }

    fn program_named(
        &self,
        player: PlayerId,
        command: &str,
        args: &[String],
    ) -> Result<(Verb, &'static str), Message> {
        let known = self.known_programs(player);
        let names: Vec<&str> = known.iter().map(|&(_, name)| name).collect();
        let [word] = args else {
            return Err(usage(
                &format!("{command} <program>"),
                Thing::Program,
                &names,
            ));
        };
        let name = choose(Thing::Program, word, &names)?;
        let index = names.iter().position(|&known| known == name).unwrap_or(0);
        Ok(known[index])
    }

    pub(super) fn not_loaded(&self, name: &str) -> Message {
        let message = named_error(name, " is not in your RAM.");
        match self.home {
            Some(home) => message
                .text(" Load it at ")
                .span(Style::Host, &self.hosts[home.0].id)
                .text("."),
            None => message,
        }
    }
}

fn named_error(name: &str, rest: &str) -> Message {
    Message::new(Kind::Error)
        .span(Style::Command, name)
        .text(rest)
}

#[cfg(test)]
mod tests {
    use crate::testing::{joined, plain, run, walk};

    const HOME: &str = "go back-room";

    #[test]
    fn unload_frees_ram_at_home() {
        assert_eq!(
            walk(&[HOME, "unload patch"]).0,
            ["You unload patch. RAM: 6/8 GB."]
        );
        assert_eq!(
            walk(&[HOME, "unload patch", "status | grep programs"]).0,
            ["Programs: scan, probe, crack, scrub"]
        );
    }

    #[test]
    fn load_puts_a_program_into_ram() {
        assert_eq!(
            walk(&[HOME, "unload patch", "load patch"]).0,
            ["You load patch. RAM: 8/8 GB."]
        );
    }

    #[test]
    fn a_program_must_be_in_ram_to_run() {
        assert_eq!(
            walk(&[HOME, "unload scan", "go gate", "scan"]).0,
            ["scan is not in your RAM. Load it at back-room."]
        );
    }

    #[test]
    fn the_ram_changes_only_at_home() {
        assert_eq!(
            run("unload scan"),
            ["You can change your RAM only at back-room."]
        );
        assert_eq!(
            run("load scan"),
            ["You can change your RAM only at back-room."]
        );
    }

    #[test]
    fn load_completes_what_is_not_in_ram_and_unload_what_is() {
        let (mut world, player) = joined();
        for line in [HOME, "unload patch"] {
            world.command(player, line);
        }
        assert_eq!(
            world.complete(player, "load ").names,
            ["patch", "firewall", "mask", "proxy", "pretext", "exploit"]
        );
        assert_eq!(world.complete(player, "unload s").names, ["scan", "scrub"]);
    }

    #[test]
    fn load_and_unload_check_the_program() {
        assert_eq!(
            walk(&[HOME, "load scan"]).0,
            ["scan is already in your RAM."]
        );
        assert_eq!(
            walk(&[HOME, "unload patch", "unload patch"]).0,
            ["patch is not in your RAM."]
        );
        let known = "scan, probe, crack, scrub, patch, firewall, mask, proxy, pretext, exploit";
        assert_eq!(
            walk(&[HOME, "load nuke"]).0,
            [format!("No program named nuke. Programs: {known}")]
        );
        assert_eq!(
            walk(&[HOME, "unload"]).0,
            [format!("Usage: unload <program>. Programs: {known}")]
        );
    }

    #[test]
    fn more_ram_holds_more_programs() {
        let (mut world, player) = joined();
        world.command(player, HOME);
        if let Some(state) = world.players.get_mut(&player) {
            state.rig.ram = 12;
        }
        assert_eq!(
            plain(&world.command(player, "load mask")),
            ["You load mask. RAM: 10/12 GB."]
        );
    }

    #[test]
    fn a_program_needs_room_in_ram() {
        assert_eq!(
            walk(&[HOME, "load mask"]).0,
            ["mask needs 2 GB, but only 0 GB are free. Unload a program first."]
        );
    }
}
