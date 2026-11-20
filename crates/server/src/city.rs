use std::fs;
use std::io;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use ttymud_game::{Content, World};

use crate::store::load_saves;

const PORT_OSTRA: &str = include_str!("../content/port-ostra.ini");
const BUILT_IN: &str = "port-ostra.ini";
const CITY: &str = "city.ini";

pub fn port_ostra(seed: u64) -> io::Result<World> {
    Ok(World::new(parse(BUILT_IN, PORT_OSTRA)?, seed))
}

pub fn load_world(dir: &Path) -> io::Result<World> {
    let mut world = World::new(city(dir)?, seed());
    let mut problems = Vec::new();
    for (path, text) in load_saves(dir)? {
        if let Err(errors) = world.restore(&path, &text) {
            problems.extend(errors.iter().map(|error| format!("{path}: {error}")));
        }
    }
    if problems.is_empty() {
        Ok(world)
    } else {
        Err(io::Error::other(problems.join("\n")))
    }
}

fn city(dir: &Path) -> io::Result<Content> {
    match fs::read_to_string(dir.join(CITY)) {
        Ok(text) => parse(CITY, &text),
        Err(error) if error.kind() == io::ErrorKind::NotFound => parse(BUILT_IN, PORT_OSTRA),
        Err(error) => Err(io::Error::new(error.kind(), format!("{CITY}: {error}"))),
    }
}

fn parse(name: &str, text: &str) -> io::Result<Content> {
    Content::parse(text).map_err(|errors| {
        let lines: Vec<String> = errors
            .iter()
            .map(|error| format!("{name}: {error}"))
            .collect();
        io::Error::other(lines.join("\n"))
    })
}

pub(crate) fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_secs())
}

pub(crate) fn seed() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| {
            since.as_secs() ^ (u64::from(since.subsec_nanos()) << 32)
        })
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};

    use super::{load_world, now};
    use crate::testing::{LAB, scratch};

    fn data(name: &str, files: &[(&str, &str)]) -> PathBuf {
        let dir = scratch(name);
        fs::create_dir_all(&dir).unwrap();
        for (file, text) in files {
            fs::write(dir.join(file), text).unwrap();
        }
        dir
    }

    fn problems(dir: &Path) -> Vec<String> {
        let error = load_world(dir).err().map(|error| error.to_string());
        error
            .unwrap_or_default()
            .lines()
            .map(str::to_owned)
            .collect()
    }

    #[test]
    fn a_data_directory_without_a_city_file_runs_port_ostra() {
        let dir = data("city-default", &[]);
        assert_eq!(load_world(&dir).unwrap().city(), "Port Ostra");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn city_ini_in_the_data_directory_replaces_port_ostra() {
        let dir = data("city-own", &[("city.ini", LAB)]);
        assert_eq!(load_world(&dir).unwrap().city(), "Lab");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn errors_in_city_ini_name_the_file_and_the_line() {
        let city = "[city lab]\nname = Lab\nstart = nowhere\n";
        let dir = data("city-broken", &[("city.ini", city)]);
        assert_eq!(
            problems(&dir),
            ["city.ini: line 3: start is an unknown host: nowhere"]
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_world_shows_the_errors_of_all_save_files() {
        let files = [("news.ini", "[gossip x]\n"), ("burns.ini", "[rumor y]\n")];
        let dir = data("city-saves", &files);
        assert_eq!(
            problems(&dir),
            [
                "burns.ini: line 1: unknown section type: rumor",
                "news.ini: line 1: unknown section type: gossip",
            ]
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn now_is_the_unix_time_in_seconds() {
        let now = now();
        assert!((1_700_000_000..10_000_000_000).contains(&now), "{now}");
    }
}
