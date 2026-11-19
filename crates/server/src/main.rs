use std::ffi::OsString;
use std::process::ExitCode;

const USAGE: &str = "\
Usage: ttymud-server [OPTION]

Options:
  -h, --help     Show this help.
  -V, --version  Show the version.
";

#[derive(Debug, PartialEq, Eq)]
enum Cli {
    Help,
    Version,
}

fn parse(args: &[OsString]) -> Result<Cli, String> {
    match args {
        [] => Ok(Cli::Help),
        [flag] => match flag.to_str() {
            Some("-h" | "--help") => Ok(Cli::Help),
            Some("-V" | "--version") => Ok(Cli::Version),
            _ => Err(format!("unknown argument '{}'", flag.to_string_lossy())),
        },
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
}
