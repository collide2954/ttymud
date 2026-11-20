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
use crate::peers::Peer;
use crate::store::Store;
use crate::{game, longline, session};

#[derive(Clone)]
pub(crate) struct Hub {
    pub(crate) events: mpsc::Sender<Event>,
    pub(crate) limits: Arc<Mutex<Limits>>,
    pub(crate) peers: Arc<Vec<Peer>>,
    pub(crate) city: Arc<str>,
    connections: Arc<AtomicU64>,
}

impl Hub {
    fn start(
        mut world: World,
        identity: Identity,
        store: Option<Store>,
        peers: Vec<Peer>,
    ) -> io::Result<Self> {
        world.set_peers(peers.iter().map(|peer| peer.name.clone()).collect());
        let city = Arc::from(world.city());
        let (events, inbox) = mpsc::channel();
        let saves = store.map(Store::sender);
        thread::Builder::new()
            .name("game".to_owned())
            .spawn(move || game::run(world, identity, &inbox, saves))?;
        Ok(Self {
            events,
            limits: Arc::new(Mutex::new(Limits::new(MAX_CONNECTIONS, MAX_PER_ADDRESS))),
            peers: Arc::new(peers),
            city,
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
    peers: Vec<Peer>,
) -> io::Result<()> {
    let hub = Hub::start(world, identity, store, peers)?;
    let keys = Arc::new(keys);
    longline::start(&hub, &keys)?;
    accept(listener, &keys, &hub);
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
    peers: Vec<Peer>,
) -> io::Result<()> {
    let hub = Hub::start(world, identity, store, peers)?;
    let keys = Arc::new(keys);
    crate::ssh::start(ssh, hub.clone())?;
    longline::start(&hub, &keys)?;
    accept(listener, &keys, &hub);
    Ok(())
}

fn accept(listener: &TcpListener, keys: &Arc<Keys>, hub: &Hub) {
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
        let (keys, hub) = (Arc::clone(keys), hub.clone());
        let spawned = thread::Builder::new()
            .name(format!("session-{connection}"))
            .spawn(move || {
                let _admission = admission;
                session::run(stream, connection, peer.ip(), &keys, &hub);
            });
        if let Err(error) = spawned {
            eprintln!("ttymud-server: connection {connection}: {error}");
        }
    }
}
