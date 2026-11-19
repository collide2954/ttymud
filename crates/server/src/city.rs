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

pub(crate) fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_secs())
}

#[must_use]
pub fn seed() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| {
            since.as_secs() ^ (u64::from(since.subsec_nanos()) << 32)
        })
}

#[cfg(test)]
mod tests {
    use super::now;

    #[test]
    fn now_is_the_unix_time_in_seconds() {
        let now = now();
        assert!((1_700_000_000..10_000_000_000).contains(&now), "{now}");
    }
}
