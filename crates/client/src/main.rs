use std::ffi::OsString;
use std::fmt::Write as _;
use std::io::{self, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use ttymud::{Choices, ConnectError, Depth, Ending, Layer, Server, Settings};
use ttymud_net::PublicKey;

const SERVERS_PAGE: &str = "https://collide2954.github.io/ttymud/servers.html";

const USAGE: &str = "\
Usage: ttymud [OPTION]
       ttymud servers
       ttymud connect <ADDRESS|NAME> [--key KEY] [--accept-new] [--config DIR]
                      [--layer LAYER] [--color DEPTH] [--ascii] [--verbose]
                      [--no-mouse]

Commands:
  servers            Show the public list of servers.
  connect <ADDRESS>  Connect to a server. The default port is 7331.
  connect <NAME>     Connect to a server from the public list.

Options:
  --key KEY        Check that the server has this key: the 64 hexadecimal
                   digits that its operator publishes.
  --accept-new     Trust a new server without a question.
  --config DIR     Keep your identity, known servers, and settings in DIR.
  --layer LAYER    Use line, edit, color, or hud. The default is the best
                   layer that the terminal can show.
  --line           The same as --layer line.
  --color DEPTH    Use none, 16, 256, or true colors.
  --ascii          Use only ASCII characters for lines and symbols.
  --verbose        Write changes of state as text, for screen readers.
  --no-mouse       Do not let the mouse wheel scroll the HUD.
  -h, --help       Show this help.
  -V, --version    Show the version.
";

#[derive(Debug, PartialEq, Eq)]
enum Cli {
    Help,
    Version,
    Servers,
    Connect(Options),
}

#[derive(Debug, Default, PartialEq, Eq)]
struct Options {
    address: String,
    key: Option<PublicKey>,
    accept_new: bool,
    config: Option<PathBuf>,
    choices: Choices,
}

fn parse(args: &[OsString]) -> Result<Cli, String> {
    match args {
        [] => Ok(Cli::Help),
        [command, rest @ ..] if command == "connect" => parse_connect(rest),
        [command] if command == "servers" => Ok(Cli::Servers),
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
        let mut value = |flag: &str| {
            let value = rest.next().ok_or(format!("{flag} needs a value"))?;
            Ok::<String, String>(value.to_string_lossy().into_owned())
        };
        let choices = &mut options.choices;
        match arg.to_str() {
            Some("--key") => {
                let key = PublicKey::from_hex(value("--key")?.trim())
                    .ok_or("--key needs the 64 hexadecimal digits of the server key")?;
                options.key = Some(key);
            }
            Some("--accept-new") => options.accept_new = true,
            Some("--line") => choices.layer = Some(Layer::Line),
            Some("--layer") => choices.layer = Some(value("--layer")?.parse()?),
            Some("--color") => choices.color = Some(value("--color")?.parse()?),
            Some("--ascii") => choices.ascii = Some(true),
            Some("--verbose") => choices.verbose = Some(true),
            Some("--no-mouse") => choices.mouse = Some(false),
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
        Cli::Servers => print!("{}", listing(&ttymud::servers())),
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
    let chosen = options.choices.or(ttymud::read_settings(&dir)?);
    let settings = chosen.settle(ttymud::detect_terminal());
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
    let servers = ttymud::servers();
    let (address, key) = target(options, &servers);
    let link = ttymud::connect(address, &identity.keys, &known, key, |address, key| {
        trust(address, key, options.accept_new)
    })
    .map_err(|error| explain(&error, &known))?;
    let ending = match settings.layer {
        Layer::Line => {
            let input = BufReader::new(io::stdin());
            ttymud::line_mode(link, input, io::stdout(), settings.verbose)
        }
        Layer::Edit => {
            let plain = Settings {
                color: Depth::None,
                ..settings
            };
            ttymud::edit_mode(link, plain)
        }
        Layer::Color => ttymud::edit_mode(link, settings),
        Layer::Hud => ttymud::hud_mode(link, settings),
    };
    ending.map_err(|error| error.to_string())
}

fn target<'a>(options: &'a Options, servers: &'a [Server]) -> (&'a str, Option<PublicKey>) {
    match servers.iter().find(|server| server.name == options.address) {
        Some(server) => (&server.address, options.key.or(Some(server.key))),
        None => (&options.address, options.key),
    }
}

fn listing(servers: &[Server]) -> String {
    let mut text = String::new();
    if servers.is_empty() {
        text.push_str("This version of ttymud knows no public servers.\n");
        text.push_str("To connect to a server by its address, type: ttymud connect ADDRESS\n");
    } else {
        let width = servers.iter().map(|server| server.name.len()).max();
        let width = width.unwrap_or_default();
        for server in servers {
            let (name, city, address) = (&server.name, &server.city, &server.address);
            let _ = writeln!(text, "{name:width$}  {city} at {address}");
            let _ = writeln!(text, "{:width$}  {}", "", server.description);
        }
        text.push_str("\nTo connect to a server, type: ttymud connect NAME\n");
    }
    let _ = writeln!(text, "To add your server to the list, read {SERVERS_PAGE}");
    text
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
    let (ConnectError::KeyChanged {
        address,
        expected,
        found,
    }
    | ConnectError::WrongKey {
        address,
        expected,
        found,
    }) = error
    else {
        return error.to_string();
    };
    let (expected, found) = (expected.fingerprint(), found.fingerprint());
    let advice = match error {
        ConnectError::WrongKey { .. } => {
            "Check the key with the operator of the server.".to_owned()
        }
        _ => format!(
            "If the operator changed the key, connect with --key and the new key, or remove the line for {address} from {}.",
            known.display()
        ),
    };
    format!(
        "WARNING: {error}.\n\
         Expected fingerprint: {expected}\n\
         Found fingerprint:    {found}\n\
         Someone can be between you and the server. Do not continue if you are not sure.\n\
         {advice}"
    )
}

#[cfg(test)]
mod tests {
    use super::{Cli, Options, listing, parse, target};
    use std::ffi::OsString;
    use ttymud::{Choices, Depth, Layer, Server};
    use ttymud_net::Keys;

    fn args(list: &[&str]) -> Vec<OsString> {
        list.iter().map(OsString::from).collect()
    }

    fn ostra() -> Server {
        Server {
            name: "ostra".to_owned(),
            city: "Port Ostra".to_owned(),
            address: "ostra.example.net:7331".to_owned(),
            key: Keys::generate().unwrap().public(),
            description: "The first city.".to_owned(),
        }
    }

    #[test]
    fn servers_takes_no_arguments() {
        assert_eq!(parse(&args(&["servers"])), Ok(Cli::Servers));
        assert_eq!(
            parse(&args(&["servers", "all"])),
            Err("too many arguments".to_owned())
        );
    }

    #[test]
    fn a_listed_name_gives_the_address_and_the_key_of_the_server() {
        let list = [ostra()];
        let given = Keys::generate().unwrap().public();
        let options = |address: &str, key| Options {
            address: address.to_owned(),
            key,
            ..Options::default()
        };
        let address = "ostra.example.net:7331";
        assert_eq!(
            target(&options("ostra", None), &list),
            (address, Some(list[0].key))
        );
        assert_eq!(
            target(&options("ostra", Some(given)), &list),
            (address, Some(given))
        );
        assert_eq!(
            target(&options("ostra:7331", None), &list),
            ("ostra:7331", None)
        );
    }

    #[test]
    fn the_listing_shows_each_server_and_how_to_connect() {
        let dock = Server {
            name: "dock-7".to_owned(),
            ..ostra()
        };
        assert_eq!(
            listing(&[dock, ostra()]),
            "dock-7  Port Ostra at ostra.example.net:7331\n        The first city.\n\
             ostra   Port Ostra at ostra.example.net:7331\n        The first city.\n\n\
             To connect to a server, type: ttymud connect NAME\n\
             To add your server to the list, read https://collide2954.github.io/ttymud/servers.html\n"
        );
    }

    #[test]
    fn an_empty_listing_tells_how_to_connect_and_how_to_add_a_server() {
        assert_eq!(
            listing(&[]),
            "This version of ttymud knows no public servers.\n\
             To connect to a server by its address, type: ttymud connect ADDRESS\n\
             To add your server to the list, read https://collide2954.github.io/ttymud/servers.html\n"
        );
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
                ..Options::default()
            }))
        );
    }

    #[test]
    fn connect_takes_the_full_key_of_the_server() {
        let key = ttymud_net::Keys::generate().unwrap().public();
        assert_eq!(
            parse(&args(&["connect", "a.example", "--key", &key.to_hex()])),
            Ok(Cli::Connect(Options {
                address: "a.example".to_owned(),
                key: Some(key),
                ..Options::default()
            }))
        );
        assert_eq!(
            parse(&args(&[
                "connect",
                "a.example",
                "--key",
                &key.fingerprint()
            ])),
            Err("--key needs the 64 hexadecimal digits of the server key".to_owned())
        );
    }

    #[test]
    fn connect_can_use_line_mode_in_a_terminal() {
        let line = Choices {
            layer: Some(Layer::Line),
            ..Choices::default()
        };
        for flags in [&["--line"][..], &["--layer", "line"]] {
            assert_eq!(
                parse(&args(&[&["connect", "a.example"][..], flags].concat())),
                Ok(Cli::Connect(Options {
                    address: "a.example".to_owned(),
                    choices: line,
                    ..Options::default()
                }))
            );
        }
    }

    #[test]
    fn connect_takes_the_display_choices() {
        let flags = [
            "connect",
            "a.example",
            "--layer",
            "color",
            "--color",
            "256",
            "--ascii",
            "--verbose",
            "--no-mouse",
        ];
        assert_eq!(
            parse(&args(&flags)),
            Ok(Cli::Connect(Options {
                address: "a.example".to_owned(),
                choices: Choices {
                    layer: Some(Layer::Color),
                    color: Some(Depth::Indexed),
                    ascii: Some(true),
                    verbose: Some(true),
                    mouse: Some(false),
                },
                ..Options::default()
            }))
        );
        assert_eq!(
            parse(&args(&["connect", "a.example", "--layer", "3d"])),
            Err("unknown layer: 3d (use line, edit, color, or hud)".to_owned())
        );
        assert_eq!(
            parse(&args(&["connect", "a.example", "--color"])),
            Err("--color needs a value".to_owned())
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
