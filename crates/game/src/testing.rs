use crate::{Content, Output, PlayerId, World};

pub(crate) const CITY: &str = include_str!("../tests/scenarios/city.ini");

pub(crate) fn world() -> World {
    World::new(Content::parse(CITY).unwrap())
}

pub(crate) fn plain(outputs: &[Output]) -> Vec<String> {
    outputs
        .iter()
        .map(|output| output.message.plain())
        .collect()
}

pub(crate) fn heard(outputs: &[Output], player: PlayerId) -> Vec<String> {
    outputs
        .iter()
        .filter(|output| output.player == player)
        .map(|output| output.message.plain())
        .collect()
}

pub(crate) fn joined() -> (World, PlayerId) {
    let mut world = world();
    let (player, _) = world.join("ghost");
    (world, player)
}

pub(crate) fn walk(lines: &[&str]) -> (Vec<String>, Option<String>) {
    let (mut world, player) = joined();
    let mut last = Vec::new();
    for line in lines {
        last = plain(&world.command(player, line));
    }
    (last, world.prompt(player))
}

pub(crate) fn run(line: &str) -> Vec<String> {
    walk(&[line]).0
}
