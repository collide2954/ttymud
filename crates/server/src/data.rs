use std::fs;
use std::io;
use std::path::Path;

use ttymud_game::World;
use ttymud_net::Keys;

use crate::city::{CITY, load_world};
use crate::identity::{Identity, load_identity};
use crate::peers::{Peer, load_peers};

const KEY_FILE: &str = "server.key";

pub struct Data {
    pub keys: Keys,
    pub peers: Vec<Peer>,
    pub world: World,
    pub identity: Identity,
}

pub fn load_data(dir: &Path) -> io::Result<Data> {
    let keys = load_keys(dir).map_err(|error| missing_key(dir, error));
    let peers = keys
        .as_ref()
        .ok()
        .map(|keys| load_peers(dir, keys.public()));
    let (world, identity) = (load_world(dir), load_identity(dir));
    match (keys, peers, world, identity) {
        (Ok(keys), Some(Ok(peers)), Ok(world), Ok(identity)) => Ok(Data {
            keys,
            peers,
            world,
            identity,
        }),
        (keys, peers, world, identity) => {
            let errors = [
                keys.err(),
                peers.and_then(Result::err),
                world.err(),
                identity.err(),
            ];
            let lines: Vec<String> = errors.iter().flatten().map(ToString::to_string).collect();
            Err(io::Error::other(lines.join("\n")))
        }
    }
}

pub fn check(dir: &Path) -> io::Result<String> {
    let data = load_data(dir)?;
    let source = if dir.join(CITY).is_file() {
        format!("from {CITY}")
    } else {
        "built in".to_owned()
    };
    let (place, city) = (dir.display(), data.world.city());
    let mut lines = vec![
        format!("{place} is ready. City: {city}, {source}."),
        format!("Accounts: {}", data.world.account_count()),
    ];
    for peer in &data.peers {
        lines.push(format!("Link: {} at {}", peer.name, peer.address));
    }
    Ok(lines.join("\n"))
}

fn missing_key(dir: &Path, error: io::Error) -> io::Error {
    if error.kind() != io::ErrorKind::NotFound {
        return error;
    }
    let place = dir.display();
    let message = format!("{place} has no server key. Run: ttymud-server init {place}");
    io::Error::new(error.kind(), message)
}

pub fn init(dir: &Path) -> io::Result<bool> {
    fs::create_dir_all(dir)?;
    let path = dir.join(KEY_FILE);
    if path.exists() {
        return Ok(false);
    }
    Keys::generate()?.save_new(&path)?;
    Ok(true)
}

pub fn load_keys(dir: &Path) -> io::Result<Keys> {
    Ok(Keys::load(&dir.join(KEY_FILE))?)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io::ErrorKind;

    use ttymud_net::Keys;

    use super::{check, init, load_keys};
    use crate::testing::{LAB, scratch};

    #[test]
    fn check_shows_the_city_the_accounts_and_the_links_of_a_ready_directory() {
        let dir = scratch("check-ready");
        init(&dir).unwrap();
        let place = dir.display().to_string();
        assert_eq!(
            check(&dir).unwrap(),
            format!("{place} is ready. City: Port Ostra, built in.\nAccounts: 0")
        );
        let key = Keys::generate().unwrap().public().to_hex();
        let link = format!("[link night-owl]\naddress = owl.example.net:7331\nkey = {key}\n");
        fs::write(dir.join("links.ini"), link).unwrap();
        fs::write(dir.join("city.ini"), LAB).unwrap();
        assert_eq!(
            check(&dir).unwrap(),
            format!(
                "{place} is ready. City: Lab, from city.ini.\nAccounts: 0\n\
                 Link: night-owl at owl.example.net:7331"
            )
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn check_shows_all_errors_of_a_data_directory() {
        let dir = scratch("check-errors");
        fs::create_dir_all(&dir).unwrap();
        let city = "[city lab]\nname = Lab\nstart = nowhere\n";
        fs::write(dir.join("city.ini"), city).unwrap();
        fs::write(dir.join("keys.ini"), "[colour x]\n").unwrap();
        let place = dir.display().to_string();
        let error = check(&dir).unwrap_err().to_string();
        assert_eq!(
            error.lines().collect::<Vec<_>>(),
            [
                format!("{place} has no server key. Run: ttymud-server init {place}").as_str(),
                "city.ini: line 3: start is an unknown host: nowhere",
                "keys.ini: line 1: unknown section type: colour",
            ]
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn init_makes_a_directory_with_a_key() {
        let dir = scratch("init");
        assert!(init(&dir).unwrap());
        assert!(dir.join("server.key").is_file());
        assert!(load_keys(&dir).is_ok());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn init_keeps_an_existing_key() {
        let dir = scratch("again");
        init(&dir).unwrap();
        let first = load_keys(&dir).unwrap().public();
        assert!(!init(&dir).unwrap());
        assert_eq!(load_keys(&dir).unwrap().public(), first);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_directory_without_init_has_no_key() {
        let dir = scratch("data-empty");
        fs::create_dir_all(&dir).unwrap();
        let error = load_keys(&dir).unwrap_err();
        assert_eq!(error.kind(), ErrorKind::NotFound);
        let _ = fs::remove_dir_all(&dir);
    }
}
