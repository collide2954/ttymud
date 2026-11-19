use std::io;
use std::time::{SystemTime, UNIX_EPOCH};

use ttymud_game::{Content, World};

const PORT_OSTRA: &str = include_str!("../content/port-ostra.ini");

pub fn port_ostra(seed: u64) -> io::Result<World> {
    let content = Content::parse(PORT_OSTRA).map_err(|errors| {
        let lines: Vec<String> = errors.iter().map(ToString::to_string).collect();
        io::Error::other(lines.join("\n"))
    })?;
    Ok(World::new(content, seed))
}

#[must_use]
pub fn seed() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| {
            since.as_secs() ^ (u64::from(since.subsec_nanos()) << 32)
        })
}
