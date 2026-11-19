use std::io;
use std::net::TcpListener;
use std::sync::{Arc, Mutex, mpsc};
use std::thread;

use ttymud_game::World;
use ttymud_net::Keys;

use crate::limits::{Admission, Limits, MAX_CONNECTIONS, MAX_PER_ADDRESS};
use crate::store::Store;
use crate::{game, session};

pub fn serve(
    listener: &TcpListener,
    keys: Keys,
    world: World,
    store: Option<Store>,
) -> io::Result<()> {
    let (events, inbox) = mpsc::channel();
    let saves = store.map(Store::sender);
    thread::Builder::new()
        .name("game".to_owned())
        .spawn(move || game::run(world, &inbox, saves))?;
    let keys = Arc::new(keys);
    let limits = Arc::new(Mutex::new(Limits::new(MAX_CONNECTIONS, MAX_PER_ADDRESS)));
    for (connection, stream) in (1_u64..).zip(listener.incoming()) {
        let Ok(stream) = stream else {
            continue;
        };
        let Ok(peer) = stream.peer_addr() else {
            continue;
        };
        let Some(admission) = Admission::new(&limits, peer.ip()) else {
            continue;
        };
        let (keys, events) = (Arc::clone(&keys), events.clone());
        let spawned = thread::Builder::new()
            .name(format!("session-{connection}"))
            .spawn(move || {
                let _admission = admission;
                session::run(stream, connection, peer.ip(), &keys, &events);
            });
        if let Err(error) = spawned {
            eprintln!("ttymud-server: connection {connection}: {error}");
        }
    }
    Ok(())
}
