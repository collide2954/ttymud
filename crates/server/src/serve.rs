use std::io;
use std::net::TcpListener;
use std::sync::{Arc, mpsc};
use std::thread;

use ttymud_game::World;
use ttymud_net::Keys;

use crate::{game, session};

pub fn serve(listener: &TcpListener, keys: Keys, world: World) -> io::Result<()> {
    let (events, inbox) = mpsc::channel();
    thread::Builder::new()
        .name("game".to_owned())
        .spawn(move || game::run(world, &inbox))?;
    let keys = Arc::new(keys);
    for (connection, stream) in (1_u64..).zip(listener.incoming()) {
        let Ok(stream) = stream else {
            continue;
        };
        let (keys, events) = (Arc::clone(&keys), events.clone());
        let spawned = thread::Builder::new()
            .name(format!("session-{connection}"))
            .spawn(move || session::run(stream, connection, &keys, &events));
        if let Err(error) = spawned {
            eprintln!("ttymud-server: connection {connection}: {error}");
        }
    }
    Ok(())
}
