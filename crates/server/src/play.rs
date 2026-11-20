use std::io::{self, BufRead, Write};
use std::path::Path;
use std::sync::mpsc::{self, RecvTimeoutError};
use std::thread;
use std::time::{Duration, Instant};

use ttymud_game::{Output, PlayerId, World};

use crate::city::{city_world, now};
use crate::ticker::{TICK, Ticker};

pub fn play(
    name: Option<&str>,
    city: Option<&Path>,
    input: impl BufRead + Send + 'static,
    output: impl Write,
) -> io::Result<()> {
    session(city_world(city)?, name, input, output, TICK)
}

fn session(
    mut world: World,
    name: Option<&str>,
    input: impl BufRead + Send + 'static,
    mut output: impl Write,
    every: Duration,
) -> io::Result<()> {
    world.set_time(now());
    let (player, joined) = world.connect("offline");
    let city = world.city();
    writeln!(
        output,
        "Offline mode. You are alone in {city}. Type quit to leave."
    )?;
    show(&mut output, &joined)?;
    if let Some(name) = name {
        show(&mut output, &world.command(player, name))?;
    }
    prompt(&mut output, &world, player)?;
    let lines = read(input)?;
    let mut ticker = Ticker::new(every, Instant::now());
    loop {
        match lines.recv_timeout(ticker.left(Instant::now())) {
            Ok(line) => {
                let line = line?;
                if line.trim() == "quit" {
                    return Ok(());
                }
                world.set_time(now());
                show(&mut output, &world.command(player, &line))?;
                prompt(&mut output, &world, player)?;
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return writeln!(output),
        }
        if ticker.due(Instant::now()) {
            world.set_time(now());
            let outputs = world.tick();
            if !outputs.is_empty() {
                writeln!(output)?;
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
    use std::fs;
    use std::io::{BufReader, Cursor, Write};
    use std::path::PathBuf;
    use std::thread;
    use std::time::Duration;

    use super::{play as run, session};
    use crate::city::city_world;
    use crate::testing::{LAB, scratch};

    #[test]
    fn text_from_a_tick_starts_on_a_new_line() {
        let (reader, mut writer) = std::io::pipe().unwrap();
        let typist = thread::spawn(move || {
            writer
                .write_all(b"ghost\ngo safehouse\ngo smile-gw\nscan\n")
                .unwrap();
            thread::sleep(Duration::from_millis(400));
            writer.write_all(b"quit\n").unwrap();
        });
        let mut output = Vec::new();
        let every = Duration::from_millis(20);
        let world = city_world(None).unwrap();
        session(
            world,
            Some("nyx"),
            BufReader::new(reader),
            &mut output,
            every,
        )
        .unwrap();
        typist.join().unwrap();
        let text = String::from_utf8(output).unwrap();
        assert!(
            text.contains("nyx@smile-gw $ \nsentry sends an ALERT."),
            "{text}"
        );
    }

    fn play(name: Option<&str>, input: &str) -> String {
        let mut output = Vec::new();
        run(
            name,
            None,
            Cursor::new(input.as_bytes().to_vec()),
            &mut output,
        )
        .unwrap();
        String::from_utf8(output).unwrap()
    }

    #[test]
    fn play_starts_with_a_new_handle() {
        let text = play(None, "");
        assert!(text.starts_with(
            "Offline mode. You are alone in Port Ostra. Type quit to leave.\nYou are new in Port Ostra."
        ));
        assert!(text.ends_with("handle: \n"), "{text}");
    }

    #[test]
    fn a_name_answers_the_first_question() {
        let text = play(Some("nyx"), "");
        assert!(text.contains("Choose a starter kit:"), "{text}");
        assert!(text.ends_with("kit: \n"), "{text}");
    }

    #[test]
    fn play_runs_each_line_as_a_command() {
        let text = play(Some("nyx"), "ghost\nls\n");
        assert!(text.contains("nyx@relay $ night-market/\n"), "{text}");
    }

    #[test]
    fn a_byte_order_mark_before_a_command_is_ignored() {
        let text = play(Some("nyx"), "ghost\n\u{feff}ls\n");
        assert!(text.contains("nyx@relay $ night-market/\n"), "{text}");
    }

    #[test]
    fn quit_ends_the_session() {
        assert!(!play(Some("nyx"), "ghost\nquit\nls\n").contains("night-market/"));
    }

    fn city_file(name: &str, text: &str) -> PathBuf {
        let dir = scratch(name);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("lab.ini");
        fs::write(&path, text).unwrap();
        path
    }

    #[test]
    fn play_walks_the_city_of_a_file() {
        let path = city_file("play-city", LAB);
        let mut output = Vec::new();
        run(None, Some(&path), Cursor::new(Vec::new()), &mut output).unwrap();
        let text = String::from_utf8(output).unwrap();
        assert!(
            text.starts_with("Offline mode. You are alone in Lab. Type quit to leave."),
            "{text}"
        );
        let _ = fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn play_shows_the_errors_of_a_city_file() {
        let path = city_file("play-broken", "[city lab]\nname = Lab\nstart = nowhere\n");
        let input = Cursor::new(Vec::new());
        let error = run(None, Some(&path), input, Vec::new()).unwrap_err();
        assert_eq!(
            error.to_string(),
            format!(
                "{}: line 3: start is an unknown host: nowhere",
                path.display()
            )
        );
        let _ = fs::remove_dir_all(path.parent().unwrap());
    }
}
