use ttymud_game::{Content, World};

const PORT_OSTRA: &str = include_str!("../content/port-ostra.ini");

#[test]
fn port_ostra_is_valid() {
    let parsed = Content::parse(PORT_OSTRA);
    assert!(parsed.is_ok(), "{:?}", parsed.err());
}

#[test]
fn port_ostra_starts_at_the_relay() {
    let mut world = World::new(Content::parse(PORT_OSTRA).unwrap());
    let (player, outputs) = world.join("guest");
    assert_eq!(outputs[0].message.plain(), "The Relay");
    assert_eq!(world.prompt(player).as_deref(), Some("guest@relay $ "));
}
