use std::ffi::OsString;
use std::io;
use std::process::ExitCode;

const USAGE: &str = "\
Usage: ttymud-server [OPTION]
       ttymud-server play [NAME]

Commands:
  play [NAME]    Walk Port Ostra alone, offline.

Options:
  -h, --help     Show this help.
  -V, --version  Show the version.
";

const GUEST: &str = "guest";

#[derive(Debug, PartialEq, Eq)]
enum Cli {
    Help,
    Version,
    Play(String),
}

fn parse(args: &[OsString]) -> Result<Cli, String> {
    match args {
        [] => Ok(Cli::Help),
        [word] => match word.to_str() {
            Some("-h" | "--help") => Ok(Cli::Help),
            Some("-V" | "--version") => Ok(Cli::Version),
            Some("play") => Ok(Cli::Play(GUEST.to_owned())),
            _ => Err(format!("unknown argument '{}'", word.to_string_lossy())),
        },
        [command, name] if command == "play" => Ok(Cli::Play(name.to_string_lossy().into_owned())),
        _ => Err("too many arguments".to_owned()),
    }
}

fn main() -> ExitCode {
    let args: Vec<OsString> = std::env::args_os().skip(1).collect();
    match parse(&args) {
        Ok(Cli::Help) => {
            print!("{USAGE}");
            ExitCode::SUCCESS
        }
        Ok(Cli::Version) => {
            println!("ttymud-server {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        Ok(Cli::Play(name)) => {
            match ttymud_server::play(&name, io::stdin().lock(), io::stdout().lock()) {
                Ok(()) => ExitCode::SUCCESS,
                Err(error) => {
                    eprintln!("ttymud-server: {error}");
                    ExitCode::FAILURE
                }
            }
        }
        Err(message) => {
            eprintln!("ttymud-server: {message}\nTry 'ttymud-server --help'.");
            ExitCode::from(2)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Cli, parse};
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
    fn play_has_a_default_name() {
        assert_eq!(parse(&args(&["play"])), Ok(Cli::Play("guest".to_owned())));
    }

    #[test]
    fn play_takes_a_name() {
        assert_eq!(
            parse(&args(&["play", "nyx"])),
            Ok(Cli::Play("nyx".to_owned()))
        );
    }
}
