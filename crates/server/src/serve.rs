use std::io;
use std::net::TcpListener;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::thread;

use ttymud_game::World;
use ttymud_net::Keys;

use crate::game::Event;
use crate::identity::Identity;
use crate::limits::{Admission, Limits, MAX_CONNECTIONS, MAX_PER_ADDRESS};
use crate::store::Store;
use crate::{game, session};

#[derive(Clone)]
pub(crate) struct Hub {
    pub(crate) events: mpsc::Sender<Event>,
    pub(crate) limits: Arc<Mutex<Limits>>,
    connections: Arc<AtomicU64>,
}

impl Hub {
    fn start(world: World, identity: Identity, store: Option<Store>) -> io::Result<Self> {
        let (events, inbox) = mpsc::channel();
        let saves = store.map(Store::sender);
        thread::Builder::new()
            .name("game".to_owned())
            .spawn(move || game::run(world, identity, &inbox, saves))?;
        Ok(Self {
            events,
            limits: Arc::new(Mutex::new(Limits::new(MAX_CONNECTIONS, MAX_PER_ADDRESS))),
            connections: Arc::new(AtomicU64::new(1)),
        })
    }

    pub(crate) fn connection(&self) -> u64 {
        self.connections.fetch_add(1, Ordering::Relaxed)
    }
}

pub fn serve(
    listener: &TcpListener,
    keys: Keys,
    world: World,
    identity: Identity,
    store: Option<Store>,
) -> io::Result<()> {
    let hub = Hub::start(world, identity, store)?;
    accept(listener, keys, &hub);
    Ok(())
}

#[cfg(feature = "ssh")]
pub fn serve_with_ssh(
    listener: &TcpListener,
    ssh: crate::ssh::SshPort,
    keys: Keys,
    world: World,
    identity: Identity,
    store: Option<Store>,
) -> io::Result<()> {
    let hub = Hub::start(world, identity, store)?;
    crate::ssh::start(ssh, hub.clone())?;
    accept(listener, keys, &hub);
    Ok(())
}

fn accept(listener: &TcpListener, keys: Keys, hub: &Hub) {
    let keys = Arc::new(keys);
    for stream in listener.incoming() {
        let Ok(stream) = stream else {
            continue;
        };
        let Ok(peer) = stream.peer_addr() else {
            continue;
        };
        let Some(admission) = Admission::new(&hub.limits, peer.ip()) else {
            continue;
        };
        let connection = hub.connection();
        let (keys, events) = (Arc::clone(&keys), hub.events.clone());
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
}
