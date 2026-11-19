use std::io::{self, Write};
use std::process::{Command, Output, Stdio};

fn server(args: &[&str]) -> io::Result<Output> {
    Command::new(env!("CARGO_BIN_EXE_ttymud-server"))
        .args(args)
        .output()
}

#[test]
fn version_prints_name_and_version() -> io::Result<()> {
    let output = server(&["--version"])?;
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        concat!("ttymud-server ", env!("CARGO_PKG_VERSION"), "\n")
    );
    Ok(())
}

#[test]
fn help_prints_usage() -> io::Result<()> {
    let output = server(&["--help"])?;
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).starts_with("Usage: ttymud-server "));
    Ok(())
}

#[test]
fn play_walks_the_default_city() -> io::Result<()> {
    let mut child = Command::new(env!("CARGO_BIN_EXE_ttymud-server"))
        .arg("play")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(b"go night-market\nquit\n")?;
    }
    let output = child.wait_with_output()?;
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("guest@night-market $ "));
    Ok(())
}

#[test]
fn unknown_argument_fails_with_a_hint() -> io::Result<()> {
    let output = server(&["--bogus"])?;
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("unknown argument '--bogus'"));
    assert!(stderr.contains("ttymud-server --help"));
    Ok(())
}
