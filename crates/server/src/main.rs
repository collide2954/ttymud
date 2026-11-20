use std::ffi::OsString;
use std::io;
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use ttymud_server::Moderation;

const USAGE: &str = "\
Usage: ttymud-server [OPTION]
       ttymud-server init <DIR>
       ttymud-server run <DIR> [--listen ADDRESS] [--ssh ADDRESS]
       ttymud-server mod <DIR> [add|remove <HANDLE>]
       ttymud-server delete <DIR> <HANDLE>
       ttymud-server city <DIR>
       ttymud-server check <DIR>
       ttymud-server play [NAME]

Commands:
  init <DIR>     Make a data directory with a new server key.
  run <DIR>      Open the city to players. The default address is 0.0.0.0:7331.
                 The city is DIR/city.ini, or Port Ostra if that file is not there.
  mod <DIR>      Show the moderators, or add or remove a moderator by handle.
  delete <DIR> <HANDLE>
                 Delete an account and its data. Stop the server first.
  city <DIR>     Write a copy of Port Ostra to DIR/city.ini, to start your own city.
  check <DIR>    Read the data directory as run does, and show all errors.
  play [NAME]    Walk Port Ostra alone, offline.

Options:
  --ssh ADDRESS  Also let players connect with SSH, for example on 0.0.0.0:7332.
                 The server must have the ssh feature.
  -h, --help     Show this help.
  -V, --version  Show the version.
";

const LISTEN: &str = "0.0.0.0:7331";
#[cfg(not(feature = "ssh"))]
const NO_SSH: &str = "this server has no SSH support. Build it with: cargo build --release -p ttymud-server --features ssh";

#[derive(Debug, PartialEq, Eq)]
enum Cli {
    Help,
    Version,
    Init(PathBuf),
    Run {
        dir: PathBuf,
        listen: String,
        ssh: Option<String>,
    },
    Mod {
        dir: PathBuf,
        action: Moderation,
    },
    Delete {
        dir: PathBuf,
        handle: String,
    },
    City(PathBuf),
    Check(PathBuf),
    Play(Option<String>),
}

fn parse(args: &[OsString]) -> Result<Cli, String> {
    let words: Vec<String> = args
        .iter()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect();
    let words: Vec<&str> = words.iter().map(String::as_str).collect();
    let dir = || PathBuf::from(&args[1]);
    match words.as_slice() {
        [] | ["-h" | "--help"] => Ok(Cli::Help),
        ["-V" | "--version"] => Ok(Cli::Version),
        ["init", _] => Ok(Cli::Init(dir())),
        ["run", _, options @ ..] => run_options(dir(), options),
        ["mod", rest @ ..] if !rest.is_empty() => {
            let action = match &rest[1..] {
                [] => Moderation::List,
                ["add", handle] => Moderation::Add((*handle).to_owned()),
                ["remove", handle] => Moderation::Remove((*handle).to_owned()),
                _ => return Err("mod takes add <HANDLE> or remove <HANDLE>".to_owned()),
            };
            Ok(Cli::Mod { dir: dir(), action })
        }
        ["delete", _, handle] => Ok(Cli::Delete {
            dir: dir(),
            handle: (*handle).to_owned(),
        }),
        ["delete", _] => Err("delete needs a handle".to_owned()),
        ["city", _] => Ok(Cli::City(dir())),
        ["check", _] => Ok(Cli::Check(dir())),
        ["play"] => Ok(Cli::Play(None)),
        ["play", name] => Ok(Cli::Play(Some((*name).to_owned()))),
        [command @ ("init" | "run" | "mod" | "delete" | "city" | "check")] => {
            Err(format!("{command} needs a data directory"))
        }
        [word] => Err(format!("unknown argument '{word}'")),
        _ => Err("too many arguments".to_owned()),
    }
}

fn run_options(dir: PathBuf, options: &[&str]) -> Result<Cli, String> {
    let (mut listen, mut ssh) = (LISTEN.to_owned(), None);
    for pair in options.chunks(2) {
        match pair {
            ["--listen", address] => (*address).clone_into(&mut listen),
            ["--ssh", address] => ssh = Some((*address).to_owned()),
            [option @ ("--listen" | "--ssh")] => return Err(format!("{option} needs an address")),
            [option, ..] => return Err(format!("unknown option '{option}'")),
            [] => {}
        }
    }
    Ok(Cli::Run { dir, listen, ssh })
}

