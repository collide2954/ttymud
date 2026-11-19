use ttymud_server::port_ostra;

#[test]
fn port_ostra_is_valid() {
    let world = port_ostra();
    assert!(world.is_ok(), "{:?}", world.err());
}

#[test]
fn port_ostra_starts_at_the_relay() {
    let mut world = port_ostra().unwrap();
    let (player, outputs) = world.join("guest");
    assert_eq!(outputs[0].message.plain(), "The Relay");
    assert_eq!(world.prompt(player).as_deref(), Some("guest@relay $ "));
}
