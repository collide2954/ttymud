# TTYmud

[![CI](https://github.com/collide2954/ttymud/actions/workflows/ci.yml/badge.svg)](https://github.com/collide2954/ttymud/actions/workflows/ci.yml)

TTYmud is a multiplayer text game for the terminal. You are a hacker in a near-future city. You take contracts, hack into corporate networks, and steal data. Security software and analysts try to trace you. If they trace you too often, your handle burns, and you start again with a new identity.

TTYmud is a MUD (multi-user dungeon). The networks are the dungeons. The hostile security software is the monster.

## Status

TTYmud is in early development. The foundation (milestone M0) is complete. There is no playable version yet. The [roadmap](docs/roadmap.md) gives the plan.

## Build

Install Rust with [rustup](https://rustup.rs). Then use these commands to build and test TTYmud:

    cargo build --workspace
    cargo test --workspace

## Goals

- **Easy to learn.** If you know a shell, you know how to play. If you do not know a shell, the game teaches you.
- **Hard to predict.** Targets change on each run. Corporations patch the exploits that players use too much.
- **Fair danger.** Hostile software shows its next action before it acts.
- **All terminals.** Line mode works on all terminals and with screen readers. Better terminals get a status bar, panes, and color.
- **Safe by default.** TTYmud encrypts all traffic. Your account is a key, not a password. The server removes terminal escape codes from all player text.
- **Easy to host.** One binary runs a city. The server keeps its data in plain text files.
- **Light.** TTYmud has very few dependencies.

## Documents

- [Design overview](docs/design/overview.md)
- [Roadmap](docs/roadmap.md)
- [Style guide](docs/style.md)
- [How to contribute](CONTRIBUTING.md)
- [Security policy](SECURITY.md)

## License

TTYmud is free and open source software. You can use it under the terms of the [Apache License 2.0](LICENSE).
