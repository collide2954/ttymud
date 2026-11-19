use std::ffi::OsString;
use std::io::{self, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use ttymud::{ConnectError, Ending};
use ttymud_net::PublicKey;

const USAGE: &str = "\
Usage: ttymud [OPTION]
       ttymud connect <ADDRESS> [--accept-new] [--config DIR]

Commands:
  connect <ADDRESS>  Connect to a server. The default port is 7331.

Options:
  --accept-new   Trust a new server without a question.
  --config DIR   Keep your identity and known servers in DIR.
  -h, --help     Show this help.
  -V, --version  Show the version.
";

#[derive(Debug, PartialEq, Eq)]
enum Cli {
    Help,
    Version,
    Connect(Options),
}

#[derive(Debug, Default, PartialEq, Eq)]
struct Options {
    address: String,
    accept_new: bool,
    config: Option<PathBuf>,
}

fn parse(args: &[OsString]) -> Result<Cli, String> {
    match args {
        [] => Ok(Cli::Help),
        [command, rest @ ..] if command == "connect" => parse_connect(rest),
        [flag] => match flag.to_str() {
            Some("-h" | "--help") => Ok(Cli::Help),
            Some("-V" | "--version") => Ok(Cli::Version),
            _ => Err(format!("unknown argument '{}'", flag.to_string_lossy())),
        },
        _ => Err("too many arguments".to_owned()),
    }
}

fn parse_connect(args: &[OsString]) -> Result<Cli, String> {
    let mut options = Options::default();
    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        match arg.to_str() {
            Some("--accept-new") => options.accept_new = true,
            Some("--config") => {
                let dir = rest.next().ok_or("--config needs a directory")?;
                options.config = Some(PathBuf::from(dir));
            }
            Some(flag) if flag.starts_with("--") => {
                return Err(format!("unknown option '{flag}'"));
            }
            _ if options.address.is_empty() => {
                options.address = arg.to_string_lossy().into_owned();
            }
            _ => return Err("too many arguments".to_owned()),
        }
    }
    if options.address.is_empty() {
        return Err("connect needs an address".to_owned());
    }
    Ok(Cli::Connect(options))
}

fn main() -> ExitCode {
    let args: Vec<OsString> = std::env::args_os().skip(1).collect();
    let cli = match parse(&args) {
        Ok(cli) => cli,
        Err(message) => {
            eprintln!("ttymud: {message}\nTry 'ttymud --help'.");
            return ExitCode::from(2);
        }
    };
    match cli {
        Cli::Help => print!("{USAGE}"),
        Cli::Version => println!("ttymud {}", env!("CARGO_PKG_VERSION")),
        Cli::Connect(options) => match connect(&options) {
            Ok(Ending::Quit) => {}
            Ok(Ending::Closed) => println!("The connection to the server closed."),
            Err(message) => {
                eprintln!("ttymud: {message}");
                return ExitCode::FAILURE;
            }
        },
    }
    ExitCode::SUCCESS
}

fn connect(options: &Options) -> Result<Ending, String> {
    let dir = options
        .config
        .clone()
        .or_else(ttymud::config_dir)
        .ok_or("cannot find a configuration directory, so use --config DIR")?;
    let place = dir.display();
    let identity = ttymud::identity(&dir)
        .map_err(|error| format!("cannot load your identity from {place}: {error}"))?;
    if identity.created {
        println!("Created your identity in {place}.");
        println!(
            "Your fingerprint is {}.",
            identity.keys.public().fingerprint()
        );
    }
    let known = dir.join("known_servers");
    let link = ttymud::connect(&options.address, &identity.keys, &known, |address, key| {
        trust(address, key, options.accept_new)
    })
    .map_err(|error| explain(&error, &known))?;
    ttymud::line_mode(link, BufReader::new(io::stdin()), io::stdout())
        .map_err(|error| error.to_string())
}

fn trust(address: &str, key: PublicKey, accept_new: bool) -> io::Result<bool> {
    let fingerprint = key.fingerprint();
    println!("{address} is a new server for you. Its fingerprint is {fingerprint}.");
    if accept_new {
        println!("ttymud trusts it because you gave --accept-new.");
        return Ok(true);
    }
    print!("Type yes to trust it: ");
    io::stdout().flush()?;
    let mut answer = String::new();
    io::stdin().read_line(&mut answer)?;
    Ok(answer.trim().eq_ignore_ascii_case("yes"))
}

fn explain(error: &ConnectError, known: &Path) -> String {
    let ConnectError::KeyChanged {
        address,
        expected,
        found,
    } = error
    else {
        return error.to_string();
    };
    let (expected, found, file) = (expected.fingerprint(), found.fingerprint(), known.display());
    format!(
        "WARNING: the key of {address} changed.\n\
         Expected fingerprint: {expected}\n\
         Found fingerprint:    {found}\n\
         Someone can be between you and the server. Do not continue if you are not sure.\n\
         If the operator changed the key, remove the line for {address} from {file}."
    )
}

#[cfg(test)]
mod tests {
    use super::{Cli, Options, parse};
    use std::ffi::OsString;

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
    fn connect_takes_an_address() {
        assert_eq!(
            parse(&args(&["connect", "ostra.example.net"])),
            Ok(Cli::Connect(Options {
                address: "ostra.example.net".to_owned(),
                ..Options::default()
            }))
        );
    }

    #[test]
    fn connect_takes_options_in_any_order() {
        assert_eq!(
            parse(&args(&[
                "connect",
                "--accept-new",
                "a.example",
                "--config",
                "dir"
            ])),
            Ok(Cli::Connect(Options {
                address: "a.example".to_owned(),
                accept_new: true,
                config: Some("dir".into()),
            }))
        );
    }

    #[test]
    fn connect_needs_an_address() {
        assert_eq!(
            parse(&args(&["connect"])),
            Err("connect needs an address".to_owned())
        );
    }

    #[test]
    fn bad_connect_options_fail() {
        assert_eq!(
            parse(&args(&["connect", "a.example", "--bogus"])),
            Err("unknown option '--bogus'".to_owned())
        );
        assert_eq!(
            parse(&args(&["connect", "a.example", "--config"])),
            Err("--config needs a directory".to_owned())
        );
        assert_eq!(
            parse(&args(&["connect", "a.example", "b.example"])),
            Err("too many arguments".to_owned())
        );
    }
}
