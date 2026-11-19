use std::io::{self, BufRead, Write};

use ttymud_game::{Content, Output, PlayerId, World};

const PORT_OSTRA: &str = include_str!("../content/port-ostra.ini");

pub(crate) fn run(name: &str, input: impl BufRead, mut output: impl Write) -> io::Result<()> {
    let content = Content::parse(PORT_OSTRA).map_err(|errors| {
        let lines: Vec<String> = errors.iter().map(ToString::to_string).collect();
        io::Error::other(lines.join("\n"))
    })?;
    let mut world = World::new(content);
    let (player, joined) = world.join(name);
    let city = world.city();
    writeln!(
        output,
        "Offline mode. You are alone in {city}. Type quit to leave."
    )?;
    show(&mut output, &joined)?;
    prompt(&mut output, &world, player)?;
    for line in input.lines() {
        let line = line?;
        let line = line.trim_start_matches('\u{feff}');
        if line.trim() == "quit" {
            return Ok(());
        }
        show(&mut output, &world.command(player, line))?;
        prompt(&mut output, &world, player)?;
    }
    writeln!(output)
}

fn show(output: &mut impl Write, outputs: &[Output]) -> io::Result<()> {
    for out in outputs {
        writeln!(output, "{}", out.message.plain())?;
    }
    Ok(())
}

fn prompt(output: &mut impl Write, world: &World, player: PlayerId) -> io::Result<()> {
    write!(output, "{}", world.prompt(player).unwrap_or_default())?;
    output.flush()
}

#[cfg(test)]
mod tests {
    use super::run;

    fn play(input: &str) -> String {
        let mut output = Vec::new();
        run("nyx", input.as_bytes(), &mut output).unwrap();
        String::from_utf8(output).unwrap()
    }

    #[test]
    fn play_greets_the_player_in_port_ostra() {
        assert!(play("").starts_with(
            "Offline mode. You are alone in Port Ostra. Type quit to leave.\nThe Relay\n"
        ));
    }

    #[test]
    fn play_runs_each_line_as_a_command() {
        assert!(play("ls\n").contains("nyx@relay $ night-market/\n"));
    }

    #[test]
    fn a_byte_order_mark_before_a_command_is_ignored() {
        assert!(play("\u{feff}ls\n").contains("nyx@relay $ night-market/\n"));
    }

    #[test]
    fn quit_ends_the_session() {
        assert!(!play("quit\nls\n").contains("night-market/"));
    }
}
