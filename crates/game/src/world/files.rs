use ttymud_text::{Kind, Message, Style};

use super::{HostId, PlayerId, Step, Thing, World, choose, listing, usage};
use crate::commands::Item;
use crate::content::File;
use crate::contract::Event;

const DOWNLOAD_NOISE: u32 = 2;

impl World {
    pub(super) fn read(
        &mut self,
        player: PlayerId,
        args: &[String],
        input: Option<Vec<Item>>,
    ) -> Result<Step, Message> {
        let readable = self.readable(player);
        let files = match input {
            Some(_) if !args.is_empty() => {
                return Err(Message::error("read takes a file or a pipe, not both."));
            }
            Some(items) => piped_files("read", items)?,
            None if args.is_empty() => {
                return Err(usage("read <file>", Thing::File, &names(&readable)));
            }
            None => pick(args, &readable)?,
        };
        let carried = |name: &str| {
            self.players
                .get(&player)
                .is_some_and(|state| state.rig.files.iter().any(|file| file.name == name))
        };
        let refusal = files
            .iter()
            .filter(|file| !carried(&file.name))
            .find_map(|file| self.locked(player, file));
        if let Some(refusal) = refusal {
            return Err(refusal);
        }
        let named = files.len() > 1;
        let mut shown = Vec::new();
        let mut items = Vec::new();
        for file in &files {
            if named {
                shown.push(Message::new(Kind::Game).span(Style::File, &file.name));
            }
            for line in &file.lines {
                shown.push(Message::game(line));
                items.push(Item::Line(line.clone()));
            }
        }
        shown.extend(self.learn(player, &files));
        shown.extend(self.study(player, &files));
        Ok(Step { shown, items })
    }

    pub(super) fn download(
        &mut self,
        player: PlayerId,
        args: &[String],
        input: Option<Vec<Item>>,
    ) -> Result<Step, Message> {
        let host = self.here(player);
        let here = self.files_on(host, player);
        let files = match input {
            Some(_) if !args.is_empty() => {
                return Err(Message::error("download takes a file or a pipe, not both."));
            }
            Some(items) => piped_files("download", items)?,
            None if args.is_empty() => {
                return Err(usage("download <file>", Thing::File, &names(&here)));
            }
            None => pick(args, &here)?,
        };
        let mut shown = Vec::new();
        let mut allowed = Vec::new();
        for file in files {
            let refusal = self.locked(player, &file);
            match refusal.or_else(|| self.own_flag(player, &file)) {
                Some(refusal) => shown.push(refusal),
                None => allowed.push(file),
            }
        }
        let Some(state) = self.players.get_mut(&player) else {
            return Ok(Step::default());
        };
        let mut copied = Vec::new();
        for file in allowed {
            if state.rig.files.iter().any(|known| known.name == file.name) {
                let name = &file.name;
                shown.push(Message::game(format!("{name} is already on your rig.")));
                continue;
            }
            shown.push(
                Message::game("Downloaded ")
                    .span(Style::File, &file.name)
                    .text("."),
            );
            if let Some(run) = &mut state.run {
                run.loot.push(file.name.clone());
            }
            copied.push(file.name.clone());
            state.rig.files.push(file);
        }
        for name in &copied {
            self.take_file(player, host, name);
        }
        if copied.iter().any(|name| self.is_goal(player, name)) {
            self.advance(player, Event::Download);
        }
        if !copied.is_empty() {
            shown.extend(self.noise(player, DOWNLOAD_NOISE));
        }
        Ok(Step::show(shown))
    }

    pub(super) fn inventory(&self, player: PlayerId) -> Step {
        let items: Vec<Item> = self
            .players
            .get(&player)
            .into_iter()
            .flat_map(|state| state.rig.files.iter().cloned().map(Item::File))
            .collect();
        let shown = if items.is_empty() {
            vec![Message::game("Your rig has no files.")]
        } else {
            listing(&items)
        };
        Step { shown, items }
    }

    pub(super) fn readable(&self, player: PlayerId) -> Vec<&File> {
        let mut found = self.files_on(self.here(player), player);
        let rig: &[File] = match self.players.get(&player) {
            Some(state) => &state.rig.files,
            None => &[],
        };
        for file in rig {
            if !found.iter().any(|known| known.name == file.name) {
                found.push(file);
            }
        }
        found
    }

    pub(super) fn files_on(&self, host: HostId, viewer: PlayerId) -> Vec<&File> {
        let copy = self
            .instance(viewer)
            .into_iter()
            .flat_map(|instance| &instance.files);
        let loose = self.drops.iter().chain(copy);
        let dropped = loose.filter(|&&(at, _)| at == host).map(|(_, file)| file);
        self.hosts[host.0].files.iter().chain(dropped).collect()
    }

    fn take_file(&mut self, player: PlayerId, host: HostId, name: &str) {
        if self.hosts[host.0]
            .files
            .iter()
            .any(|file| file.name == name)
        {
            return;
        }
        let here = |&(at, ref file): &(HostId, File)| at == host && file.name == name;
        if let Some(index) = self.drops.iter().position(here) {
            self.drops.remove(index);
        } else if let Some(instance) = self.instance_mut(player)
            && let Some(index) = instance.files.iter().position(here)
        {
            instance.files.remove(index);
        }
    }
}

fn names<'a>(files: &[&'a File]) -> Vec<&'a str> {
    files.iter().map(|file| file.name.as_str()).collect()
}

