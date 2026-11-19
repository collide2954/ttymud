use std::ffi::OsString;
use std::io;
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use ttymud_server::Moderation;

const USAGE: &str = "\
Usage: ttymud-server [OPTION]
       ttymud-server init <DIR>
       ttymud-server run <DIR> [--listen ADDRESS]
       ttymud-server mod <DIR> [add|remove <HANDLE>]
       ttymud-server play [NAME]

Commands:
  init <DIR>     Make a data directory with a new server key.
  run <DIR>      Open the city to players. The default address is 0.0.0.0:7331.
  mod <DIR>      Show the moderators, or add or remove a moderator by handle.
  play [NAME]    Walk Port Ostra alone, offline.

Options:
  -h, --help     Show this help.
  -V, --version  Show the version.
";

const LISTEN: &str = "0.0.0.0:7331";

#[derive(Debug, PartialEq, Eq)]
enum Cli {
    Help,
    Version,
    Init(PathBuf),
    Run { dir: PathBuf, listen: String },
    Mod { dir: PathBuf, action: Moderation },
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
        ["run", _] => Ok(Cli::Run {
            dir: dir(),
            listen: LISTEN.to_owned(),
        }),
        ["run", _, "--listen", address] => Ok(Cli::Run {
            dir: dir(),
            listen: (*address).to_owned(),
        }),
        ["mod", rest @ ..] if !rest.is_empty() => {
            let action = match &rest[1..] {
                [] => Moderation::List,
                ["add", handle] => Moderation::Add((*handle).to_owned()),
                ["remove", handle] => Moderation::Remove((*handle).to_owned()),
                _ => return Err("mod takes add <HANDLE> or remove <HANDLE>".to_owned()),
            };
            Ok(Cli::Mod { dir: dir(), action })
        }
        ["play"] => Ok(Cli::Play(None)),
        ["play", name] => Ok(Cli::Play(Some((*name).to_owned()))),
        [command @ ("init" | "run" | "mod")] => Err(format!("{command} needs a data directory")),
        [word] => Err(format!("unknown argument '{word}'")),
        _ => Err("too many arguments".to_owned()),
    }
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
        Cli::Run { dir, listen } => run(&dir, &listen),
        Cli::Mod { dir, action } => {
            ttymud_server::moderate(&dir, &action).map(|done| println!("{done}"))
        }
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
    let fingerprint = ttymud_server::load_keys(dir)?.public().fingerprint();
    let place = dir.display();
    if created {
        println!("Created a server key in {place}.");
    } else {
        println!("{place} already has a server key.");
    }
    println!("Fingerprint: {fingerprint}");
    Ok(())
}

fn run(dir: &Path, listen: &str) -> io::Result<()> {
    let keys = ttymud_server::load_keys(dir).map_err(|error| missing_key(dir, error))?;
    let world = ttymud_server::load_world(dir)?;
    let identity = ttymud_server::load_identity(dir)?;
    let store = ttymud_server::Store::start(dir)?;
    let listener = TcpListener::bind(listen)?;
    println!("{} is open on {}.", world.city(), listener.local_addr()?);
    println!("Fingerprint: {}", keys.public().fingerprint());
    println!("Accounts: {}", world.account_count());
    ttymud_server::serve(&listener, keys, world, identity, Some(store))
}

fn missing_key(dir: &Path, error: io::Error) -> io::Error {
    if error.kind() != io::ErrorKind::NotFound {
        return error;
    }
    let place = dir.display();
    let message = format!("{place} has no server key. Run: ttymud-server init {place}");
    io::Error::new(error.kind(), message)
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
                listen: "0.0.0.0:7331".to_owned()
            })
        );
    }

    #[test]
    fn run_takes_a_listen_address() {
        assert_eq!(
            parse(&args(&["run", "data", "--listen", "127.0.0.1:4000"])),
            Ok(Cli::Run {
                dir: "data".into(),
                listen: "127.0.0.1:4000".to_owned()
            })
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
