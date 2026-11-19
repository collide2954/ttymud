use crate::{Content, Output, PlayerId, World};

pub(crate) const CITY: &str = include_str!("../tests/scenarios/city.ini");

pub(crate) const JOB: &str = "\
[contract lab-job]
fixer = pell
network = lab
goal = data.db
pay = 100
offer = The lab has data that a client wants.
offer = Get data.db and leave clean.
step = home: Go to back-room. Your rig is there.
step = enter: Type go gate to connect to the lab.
step = scan: Type scan to find the ICE.
step = download: Go to core and type download data.db.
step = exit: Type back until you leave the lab.
";

pub(crate) fn world() -> World {
    World::new(Content::parse(CITY).unwrap(), 0)
}

pub(crate) fn hired_world() -> World {
    let city = CITY.replace("home = back-room", "home = back-room\ncontract = lab-job");
    World::new(Content::parse(&format!("{city}{JOB}")).unwrap(), 0)
}

pub(crate) fn hired() -> (World, PlayerId) {
    let mut world = hired_world();
    let (player, _) = world.join("ghost");
    (world, player)
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

pub(crate) fn hacker(world: &mut World, account: &str, name: &str) -> PlayerId {
    let (player, _) = world.connect(account);
    for line in [name, "ghost"] {
        world.command(player, line);
    }
    player
}

pub(crate) fn hired_at(time: u64) -> (World, PlayerId) {
    let mut world = world();
    world.set_time(time);
    let (player, _) = world.join("ghost");
    for line in ["go bazaar", "accept 1", "go relay", "go back-room"] {
        world.command(player, line);
    }
    (world, player)
}

pub(crate) fn enter_job(world: &mut World, player: PlayerId) -> Vec<String> {
    let id = world.job_entry();
    plain(&world.command(player, &format!("go {id}")))
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
        last = plain(&step(&mut world, player, line));
    }
    (last, world.prompt(player))
}

pub(crate) fn step(world: &mut World, player: PlayerId, line: &str) -> Vec<Output> {
    let Some(count) = line.strip_prefix('~') else {
        return world.command(player, line);
    };
    let count: usize = count.parse().unwrap_or(1);
    (0..count).flat_map(|_| world.tick()).collect()
}

pub(crate) fn run(line: &str) -> Vec<String> {
    walk(&[line]).0
}