fn pick(args: &[String], files: &[&File]) -> Result<Vec<File>, Message> {
    let names = names(files);
    args.iter()
        .map(|arg| {
            let name = choose(Thing::File, arg, &names)?;
            let index = names.iter().position(|&known| known == name).unwrap_or(0);
            Ok(files[index].clone())
        })
        .collect()
}

fn piped_files(command: &str, items: Vec<Item>) -> Result<Vec<File>, Message> {
    let files: Vec<File> = items
        .into_iter()
        .filter_map(|item| match item {
            Item::File(file) => Some(file),
            Item::Route(_) | Item::Ice { .. } | Item::Rig { .. } | Item::Line(_) => None,
        })
        .collect();
    if files.is_empty() {
        return Err(Message::error(format!(
            "{command} needs files, but the input has none."
        )));
    }
    Ok(files)
}

#[cfg(test)]
mod tests {
    use crate::testing::{run, walk};

    #[test]
    fn read_shows_the_lines_of_a_file() {
        assert_eq!(
            run("read motd.txt"),
            ["Welcome to the Relay.", "Be polite."]
        );
    }

    #[test]
    fn cat_is_another_name_for_read() {
        assert_eq!(run("cat rules.txt"), ["No spam."]);
    }

    #[test]
    fn read_accepts_a_unique_prefix() {
        assert_eq!(run("read mo"), run("read motd.txt"));
    }

    #[test]
    fn read_names_each_file_when_it_shows_more_than_one() {
        assert_eq!(
            run("read motd.txt rules.txt"),
            [
                "motd.txt",
                "Welcome to the Relay.",
                "Be polite.",
                "rules.txt",
                "No spam.",
            ]
        );
    }

    #[test]
    fn read_takes_piped_files() {
        assert_eq!(run("ls | grep rules | read"), ["No spam."]);
        assert_eq!(run("ls | read"), run("read motd.txt rules.txt"));
    }

    #[test]
    fn grep_filters_the_lines_of_a_file() {
        assert_eq!(run("read motd.txt | grep polite"), ["Be polite."]);
    }

    #[test]
    fn read_needs_files_in_its_input() {
        assert_eq!(
            run("ls | grep market | read"),
            ["read needs files, but the input has none."]
        );
    }

    #[test]
    fn an_unknown_file_lists_the_files() {
        assert_eq!(
            run("read secrets.txt"),
            ["No file named secrets.txt. Files: motd.txt, rules.txt"]
        );
    }

    #[test]
    fn a_misspelled_file_gets_a_suggestion() {
        assert_eq!(
            run("read mtod.txt"),
            ["No file named mtod.txt. Did you mean motd.txt?"]
        );
    }

    #[test]
    fn read_needs_a_file() {
        assert_eq!(
            run("read"),
            ["Usage: read <file>. Files: motd.txt, rules.txt"]
        );
    }

    #[test]
    fn read_takes_a_file_or_a_pipe_but_not_both() {
        assert_eq!(
            run("ls | read motd.txt"),
            ["read takes a file or a pipe, not both."]
        );
    }

    #[test]
    fn download_copies_a_file_to_the_rig() {
        assert_eq!(run("download motd.txt"), ["Downloaded motd.txt."]);
        assert_eq!(
            walk(&["download motd.txt", "go market", "read motd.txt"]).0,
            ["Welcome to the Relay.", "Be polite."]
        );
    }

    #[test]
    fn get_is_another_name_for_download() {
        assert_eq!(run("get motd.txt"), ["Downloaded motd.txt."]);
    }

    #[test]
    fn download_takes_piped_files() {
        assert_eq!(
            run("ls | grep txt | download"),
            ["Downloaded motd.txt.", "Downloaded rules.txt."]
        );
    }

    #[test]
    fn a_file_downloads_only_once() {
        assert_eq!(
            walk(&["download motd.txt", "download motd.txt"]).0,
            ["motd.txt is already on your rig."]
        );
    }

    #[test]
    fn download_needs_a_file_on_this_host() {
        assert_eq!(
            walk(&["download motd.txt", "go market", "download motd.txt"]).0,
            ["No file named motd.txt. Files: prices.db"]
        );
    }

    #[test]
    fn download_needs_a_file() {
        assert_eq!(
            run("download"),
            ["Usage: download <file>. Files: motd.txt, rules.txt"]
        );
    }

    #[test]
    fn inventory_lists_the_files_on_the_rig() {
        assert_eq!(walk(&["download motd.txt", "inventory"]).0, ["motd.txt"]);
    }

    #[test]
    fn inv_and_i_are_other_names_for_inventory() {
        for name in ["inv", "i"] {
            assert_eq!(walk(&["get motd.txt", name]).0, ["motd.txt"], "{name}");
        }
    }

    #[test]
    fn an_empty_rig_says_so() {
        assert_eq!(run("inventory"), ["Your rig has no files."]);
    }

    #[test]
    fn inventory_pipes_its_files() {
        assert_eq!(
            walk(&["get motd.txt", "go market", "inv | read"]).0,
            ["Welcome to the Relay.", "Be polite."]
        );
    }

    #[test]
    fn download_gives_no_output_to_a_pipe() {
        assert_eq!(
            run("download motd.txt | grep x"),
            ["download gives no output to a pipe."]
        );
    }
}
