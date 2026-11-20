use ttymud_text::{Kind, Message, Style};

use super::{PlayerId, Step, Thing, World, choose, lines, usage};
use crate::content::{File, is_script};

pub(super) const MAX_LINES: usize = 20;
const MAX_SCRIPTS: usize = 10;
const NAME_RULE: &str = "A script name has 1 to 16 lowercase letters, digits, or hyphens, and ends in .sh. Example: sweep.sh";

#[derive(Clone, Copy)]
enum Write {
    Replace,
    Append,
}

impl World {
    pub(super) fn script_names(&self, player: PlayerId) -> Vec<&str> {
        self.players
            .get(&player)
            .into_iter()
            .flat_map(|state| &state.rig.files)
            .filter(|file| is_script(&file.name))
            .map(|file| file.name.as_str())
            .collect()
    }

    pub(super) fn scripts(&self, player: PlayerId) -> Vec<String> {
        let names = self.script_names(player);
        names.into_iter().map(str::to_owned).collect()
    }

    pub(super) fn echo(&mut self, player: PlayerId, args: &[String]) -> Result<Step, Message> {
        let Some((words, write, name)) = redirect(args) else {
            return Ok(lines(vec![Message::game(args.join(" "))]));
        };
        if !is_script(name) {
            return Err(Message::error(NAME_RULE));
        }
        let Some(state) = self.players.get_mut(&player) else {
            return Ok(Step::default());
        };
        let count = write_line(&mut state.rig.files, name, write, words.join(" "))?;
        let unit = if count == 1 { "line" } else { "lines" };
        Ok(Step::show(vec![
            Message::new(Kind::Game)
                .span(Style::File, name)
                .text(format!(" has {count} {unit}.")),
        ]))
    }

    pub(super) fn rm(&mut self, player: PlayerId, args: &[String]) -> Result<Step, Message> {
        if let Some(step) = self.delete_here(player, args)? {
            return Ok(step);
        }
        let scripts = self.scripts(player);
        let names: Vec<&str> = scripts.iter().map(String::as_str).collect();
        if args.is_empty() {
            return Err(usage("rm <script>", Thing::Script, &names));
        }
        let mut doomed = Vec::new();
        for arg in args {
            let name = exact(arg, &names)?;
            if !doomed.contains(&name) {
                doomed.push(name);
            }
        }
        if let Some(state) = self.players.get_mut(&player) {
            state
                .rig
                .files
                .retain(|file| !doomed.contains(&file.name.as_str()));
        }
        let shown = doomed
            .into_iter()
            .map(|name| {
                Message::game("You delete ")
                    .span(Style::File, name)
                    .text(".")
            })
            .collect();
        Ok(Step::show(shown))
    }
}

fn write_line(
    files: &mut Vec<File>,
    name: &str,
    write: Write,
    text: String,
) -> Result<usize, Message> {
    if let Some(file) = files.iter_mut().find(|file| file.name == name) {
        match write {
            Write::Replace => file.lines = vec![text],
            Write::Append if file.lines.len() >= MAX_LINES => {
                return Err(Message::error("")
                    .span(Style::File, name)
                    .text(format!(" has {MAX_LINES} lines, the maximum for a script.")));
            }
            Write::Append => file.lines.push(text),
        }
        return Ok(file.lines.len());
    }
    if files.iter().filter(|file| is_script(&file.name)).count() >= MAX_SCRIPTS {
        return Err(Message::error(format!(
            "Your rig holds {MAX_SCRIPTS} scripts at most. Type rm <script> to delete one."
        )));
    }
    files.push(File::new(name, vec![text]));
    Ok(1)
}

fn redirect(args: &[String]) -> Option<(&[String], Write, &str)> {
    let (last, words) = args.split_last()?;
    let write = |word: &str| match word {
        ">" => Some(Write::Replace),
        ">>" => Some(Write::Append),
        _ => None,
    };
    if let [words @ .., arrow] = words
        && let Some(write) = write(arrow)
    {
        return Some((words, write, last));
    }
    if let Some(name) = last.strip_prefix(">>") {
        return Some((words, Write::Append, name));
    }
    last.strip_prefix('>')
        .map(|name| (words, Write::Replace, name))
}

fn exact<'a>(word: &str, names: &[&'a str]) -> Result<&'a str, Message> {
    if let Some(&name) = names.iter().find(|&&name| name == word) {
        return Ok(name);
    }
    let near = choose(Thing::Script, word, names)?;
    Err(
        Message::error(format!("No script named {word}. Did you mean "))
            .span(Style::File, near)
            .text("?"),
    )
}

#[cfg(test)]
mod tests {
    use crate::content::is_script;
    use crate::testing::{plain, run, walk, world};

    #[test]
    fn echo_shows_its_text() {
        assert_eq!(run("echo hello   world"), ["hello world"]);
        assert_eq!(run("echo 'a  b' | grep a"), ["a  b"]);
    }

