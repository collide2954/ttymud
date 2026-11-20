# TTYmud

[![CI](https://github.com/collide2954/ttymud/actions/workflows/ci.yml/badge.svg)](https://github.com/collide2954/ttymud/actions/workflows/ci.yml)

TTYmud is a multiplayer text game for the terminal. You are a hacker in a near-future city. You take contracts, hack into corporate networks, and steal data. Security software and analysts try to trace you. If they trace you too often, your handle burns, and you start again with a new identity.

TTYmud is a MUD (multi-user dungeon). The networks are the dungeons. The hostile security software is the monster.

The website is at [collide2954.github.io/ttymud](https://collide2954.github.io/ttymud/).

## Status

TTYmud is in early development. Milestones M0 to M17 are complete. You can run a server and play the default city with other players over an encrypted connection. In a terminal, the client shows a HUD with status bars, an output pane, and a chat pane.

You select a handle and a starter kit, and the server keeps them. Your first job is a hack into the network of a dental office. After that, fixers post new jobs each day against six corporations. The server makes a new network for each job. A job asks you to steal a file, to delete a file, or to put a false file on a host. You earn credits, buy programs and hardware, and sell stolen data to a fence.

The corporations react to all players. Noise raises their alert levels, and analysts read the logs of each run. Analysts lock stolen accounts and reset their passwords, and a strict analyst can cut a route. A corporation patches the software that players exploit too often. Each target has a honeypot file, and bigger corporations use stronger ICE. The news feed reports the events of the city, and the fixers tell rumors.

You can make a crew of up to 4 handles. Crewmates share one copy of a target network, and each role helps the crew. Crews take heists, and the goal of a heist is in a vault that has no route.

You compile stronger versions of your programs from blueprints. The fence pays less for data that many players sell. The police auction the rigs of burned handles.

Handles can attack each other only by consent. In a darknet, other handles can crash your rig, take its data, and tip off the police. In the arena, handles fight duels for credits, and crews play capture the flag.

Your rig can run scripts: files of commands that you write with `echo`, as in a shell. A script runs one command on each tick, uses `&&` and `||`, and costs RAM and cycles.

An operator can also open an SSH port with the `ssh` feature. Then players can play with a normal SSH client, and their SSH key is their account.

The client and the website have a public list of servers. You can connect to a server on the list by its name, and the client checks the key of the server.

Operators can make links between their servers. The players of linked cities talk in one channel, and each city sends its headlines to the others. An operator can also write a new city in one text file, and the `check` command finds its errors.

If the targets trace you too often, your handle burns. You can talk in channels and send private messages. Moderators and safety tools keep the chat safe, and a recovery code keeps your account safe. The [roadmap](docs/roadmap.md) gives the plan.

## Build

Install Rust with [rustup](https://rustup.rs). Then use these commands to build and test TTYmud:

    cargo build --workspace
    cargo test --workspace

## Try it

Use this command to walk Port Ostra alone, without a network:

    cargo run -p ttymud-server -- play

Type a handle, and then select a starter kit. Type `help` to see the commands. Type `contract` to see your first job, and type `jobs` in the Back Room for more work. Type `quit` to leave.

## Play on a network

Use these commands to make a data directory and to start a server on your computer:

    cargo run --release -p ttymud-server -- init ostra-data
    cargo run --release -p ttymud-server -- run ostra-data

Then use this command in a different terminal to connect:

    cargo run --release -p ttymud -- connect localhost

To see the public list of servers, use this command:

    cargo run --release -p ttymud -- servers

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

- [Website](https://collide2954.github.io/ttymud/)
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
