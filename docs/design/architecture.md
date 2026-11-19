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
    │   ├── wire/        ttymud-wire: protocol messages and their encoding
    │   ├── net/         ttymud-net: keys, Noise sessions, and frames
    │   ├── server/      ttymud-server: the server binary and the default city
    │   └── client/      ttymud: the client binary
    ├── tools/
    │   └── stelint/     stelint: the STE check for the documents
    └── docs/            the documents, in ASD-STE100

Each directory in `crates/` and `tools/` contains one package. The name before the colon is the package name.

These rules control the dependencies between crates:

- `text` has no dependencies. `game` and `wire` use only `text`. These three crates do no input or output. As a result, their tests are fast and always give the same result.
- `net` uses only `snow`. It makes keys, does the Noise handshake, and encrypts the frames.
- `server` uses `game`, `wire`, `net`, and `text`.
- `client` uses `wire`, `net`, and `text`, but not `game`. The client does not know the game rules. As a result, a modified client cannot change the rules.
- `stelint` is a tool for the documents. The game does not use it.

## Binaries

| Binary | User | Use |
| --- | --- | --- |
| `ttymud` | Player | Connect to a server and play. |
| `ttymud-server` | Operator | Run a city. |

    ttymud connect <host>[:<port>] [--accept-new] [--config <dir>] [--line]

    ttymud-server init <data-dir>
    ttymud-server run <data-dir> [--listen <address>]
    ttymud-server play [name]

The default port is 7331. The `play` command lets one player walk Port Ostra without a network. It is for tests and for a first look at the game. It uses real time and asks for a handle, as the server does. It saves nothing. The [player guide](../player.md) and the [operator guide](../operator.md) tell how to use the commands.

These commands come later: `ttymud identity`, `ttymud recover`, and `ttymud-server check`.

## Server

The server uses threads from the Rust standard library. It does not use an async runtime.

- **Accept thread.** It accepts TCP connections and applies the connection limits. For each connection, it starts a session thread.
- **Session threads.** Each session does the Noise handshake, decodes the messages, and applies the rate limits. It sends inputs to the game loop through a channel.
- **Writer threads.** Each session has a writer thread. It receives outputs from the game loop through a different channel, and it encrypts and writes them.
- **Game loop thread.** It owns the world state. No other thread can touch the world, so the world has no locks. The loop applies each input when it arrives.
- **Storage thread.** It writes the save files to the data directory. As a result, a slow disk does not stop the game loop.

The game loop also runs a tick every two seconds. If the loop is late, it does not run the ticks that it missed. After the text of a tick, the loop sends a Refresh message with the new prompt. Each player who gets text from a different player also gets a Refresh message.

After each input and each tick, the game loop gets the changed save texts from the world. It sends them to the storage thread. When the server starts, it reads all save files before it opens the port.

If a key connects again, the new connection replaces the old connection. The old connection gets a Close message with the reason. After a Close message, the writer thread stops all writes and waits for the client to close the connection. As a result, the reset of a connection cannot delete the Close message.

The server accepts a maximum of 256 connections. This design is for a maximum of about 500 players on a small server. Most MUDs have fewer players than this.

## Client

The client has three threads:

- **Input thread.** It reads lines in line mode, or keys in edit mode.
- **Network thread.** It receives and decodes the messages from the server.
- **Main thread.** It receives the events of the two other threads through one channel. It sends the commands to the server, and it writes to the terminal.

The main thread gives each event to a view. The line view writes lines. If text comes after an open prompt, the line view starts the text on a new line. The screen view has a line editor, and it keeps the input line below the text from the server. The client uses the screen view when the input and the output are terminals.

When the player quits, the client waits until each command has its prompt, for a maximum of 5 seconds. As a result, the player sees the answers to the last commands.

## Game core

The `game` crate is a pure state machine. It does not read the clock, and it does not get random numbers from the operating system.

    let mut world = World::new(Content::parse(text)?, seed);
    let (player, outputs) = world.connect(account);
    let outputs = world.command(player, "nyx");
    let outputs = world.command(player, "ghost");
    world.set_time(now);
    let outputs = world.tick();
    let saves = world.saves();

