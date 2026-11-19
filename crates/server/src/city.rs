use std::io;

use ttymud_game::{Content, World};

const PORT_OSTRA: &str = include_str!("../content/port-ostra.ini");

pub fn port_ostra() -> io::Result<World> {
    let content = Content::parse(PORT_OSTRA).map_err(|errors| {
        let lines: Vec<String> = errors.iter().map(ToString::to_string).collect();
        io::Error::other(lines.join("\n"))
    })?;
    Ok(World::new(content))
}
