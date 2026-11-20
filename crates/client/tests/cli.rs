#![allow(clippy::unwrap_used)]

use std::io::{self, Write};
use std::net::TcpListener;
use std::process::{Command, Output, Stdio};
use std::thread;

use ttymud_net::Keys;
use ttymud_text::Message;
use ttymud_wire::{ClientMessage, ServerMessage};

fn ttymud(args: &[&str]) -> io::Result<Output> {
    Command::new(env!("CARGO_BIN_EXE_ttymud"))
        .args(args)
        .output()
}

fn ttymud_with_input(args: &[&str], input: &str) -> io::Result<Output> {
    let mut child = Command::new(env!("CARGO_BIN_EXE_ttymud"))
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(input.as_bytes())?;
    }
    child.wait_with_output()
}

fn echo_server() -> (String, String) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap().to_string();
    let keys = Keys::generate().unwrap();
    let key = keys.public().to_hex();
    thread::spawn(move || {
        for mut stream in listener.incoming().flatten() {
            let Ok(session) = ttymud_net::accept(&mut stream, &keys) else {
                continue;
            };
            let (mut receiver, mut sender) = session.split(stream.try_clone().unwrap(), stream);
            let mut send = |message: ServerMessage| {
                let _ = sender.send(&message.encode());
            };
            send(ServerMessage::Welcome {
                city: "Test City".to_owned(),
                name: "anon-test".to_owned(),
            });
            send(ServerMessage::Prompt("$ ".to_owned()));
            while let Ok(bytes) = receiver.receive() {
                let Ok(ClientMessage::Command(line)) = ClientMessage::decode(&bytes) else {
                    break;
                };
                send(ServerMessage::Text(Message::game(format!("echo {line}"))));
                send(ServerMessage::Prompt("$ ".to_owned()));
            }
        }
    });
    (address, key)
}

fn scratch(name: &str) -> String {
    let dir = std::env::temp_dir().join(format!("ttymud-cli-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir.to_string_lossy().into_owned()
}

#[test]
fn version_prints_name_and_version() -> io::Result<()> {
    let output = ttymud(&["--version"])?;
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        concat!("ttymud ", env!("CARGO_PKG_VERSION"), "\n")
    );
    Ok(())
}

#[test]
fn help_prints_usage() -> io::Result<()> {
    let output = ttymud(&["--help"])?;
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).starts_with("Usage: ttymud "));
    Ok(())
}

#[test]
fn unknown_argument_fails_with_a_hint() -> io::Result<()> {
    let output = ttymud(&["--bogus"])?;
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("unknown argument '--bogus'"));
    assert!(stderr.contains("ttymud --help"));
    Ok(())
}

#[test]
fn connect_plays_on_a_server() -> io::Result<()> {
    let (address, _) = echo_server();
    let config = scratch("play");
    let args = ["connect", &address, "--accept-new", "--config", &config];
    let output = ttymud_with_input(&args, "look\nquit\n")?;
    let _ = std::fs::remove_dir_all(&config);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "{stdout}");
    assert!(stdout.contains("Created your identity"), "{stdout}");
    assert!(stdout.contains("is a new server for you"), "{stdout}");
    assert!(
        stdout.contains("Connected to Test City as anon-test."),
        "{stdout}"
    );
    assert!(stdout.contains("echo look"), "{stdout}");
    Ok(())
}

#[test]
fn a_given_key_connects_without_a_question() -> io::Result<()> {
    let (address, key) = echo_server();
    let config = scratch("key");
    let args = ["connect", &address, "--key", &key, "--config", &config];
    let output = ttymud_with_input(&args, "look\nquit\n")?;
    let _ = std::fs::remove_dir_all(&config);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "{stdout}");
    assert!(!stdout.contains("is a new server for you"), "{stdout}");
    assert!(stdout.contains("echo look"), "{stdout}");
    Ok(())
}

#[test]
fn a_wrong_given_key_stops_with_a_warning() -> io::Result<()> {
    let (address, _) = echo_server();
    let config = scratch("wrong-key");
    let other = Keys::generate().unwrap().public().to_hex();
    let args = ["connect", &address, "--key", &other, "--config", &config];
    let output = ttymud_with_input(&args, "")?;
    let _ = std::fs::remove_dir_all(&config);
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("WARNING: the key of") && stderr.contains("is not the key that you gave"),
        "{stderr}"
    );
    Ok(())
}

#[test]
fn a_new_server_needs_a_yes() -> io::Result<()> {
    let (address, _) = echo_server();
    let config = scratch("refuse");
    let output = ttymud_with_input(&["connect", &address, "--config", &config], "no\n")?;
    let _ = std::fs::remove_dir_all(&config);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("Type yes to trust it"));
    assert!(String::from_utf8_lossy(&output.stderr).contains("you did not trust the server"));
    Ok(())
}