Each output has a player and a styled message. A command checks all stages of a pipeline before it runs the first stage. The `prompt` function gives the prompt of a player.

The `connect` function starts the session of an account. For a new account, the first two commands give the name of the handle and the starter kit. Tests can use `join`, which makes a handle that is ready at once and knows all programs.

The world does not read the clock. Before each input and each tick, the caller gives the time in seconds with `set_time`. Heat cools with this time, and the burn list shows its dates.

The `saves` function gives the save texts that changed since the last call, each with its path. When the server starts, the `restore` function reads each save text back.

The caller calls `tick` every two seconds. On each tick, each rig gets a cycle, the ICE acts, and contracts give hints. The ICE selects its actions with a small random number generator from the seed. As a result, a test with the same seed gets the same result each time.

The world also keeps a world seed. A new world saves it in `world.ini`, and `restore` reads it back. The job board and the target generator use only the world seed, the day, and the number of the job. As a result, they give the same targets after a restart. The server gives `World::new` a seed from the clock, so the ICE acts in a different order after each start.

The target generator is a pure function in the `target` module. It takes a corp, the host templates, and a seed, and it gives the hosts, routes, ICE, files, and credentials of one network. The world makes the network only when a player accepts the job.

## Storage

The server keeps all data in plain text files in its data directory. Each account has its own file in `accounts/`, with its handle. The burn list is in `burns.ini`, and the world seed is in `world.ini`. Later, each crew will have its own file. An account file keeps a job from the board as its fixer, its day, and its number, and the server makes the network again. The storage thread writes a file in three steps:

1. It writes the data to a temporary file.
2. It flushes the temporary file to the disk.
3. It renames the temporary file to the real name.

As a result, a crash cannot leave a damaged file. To make a backup, copy the data directory.

## Content files

Hubs, corporations, host templates, fixers, shops, and ICE are content files. The programs and the starter kits are in the code. They use a small text format with sections and keys. The [content](../content.md) document gives the format. The save files use the same format, and the same parser reads them.

The default city, Port Ostra, is in `crates/server/content/`. The server binary contains it, so a server needs no other files. Later, an operator will put content files in the data directory. A file in the data directory will replace the default file with the same name. The `ttymud-server check` command will find errors in content files.

## Wire protocol

All messages go in Noise transport frames. A message starts with a type byte, and the fields of the message follow it. Integers use the LEB128 variable-length format. Strings are UTF-8, with a length prefix.

| From client to server | Contents |
| --- | --- |
| Command | One input line |
| Complete | The input before the cursor, for tab completion |

| From server to client | Contents |
| --- | --- |
| Welcome | The name of the city and the name of the player |
| Text | A message kind and a list of styled spans |
| Prompt | The prompt for the next input line |
| Candidates | The results of a tab completion |
| Close | The reason for the disconnection |
| Refresh | A new prompt after text that is not an answer to a command |

The [protocol](../protocol.md) document gives the full details. Two messages come later. Hello will give the protocol version, the client layer, and the terminal size. Status will give trace, integrity, heat, cycles, and RAM.

## Dependencies

TTYmud keeps dependencies to a minimum. Each dependency must have a reason in this table. If you add a dependency, add its row to this table in the same commit.

| Crate | Used by | Reason |
| --- | --- | --- |
| `snow` | net | The Noise protocol. TTYmud does not write its own cryptography. |
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
- **Scenario tests** for the game. Each file in `crates/game/tests/scenarios/` is a transcript of commands and their expected output. The test runs the commands on a test city and compares the outputs with the transcript. A comment line starts with `#`. A line such as `~ 3` lets three ticks go by.
- **Screen tests** for the client. They compare the bytes that the screen view writes with the expected bytes. The color layers will add one test for each color depth.
- **Round-trip tests** for the wire protocol.
- **Integration tests** that start a server and a client in one process on a local port. Some tests use a small fake server.
- **The STE check** for all documents.

## Continuous integration

GitHub Actions runs `cargo fmt --check`, `cargo clippy`, and `cargo test` on Linux, macOS, and Windows. It runs for each push to `main` and for each pull request. The workflow uses only the official checkout action, pinned to a commit hash.
