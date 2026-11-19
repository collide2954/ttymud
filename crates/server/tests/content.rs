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
