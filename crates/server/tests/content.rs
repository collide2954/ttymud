use ttymud_server::port_ostra;

#[test]
fn port_ostra_is_valid() {
    let world = port_ostra(0);
    assert!(world.is_ok(), "{:?}", world.err());
}

#[test]
fn port_ostra_starts_at_the_relay() {
    let mut world = port_ostra(0).unwrap();
    let (player, outputs) = world.join("guest");
    assert_eq!(outputs[0].message.plain(), "The Relay");
    assert_eq!(world.prompt(player).as_deref(), Some("guest@relay $ "));
}

#[test]
fn port_ostra_has_three_channels_and_starts_players_in_two() {
    let mut world = port_ostra(0).unwrap();
    let (player, _) = world.join("guest");
    let shown: Vec<String> = world
        .command(player, "channels")
        .iter()
        .map(|out| out.message.plain())
        .collect();
    assert_eq!(
        shown[..3],
        [
            "#ostra: All of Port Ostra. Talk about anything. You are in this channel.",
            "#help: Ask questions about the game. Other players answer. You are in this channel.",
            "#market: Buy, sell, and trade.",
        ]
    );
}

#[test]
fn pell_offers_the_first_job() {
    let (_, outputs) = port_ostra(0).unwrap().join("guest");
    let shown: Vec<String> = outputs.iter().map(|out| out.message.plain()).collect();
    assert!(
        shown.contains(&"Pell sends you a job:".to_owned()),
        "{shown:?}"
    );
}

#[test]
fn the_first_job_can_be_done() {
    let mut world = port_ostra(0).unwrap();
    let (player, _) = world.join("guest");
    let lines = [
        "go safehouse",
        "go smile-gw",
        "scan",
        "~",
        "~",
        "probe sentry | crack",
        "go front-desk",
        "go records",
        "get patients.db",
        "back",
        "back",
        "back",
    ];
    let mut last = Vec::new();
    for line in lines {
        let outputs = if line == "~" {
            world.tick()
        } else {
            world.command(player, line)
        };
        last = outputs.iter().map(|out| out.message.plain()).collect();
    }
    let done = "Pell: Clean work. The contract is complete.".to_owned();
    assert!(last.contains(&done), "{last:?}");
}

#[test]
fn the_fixers_post_heists_and_the_shop_sells_decoy() {
    let mut world = port_ostra(0).unwrap();
    world.set_time(20_000 * 86_400 + 12 * 3_600);
    let (player, _) = world.join("guest");
    let mut run = |line: &str| -> Vec<String> {
        let outputs = world.command(player, line);
        outputs.iter().map(|out| out.message.plain()).collect()
    };
    run("go back-room");
    let board = run("jobs");
    let heading = board
        .iter()
        .position(|line| line == "Heists for crews today:");
    let heists = heading.map_or(0, |heading| board.len() - heading - 3);
    assert_eq!(heists, 2, "{board:?}");
    let (ghost, _) = world.connect("a");
    let mut ask = |line: &str| -> Vec<String> {
        let outputs = world.command(ghost, line);
        outputs.iter().map(|out| out.message.plain()).collect()
    };
    for line in ["nyx", "ghost", "go night-market"] {
        ask(line);
    }
    let shop = ask("shop");
    assert!(
        shop.iter().any(|line| line.starts_with("decoy: 500 cr.")),
        "{shop:?}"
    );
}

#[test]
fn the_market_sells_blueprints_and_the_evidence_locker_holds_the_auction() {
    let mut world = port_ostra(0).unwrap();
    let (player, _) = world.join("guest");
    let mut run = |line: &str| -> Vec<String> {
        let outputs = world.command(player, line);
        outputs.iter().map(|out| out.message.plain()).collect()
    };
    run("go night-market");
    let shop = run("shop");
    assert!(
        shop.iter()
            .any(|line| line.starts_with("crack-v2: 600 cr. The blueprint for crack v2.")),
        "{shop:?}"
    );
    run("go evidence-locker");
    assert_eq!(run("lots"), ["The police have no lots for auction now."]);
    let notice = run("read notice.txt");
    assert!(
        notice
            .iter()
            .any(|line| line.contains("bid <lot> <credits>")),
        "{notice:?}"
    );
}

#[test]
fn the_hosts_of_a_job_run_software_that_scan_shows() {
    let mut world = port_ostra(0).unwrap();
    world.set_time(20_000 * 86_400 + 12 * 3_600);
    let (player, _) = world.join("guest");
    let mut run = |line: &str| -> Vec<String> {
        let outputs = world.command(player, line);
        outputs.iter().map(|out| out.message.plain()).collect()
    };
    for line in ["contract drop", "go back-room", "accept 1", "go relay"] {
        run(line);
    }
    run("go safehouse");
    let routes = run("ls");
    let entry = routes
        .iter()
        .rfind(|route| route.ends_with('/'))
        .map_or_default(|route| route.trim_end_matches('/').to_owned());
    run(&format!("go {entry}"));
    let shown = run("scan");
    assert!(
        shown.iter().any(|line| line.starts_with("Software: ")),
        "{shown:?}"
    );
}
