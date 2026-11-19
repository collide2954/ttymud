# TTYmud

[![CI](https://github.com/collide2954/ttymud/actions/workflows/ci.yml/badge.svg)](https://github.com/collide2954/ttymud/actions/workflows/ci.yml)

TTYmud is a multiplayer text game for the terminal. You are a hacker in a near-future city. You take contracts, hack into corporate networks, and steal data. Security software and analysts try to trace you. If they trace you too often, your handle burns, and you start again with a new identity.

TTYmud is a MUD (multi-user dungeon). The networks are the dungeons. The hostile security software is the monster.

## Status

TTYmud is in early development. Milestones M0 to M3 are complete. You can run a server and play the default city with other players over an encrypted connection. Your first job is a hack into the network of a dental office. Handles and heat come in M4. The [roadmap](docs/roadmap.md) gives the plan.

## Build

Install Rust with [rustup](https://rustup.rs). Then use these commands to build and test TTYmud:

    cargo build --workspace
    cargo test --workspace

## Try it

Use this command to walk Port Ostra alone, without a network:

    cargo run -p ttymud-server -- play

Type `help` to see the commands. Type `contract` to see your first job. Type `quit` to leave.

## Play on a network

Use these commands to make a data directory and to start a server on your computer:

    cargo run --release -p ttymud-server -- init ostra-data
    cargo run --release -p ttymud-server -- run ostra-data

Then use this command in a different terminal to connect:

    cargo run --release -p ttymud -- connect localhost

The [player guide](docs/player.md) tells you how to play. The [operator guide](docs/operator.md) tells you how to run a server for other players.

## Goals

- **Easy to learn.** If you know a shell, you know how to play. If you do not know a shell, the game teaches you.
- **Hard to predict.** Targets change on each run. Corporations patch the exploits that players use too much.
- **Fair danger.** Hostile software shows its next action before it acts.
- **All terminals.** Line mode works on all terminals and with screen readers. Better terminals get a status bar, panes, and color.
- **Safe by default.** TTYmud encrypts all traffic. Your account is a key, not a password. The server removes terminal escape codes from all player text.
- **Easy to host.** One binary runs a city. The server keeps its data in plain text files.
- **Light.** TTYmud has very few dependencies.

## Documents

- [Player guide](docs/player.md)
- [Operator guide](docs/operator.md)
- [Design overview](docs/design/overview.md)
- [Roadmap](docs/roadmap.md)
- [Protocol](docs/protocol.md)
- [Content files](docs/content.md)
- [Style guide](docs/style.md)
- [How to contribute](CONTRIBUTING.md)
- [Security policy](SECURITY.md)

## License

TTYmud is free and open source software. You can use it under the terms of the [Apache License 2.0](LICENSE).
