use crate::{Content, Output, PlayerId, World};

pub(crate) const CITY: &str = "\
[city test]
name = Test City
start = relay

[host relay]
name = The Relay
desc = A crowded public node.
route = market
route = back-room
route = bazaar

[host market]
name = The Night Market
desc = Stalls sell cracked firmware.

[host back-room]
name = The Back Room
desc = Fixers drink here.

[host bazaar]
name = The Bazaar
desc = Everything is for sale.
route = market

[file relay/motd.txt]
line = Welcome to the Relay.
line = Be polite.

[file relay/rules.txt]
line = No spam.

[file market/prices.db]
line = crack v1.0 costs 200 cr.
";

pub(crate) fn world() -> World {
    World::new(Content::parse(CITY).unwrap())
}

pub(crate) fn plain(outputs: &[Output]) -> Vec<String> {
    outputs
        .iter()
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