fn main() -> ExitCode {
    let args: Vec<OsString> = std::env::args_os().skip(1).collect();
    let cli = match parse(&args) {
        Ok(cli) => cli,
        Err(message) => {
            eprintln!("ttymud-server: {message}\nTry 'ttymud-server --help'.");
            return ExitCode::from(2);
        }
    };
    let result = match cli {
        Cli::Help => {
            print!("{USAGE}");
            Ok(())
        }
        Cli::Version => {
            println!("ttymud-server {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        Cli::Init(dir) => init(&dir),
        Cli::Run { dir, listen, ssh } => run(&dir, &listen, ssh.as_deref()),
        Cli::Mod { dir, action } => {
            ttymud_server::moderate(&dir, &action).map(|done| println!("{done}"))
        }
        Cli::Delete { dir, handle } => {
            ttymud_server::delete(&dir, &handle).map(|done| println!("{done}"))
        }
        Cli::City(dir) => ttymud_server::write_city(&dir).map(|path| {
            println!("Wrote a copy of Port Ostra to {}.", path.display());
            println!(
                "Change it to make your own city. Then run: ttymud-server check {}",
                dir.display()
            );
        }),
        Cli::Check(dir) => ttymud_server::check(&dir).map(|done| println!("{done}")),
        Cli::Play(name) => ttymud_server::play(
            name.as_deref(),
            io::BufReader::new(io::stdin()),
            io::stdout().lock(),
        ),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("ttymud-server: {error}");
            ExitCode::FAILURE
        }
    }
}

fn init(dir: &Path) -> io::Result<()> {
    let created = ttymud_server::init(dir)?;
    let public = ttymud_server::load_keys(dir)?.public();
    let place = dir.display();
    if created {
        println!("Created a server key in {place}.");
    } else {
        println!("{place} already has a server key.");
    }
    println!("Fingerprint: {}", public.fingerprint());
    println!("Key: {}", public.to_hex());
    Ok(())
}

fn run(dir: &Path, listen: &str, ssh: Option<&str>) -> io::Result<()> {
    #[cfg(not(feature = "ssh"))]
    if ssh.is_some() {
        return Err(io::Error::other(NO_SSH));
    }
    let ttymud_server::Data {
        keys,
        peers,
        world,
        identity,
    } = ttymud_server::load_data(dir)?;
    let store = ttymud_server::Store::start(dir)?;
    let listener = TcpListener::bind(listen)?;
    println!("{} is open on {}.", world.city(), listener.local_addr()?);
    println!("Fingerprint: {}", keys.public().fingerprint());
    println!("Key: {}", keys.public().to_hex());
    println!("Accounts: {}", world.account_count());
    for peer in &peers {
        println!("Link: {} at {}", peer.name, peer.address);
    }
    #[cfg(feature = "ssh")]
    if let Some(address) = ssh {
        let key = ttymud_server::ssh_host_key(dir)?;
        let port = ttymud_server::SshPort::new(TcpListener::bind(address)?, key);
        println!("SSH is open on {}.", port.address()?);
        println!("SSH fingerprint: {}", port.fingerprint());
        return ttymud_server::serve_with_ssh(
            &listener,
            port,
            keys,
            world,
            identity,
            Some(store),
            peers,
        );
    }
    ttymud_server::serve(&listener, keys, world, identity, Some(store), peers)
}

#[cfg(test)]
mod tests {
    use super::{Cli, parse};
    use std::ffi::OsString;
    use ttymud_server::Moderation;

    fn args(list: &[&str]) -> Vec<OsString> {
        list.iter().map(OsString::from).collect()
    }

    #[test]
    fn no_arguments_show_help() {
        assert_eq!(parse(&args(&[])), Ok(Cli::Help));
    }

    #[test]
    fn short_flags_match_long_flags() {
        assert_eq!(parse(&args(&["-h"])), Ok(Cli::Help));
        assert_eq!(parse(&args(&["-V"])), Ok(Cli::Version));
    }

    #[test]
    fn extra_arguments_are_rejected() {
        assert_eq!(
            parse(&args(&["--version", "now"])),
            Err("too many arguments".to_owned())
        );
    }

    #[test]
    fn init_takes_a_data_directory() {
        assert_eq!(
            parse(&args(&["init", "data"])),
            Ok(Cli::Init("data".into()))
        );
    }

    #[test]
    fn init_and_run_need_a_data_directory() {
        assert_eq!(
            parse(&args(&["init"])),
            Err("init needs a data directory".to_owned())
        );
        assert_eq!(
            parse(&args(&["run"])),
            Err("run needs a data directory".to_owned())
        );
    }

    #[test]
    fn run_listens_on_port_7331_by_default() {
        assert_eq!(
            parse(&args(&["run", "data"])),
            Ok(Cli::Run {
                dir: "data".into(),
                listen: "0.0.0.0:7331".to_owned(),
                ssh: None,
            })
        );
    }

    #[test]
    fn run_takes_a_listen_address() {
        assert_eq!(
            parse(&args(&["run", "data", "--listen", "127.0.0.1:4000"])),
            Ok(Cli::Run {
                dir: "data".into(),
                listen: "127.0.0.1:4000".to_owned(),
                ssh: None,
            })
        );
    }

    #[test]
    fn run_takes_an_ssh_address_and_the_options_in_any_order() {
        let both = Cli::Run {
            dir: "data".into(),
            listen: "127.0.0.1:4000".to_owned(),
            ssh: Some("0.0.0.0:7332".to_owned()),
        };
        let line = [
            "run",
            "data",
            "--ssh",
            "0.0.0.0:7332",
            "--listen",
            "127.0.0.1:4000",
        ];
        assert_eq!(parse(&args(&line)), Ok(both));
        assert_eq!(
            parse(&args(&["run", "data", "--ssh"])),
            Err("--ssh needs an address".to_owned())
        );
        assert_eq!(
            parse(&args(&["run", "data", "--port", "22"])),
            Err("unknown option '--port'".to_owned())
        );
    }

    #[test]
    fn mod_shows_adds_or_removes_moderators() {
        let action = |list: &[&str]| match parse(&args(list)) {
            Ok(Cli::Mod { dir, action }) => Ok((dir, action)),
            other => Err(other),
        };
        assert_eq!(
            action(&["mod", "data"]),
            Ok(("data".into(), Moderation::List))
        );
        assert_eq!(
            action(&["mod", "data", "add", "nyx"]),
            Ok(("data".into(), Moderation::Add("nyx".to_owned())))
        );
        assert_eq!(
            action(&["mod", "data", "remove", "nyx"]),
            Ok(("data".into(), Moderation::Remove("nyx".to_owned())))
        );
        assert_eq!(
            parse(&args(&["mod"])),
            Err("mod needs a data directory".to_owned())
        );
        assert_eq!(
            parse(&args(&["mod", "data", "promote", "nyx"])),
            Err("mod takes add <HANDLE> or remove <HANDLE>".to_owned())
        );
    }

    #[test]
    fn delete_takes_a_data_directory_and_a_handle() {
        assert_eq!(
            parse(&args(&["delete", "data", "nyx"])),
            Ok(Cli::Delete {
                dir: "data".into(),
                handle: "nyx".to_owned(),
            })
        );
        assert_eq!(
            parse(&args(&["delete", "data"])),
            Err("delete needs a handle".to_owned())
        );
        assert_eq!(
            parse(&args(&["delete"])),
            Err("delete needs a data directory".to_owned())
        );
    }

    #[test]
    fn city_and_check_take_a_data_directory() {
        assert_eq!(
            parse(&args(&["city", "data"])),
            Ok(Cli::City("data".into()))
        );
        assert_eq!(
            parse(&args(&["check", "data"])),
            Ok(Cli::Check("data".into()))
        );
        assert_eq!(
            parse(&args(&["city"])),
            Err("city needs a data directory".to_owned())
        );
        assert_eq!(
            parse(&args(&["check"])),
            Err("check needs a data directory".to_owned())
        );
    }

    #[test]
    fn play_needs_no_name() {
        assert_eq!(parse(&args(&["play"])), Ok(Cli::Play(None)));
    }

    #[test]
    fn play_takes_a_name() {
        assert_eq!(
            parse(&args(&["play", "nyx"])),
            Ok(Cli::Play(Some("nyx".to_owned())))
        );
    }
}
