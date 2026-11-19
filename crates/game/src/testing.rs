use crate::{Content, Output, World};

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