    #[test]
    fn echo_writes_a_script_and_adds_lines_to_it() {
        let lines = [
            "echo 'scan | probe' > sweep.sh",
            "echo 'crack $1 || jackout' >> sweep.sh",
        ];
        assert_eq!(walk(&lines[..1]).0, ["sweep.sh has 1 line."]);
        assert_eq!(walk(&lines).0, ["sweep.sh has 2 lines."]);
        assert_eq!(
            walk(&[&lines[..], &["cat sweep.sh"]].concat()).0,
            ["scan | probe", "crack $1 || jackout"]
        );
        assert_eq!(walk(&[&lines[..], &["inventory"]].concat()).0, ["sweep.sh"]);
    }

    #[test]
    fn a_single_arrow_replaces_the_script() {
        let shown = walk(&[
            "echo a >> x.sh",
            "echo b >> x.sh",
            "echo c > x.sh",
            "cat x.sh",
        ]);
        assert_eq!(shown.0, ["c"]);
    }

    #[test]
    fn the_arrow_can_touch_the_name() {
        let shown = walk(&["echo look >x.sh", "echo ls >>x.sh", "cat x.sh"]);
        assert_eq!(shown.0, ["look", "ls"]);
    }

    #[test]
    fn a_script_name_ends_in_sh() {
        let rule = "A script name has 1 to 16 lowercase letters, digits, or hyphens, and ends in .sh. Example: sweep.sh";
        for line in [
            "echo x > notes.txt",
            "echo x > Sweep.sh",
            "echo x > a_b.sh",
            "echo x > .sh",
            "echo x > abcdefghijklmnopq.sh",
            "echo x >",
            "echo x >>",
        ] {
            assert_eq!(run(line), [rule], "{line}");
        }
        assert!(is_script("abcdefghijklmnop.sh"));
        assert!(is_script("dig-2a.sh"));
    }

    #[test]
    fn a_script_has_20_lines_at_most() {
        let (mut world, player) = crate::testing::joined();
        for _ in 0..20 {
            world.command(player, "echo ls >> x.sh");
        }
        assert_eq!(
            plain(&world.command(player, "echo ls >> x.sh")),
            ["x.sh has 20 lines, the maximum for a script."]
        );
        assert_eq!(
            plain(&world.command(player, "echo ls > x.sh")),
            ["x.sh has 1 line."]
        );
    }

    #[test]
    fn a_rig_holds_10_scripts_at_most() {
        let (mut world, player) = crate::testing::joined();
        for index in 0..10 {
            world.command(player, &format!("echo ls > s{index}.sh"));
        }
        assert_eq!(
            plain(&world.command(player, "echo ls > more.sh")),
            ["Your rig holds 10 scripts at most. Type rm <script> to delete one."]
        );
        assert_eq!(
            plain(&world.command(player, "echo look >> s3.sh")),
            ["s3.sh has 2 lines."]
        );
    }

    #[test]
    fn rm_deletes_scripts_from_the_rig() {
        let lines = ["echo ls > a.sh", "echo ls > b.sh", "get motd.txt"];
        assert_eq!(
            walk(&[&lines[..], &["rm a.sh b.sh a.sh"]].concat()).0,
            ["You delete a.sh.", "You delete b.sh."]
        );
        assert_eq!(
            walk(&[&lines[..], &["rm a.sh", "inventory"]].concat()).0,
            ["b.sh", "motd.txt"]
        );
    }

    #[test]
    fn rm_needs_the_exact_name_of_a_script() {
        let lines = ["echo ls > sweep.sh", "get motd.txt"];
        let shown = |line: &str| walk(&[&lines[..], &[line]].concat()).0;
        assert_eq!(shown("rm"), ["Usage: rm <script>. Scripts: sweep.sh"]);
        assert_eq!(
            shown("rm sw"),
            ["No script named sw. Did you mean sweep.sh?"]
        );
        assert_eq!(
            shown("rm motd.txt"),
            ["No script named motd.txt. Scripts: sweep.sh"]
        );
        assert_eq!(run("rm x.sh"), ["No script named x.sh. Scripts: none"]);
    }

    #[test]
    fn a_script_stays_on_the_rig_after_a_restart() {
        let mut first = world();
        let (player, _) = first.connect("a");
        for line in [
            "nyx",
            "ghost",
            "echo 'status | grep \"Trace: 9\" && jackout' > watch.sh",
            "echo '# $1 is a host' >> watch.sh",
        ] {
            first.command(player, line);
        }
        first.leave(player);
        let saves = first.saves();
        let (_, text) = saves
            .iter()
            .find(|(path, _)| path == "accounts/a.ini")
            .unwrap();
        let mut second = world();
        second.restore("accounts/a.ini", text).unwrap();
        let (player, _) = second.connect("a");
        assert_eq!(
            plain(&second.command(player, "cat watch.sh")),
            ["status | grep \"Trace: 9\" && jackout", "# $1 is a host"]
        );
    }
}
