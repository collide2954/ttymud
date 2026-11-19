# Architecture

This document gives the structure of the TTYmud code. It tells which crate does what, how the server works, and how the tests work.

## Workspace

TTYmud is a Cargo workspace.

    ttymud/
    ├── Cargo.toml       the workspace, shared lints, and shared versions
    ├── clippy.toml      clippy settings
    ├── rustfmt.toml     rustfmt settings
    ├── crates/
    │   ├── text/        ttymud-text: styles, spans, messages, and the text sanitizer
    │   ├── game/        ttymud-game: rules, world, content files, and the command parser
    │   ├── wire/        ttymud-wire: frames and protocol messages
    │   ├── server/      ttymud-server: the server binary and the default city
    │   └── client/      ttymud: the client binary
    ├── tools/
    │   └── stelint/     stelint: the STE check for the documents
    └── docs/            the documents, in ASD-STE100

Each directory in `crates/` and `tools/` contains one package. The name before the colon is the package name.

These rules control the dependencies between crates:

- `text` has no dependencies. `game` and `wire` use only `text`. These three crates do no input or output. As a result, their tests are fast and always give the same result.
- `server` uses `game` and `wire`.
- `client` uses `wire` and `text`, but not `game`. The client does not know the game rules. As a result, a modified client cannot change the rules.
- `stelint` is a tool for the documents. The game does not use it.

## Binaries

| Binary | User | Use |
| --- | --- | --- |
| `ttymud` | Player | Connect to a server and play. |
| `ttymud-server` | Operator | Run a city. |

    ttymud connect <host>[:<port>]
    ttymud identity
    ttymud recover <host>[:<port>]

    ttymud-server init <data-dir>
    ttymud-server check <data-dir>
    ttymud-server run <data-dir>
    ttymud-server play [name]

The default port is 7331. The `play` command lets one player walk Port Ostra without a network. It is for tests and for a first look at the game.

## Server

The server uses threads from the Rust standard library. It does not use an async runtime.

- **Accept thread.** It accepts TCP connections. For each connection, it starts a session thread. It applies the connection limits.
- **Session threads.** Each session does the Noise handshake and decodes the frames. It sends inputs to the game loop through a channel. It receives outputs from the game loop through a different channel.
- **Game loop thread.** It owns the world state. No other thread can touch the world, so the world has no locks. The loop applies each input when it arrives and runs a game tick every two seconds.
- **Storage thread.** It writes changes to disk. As a result, a slow disk does not stop the game loop.

This design is for a maximum of about 500 players on a small server. Most MUDs have fewer players than this.

## Game core

The `game` crate is a pure state machine. It does not read the clock, and it does not get random numbers from the operating system.

    let mut world = World::new(Content::parse(text)?);
    let (player, outputs) = world.join("ghost");
    let outputs = world.command(player, "ls | grep txt");

Each output has a player and a styled message. A command checks all stages of a pipeline before it runs the first stage. The `prompt` function gives the prompt of a player.

Game time comes in M3 with `world.tick(now)`. The caller will give the time and the random seed. As a result, a test can give a seed and a time, and the test gets the same result each time.

## Storage

The server keeps all data in plain text files in its data directory. Each account, handle, and crew has its own file. The server writes a file in three steps:

1. It writes the data to a temporary file.
2. It flushes the temporary file to the disk.
3. It renames the temporary file to the real name.

As a result, a crash cannot leave a damaged file. To make a backup, copy the data directory.

## Content files

Hubs, corporations, ICE, programs, starter kits, and text templates are content files. They use a small text format with sections and keys. The [content](../content.md) document gives the format. The same parser will also read save files.

The default city, Port Ostra, is in `crates/server/content/`. The server binary contains it, so a server needs no other files. An operator can put content files in the data directory. A file in the data directory replaces the default file with the same name. The `ttymud-server check` command finds errors in content files.

## Wire protocol

All messages go in Noise transport frames. A message starts with a type byte, and the fields of the message follow it. Integers use the LEB128 variable-length format. Strings are UTF-8, with a length prefix.

| From client to server | Contents |
| --- | --- |
| Hello | The protocol version, the client layer, and the terminal size |
| Command | One input line |
| Complete | A part of an input line, for tab completion |

| From server to client | Contents |
| --- | --- |
| Welcome | The server name, the message of the day, and the account state |
| Text | A message kind and a list of styled spans |
| Status | Trace, integrity, heat, cycles, RAM, and the prompt |
| Candidates | The results of a tab completion |
| Close | The reason for the disconnection |

A protocol specification will give the full details.

## Dependencies

TTYmud keeps dependencies to a minimum. Each dependency must have a reason in this table. If you add a dependency, add its row to this table in the same commit.

| Crate | Used by | Reason |
| --- | --- | --- |
| `snow` | server, client | The Noise protocol. TTYmud does not write its own cryptography. |
| `crossterm` | client | Raw mode, keys, and cursor control on Unix and Windows. It also supports the old Windows console. |

TTYmud does not use serde, tokio, clap, or a crate for logs. The standard library and small local code do these tasks.

## Code rules

- Write the test first. Then write the code.
- All crates use the lints of the workspace. Unsafe code is forbidden.
- Clippy uses the pedantic group. The lints that ask for doc comments are off, because TTYmud keeps comments to a minimum.
- Do not use `unwrap` or `expect` outside tests. Use error types.
- Format the code with `cargo fmt`. Fix all warnings from `cargo clippy`.
- Keep comments to a minimum. Clear names are better than comments.
- Use the latest stable Rust. The `rust-version` field in `Cargo.toml` gives the minimum version.

## Tests

- **Unit tests** in each crate.
- **Scenario tests** for the game. Each file in `crates/game/tests/scenarios/` is a transcript of commands and their expected output. The test runs the commands on a test city and compares the outputs with the transcript. A comment line starts with `#`.
- **Golden tests** for the client renderer. There is one test for each layer and each color depth.
- **Round-trip tests** for the wire protocol.
- **Integration tests** that start a server and a client in one process on a local port.
- **The STE check** for all documents.

## Continuous integration

GitHub Actions runs `cargo fmt --check`, `cargo clippy`, and `cargo test` on Linux, macOS, and Windows. It runs for each push to `main` and for each pull request. The workflow uses only the official checkout action, pinned to a commit hash.
