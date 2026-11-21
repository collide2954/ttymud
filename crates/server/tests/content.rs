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
fn the_first_job_has_a_step_for_the_sentry_and_for_the_warden() {
    let mut world = port_ostra(0).unwrap();
    let (player, _) = world.join("guest");
    let mut next = |lines: &[&str]| -> String {
        for &line in lines {
            if line == "~" {
                world.tick();
            } else {
                world.command(player, line);
            }
        }
        let shown = world.command(player, "contract");
        shown.last().map_or_default(|out| out.message.plain())
    };
    assert_eq!(
        next(&["go safehouse", "go smile-gw", "scan"]),
        "Next: Type probe sentry | crack. The probe finds the weak point of the sentry, and the crack hits it."
    );
    assert_eq!(
        next(&["~", "~", "probe sentry | crack"]),
        "Next: A warden guards the route to records. Type probe warden | crack, and then crack warden."
    );
    let warden = [
        "~",
        "~",
        "~",
        "~",
        "probe warden | crack",
        "~",
        "~",
        "~",
        "crack warden",
    ];
    assert_eq!(
        next(&warden),
        "Next: The file is on records. Go there, then type download patients.db."
    );
}

#[test]
fn an_alert_in_the_first_job_wakes_no_hound() {
    let mut world = port_ostra(0).unwrap();
    let (player, _) = world.join("guest");
    for line in ["go safehouse", "go smile-gw", "scan"] {
        world.command(player, line);
    }
    let mut seen = Vec::new();
    for _ in 0..12 {
        seen.extend(world.tick().iter().map(|out| out.message.plain()));
    }
    assert!(
        seen.iter()
            .any(|line| line.starts_with("sentry sends an ALERT.")),
        "{seen:?}"
    );
    assert!(
        !seen.iter().any(|line| line.contains("hunts you")),
        "{seen:?}"
    );
}

#[test]
fn the_sample_script_breaks_the_sentry_of_the_first_job() {
    let mut world = port_ostra(0).unwrap();
    let (player, _) = world.join("guest");
    let lines = [
        "go night-market",
        "get break.sh",
        "back",
        "go safehouse",
        "unload patch",
        "go smile-gw",
    ];
    for line in lines {
        world.command(player, line);
    }
    let started: Vec<String> = world
        .command(player, "sh break.sh sentry")
        .iter()
        .map(|out| out.message.plain())
        .collect();
    assert_eq!(
        started,
        ["break.sh starts. It runs one command on each tick."]
    );
    let mut seen = Vec::new();
    for _ in 0..8 {
        let outputs = world.tick();
        let mine = outputs.iter().filter(|out| out.player == player);
        seen.extend(mine.map(|out| out.message.plain()));
    }
    for line in [
        "break.sh: probe sentry | crack",
        "crack breaks sentry through its weak point.",
        "No match for sentry.",
        "break.sh ends.",
    ] {
        assert!(seen.contains(&line.to_owned()), "{line}: {seen:?}");
    }
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
fn the_undercity_is_a_darknet_under_the_night_market_with_a_better_fence() {
    let mut world = port_ostra(0).unwrap();
    let (player, _) = world.join("guest");
    let mut run = |line: &str| -> Vec<String> {
        let outputs = world.command(player, line);
        outputs.iter().map(|out| out.message.plain()).collect()
    };
    run("go night-market");
    assert_eq!(
        run("go undercity"),
        [
            "The Undercity is a darknet. Other handles there can crash your rig, take your files, and tip off the police. Type go undercity again to agree."
        ]
    );
    assert_eq!(
        run("go undercity")[0],
        "You enter the darknet The Undercity. Other handles can attack you here."
    );
    run("go black-market");
    let shop = run("shop");
    assert!(
        shop.iter()
            .any(|line| line.starts_with("crack-v3: 2500 cr. The blueprint for crack v3.")),
        "{shop:?}"
    );
    assert!(
        shop.iter().any(|line| line.contains("pays 50% more")),
        "{shop:?}"
    );
}

#[test]
fn two_handles_can_duel_in_the_grid() {
    let mut world = port_ostra(0).unwrap();
    let (nyx, _) = world.join("nyx");
    let (zer0, _) = world.join("zer0");
    world.command(nyx, "duel zer0");
    world.command(zer0, "duel nyx");
    assert_eq!(
        world.prompt(nyx).as_deref(),
        Some("[integ 100] nyx@grid-north $ ")
    );
    assert_eq!(
        world.prompt(zer0).as_deref(),
        Some("[integ 100] zer0@grid-south $ ")
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
