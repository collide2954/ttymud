use std::io::{self, BufRead, Write};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::thread;
use std::time::Instant;

use ttymud_game::{Output, PlayerId, World};

use crate::port_ostra;
use crate::ticker::{TICK, Ticker};

pub fn play(
    name: &str,
    input: impl BufRead + Send + 'static,
    mut output: impl Write,
) -> io::Result<()> {
    let mut world = port_ostra()?;
    let (player, joined) = world.join(name);
    let city = world.city();
    writeln!(
        output,
        "Offline mode. You are alone in {city}. Type quit to leave."
    )?;
    show(&mut output, &joined)?;
    prompt(&mut output, &world, player)?;
    let lines = read(input)?;
    let mut ticker = Ticker::new(TICK, Instant::now());
    loop {
        match lines.recv_timeout(ticker.left(Instant::now())) {
            Ok(line) => {
                let line = line?;
                if line.trim() == "quit" {
                    return Ok(());
                }
                show(&mut output, &world.command(player, &line))?;
                prompt(&mut output, &world, player)?;
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return writeln!(output),
        }
        if ticker.due(Instant::now()) {
            let outputs = world.tick();
            if !outputs.is_empty() {
                show(&mut output, &outputs)?;
                prompt(&mut output, &world, player)?;
            }
        }
    }
}

fn read(input: impl BufRead + Send + 'static) -> io::Result<mpsc::Receiver<io::Result<String>>> {
    let (sender, receiver) = mpsc::channel();
    thread::Builder::new()
        .name("input".to_owned())
        .spawn(move || {
            for line in input.lines() {
                if sender.send(line).is_err() {
                    return;
                }
            }
        })?;
    Ok(receiver)
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
    use std::io::Cursor;

    use super::play as run;

    fn play(input: &str) -> String {
        let mut output = Vec::new();
        run("nyx", Cursor::new(input.as_bytes().to_vec()), &mut output).unwrap();
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
