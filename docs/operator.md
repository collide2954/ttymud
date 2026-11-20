# Operator guide

This guide tells you how to run a TTYmud server. A server runs one city. In version 0.1, the server runs the default city, Port Ostra.

## Build the server

Install Rust with [rustup](https://rustup.rs). Then use this command to build the server:

    cargo build --release -p ttymud-server

The binary is `target/release/ttymud-server`. On Windows, the binary is `target\release\ttymud-server.exe`.

## Make a data directory

The server keeps its data in a data directory. Use this command to make one:

    ttymud-server init ostra-data

The command makes the directory and a new server key in the file `server.key`. Then it shows the fingerprint of the key. If the directory already has a key, the command does not change the key. It only shows the fingerprint again.

On Unix, the key file has the permissions 0600.

> CAUTION: Keep the file `server.key` secret. A person with this file can pretend to be your server.

## Start the server

Use this command to start the server:

    ttymud-server run ostra-data

By default, the server accepts connections on all IPv4 interfaces, on port 7331. To use a different address or port, use the `--listen` option:

    ttymud-server run ostra-data --listen 127.0.0.1:7400

When the server starts, it shows the name of the city, its address, the fingerprint of its key, and the number of accounts. To stop the server, push Ctrl+C.

## Publish the fingerprint

At the first connection to a server, the client shows the fingerprint of the server key. The player must compare it with a fingerprint from you. Publish the fingerprint in a place that your players trust, for example your web site.

If the key of a server changes, each client that knows the server stops with a warning. Do not change the key without a good reason. If you must change the key, tell your players before you change it.

## Open the port

Players connect with TCP to port 7331, or to the port that you set with `--listen`. If the server is behind a firewall or a router, open that port for TCP.

## Limits

The server has these limits:

- A maximum of 256 connections at the same time.
- A maximum of 8 connections from one IP address.
- A maximum of 10 seconds for each handshake.
- Rate limits for commands and chat. The [protocol](protocol.md) document gives the values.

## Moderators

Moderators help you keep the chat safe. Use the `mod` command to select them. To show the moderators, use this command:

    ttymud-server mod ostra-data

To add or remove a moderator, give the handle:

    ttymud-server mod ostra-data add nyx
    ttymud-server mod ostra-data remove nyx

The command writes the file `moderators.ini`. Restart the server to use the change. A moderator is an account, so the account stays a moderator after a burn. To remove a moderator without a handle, give the name of the account file in `accounts/`, without `.ini`.

Moderators have these commands in the game:

| Command | Action |
| --- | --- |
| `mute <handle> <minutes>` | Stops the chat messages of a handle. `unmute <handle>` lets the handle talk again. |
| `kick <handle>` | Disconnects a handle. |
| `ban <handle> [ip]` | Disconnects a handle and refuses its account. With `ip`, it also refuses the IP address of the handle. `unban <handle>` removes the ban. |
| `bans` | Shows the bans. |
| `reports [number]` | Shows the open reports, or one report with its chat. |
| `resolve <number>` | Closes a report. |

A moderator cannot mute, kick, or ban a different moderator. The server writes each action of a moderator to the file `audit.log`. The times in the file are in UTC.

> CAUTION: Use IP bans carefully. Many players can share one IP address, for example in a school or behind a mobile network.

## Privacy

The connections are encrypted, but the server can read all text that players send, also chat. Tell your players this. The server writes chat to disk only in a report. The file `reports.ini` keeps the chat of the reported handle from the 15 minutes before the report.

The server keeps the IP address of each connection in memory only. An IP ban writes the address to the file `bans.ini`.

## Game data

The server keeps the game data in plain text files in the data directory:

- `accounts/`: one file for each account. The file has the handle, the rig, the credits, the reputation, the contract, the know-how, and the stash of the account. The name of the file is the first public key of the account in hexadecimal.
- `keys.ini`: the keys of each account, and a hash of each recovery code. The server never keeps a recovery code.
- `burns.ini`: the burn list.
- `world.ini`: the seed of the world. The server makes the file at its first start. A `[corp]` section keeps the state of each corporation: its alert level, its exploit counts, and its patches.
- `news.ini`: the last 50 headlines of the news feed.
- `crews.ini`: the crews, with their members, their reputation, and their heists.
- `moderators.ini`: the moderators. The `mod` command writes this file.
- `audit.log`: the actions of the moderators, one on each line.
- `reports.ini`: the reports of the players, with their chat.
- `bans.ini`: the banned accounts, and the IP addresses of IP bans.

The server writes a file each time its data changes. A storage thread writes the files, so a slow disk does not stop the game. The server writes each file to a temporary file first, and then it renames the file. As a result, a crash cannot leave a damaged file.

When the server starts, it reads all the files. If a file has an error, the server shows the file, the line, and the error. Then it stops.

> CAUTION: Stop the server before you change a file in the data directory. If the server runs, it can write over your change.

To make a backup, stop the server and copy the data directory. To delete an account, stop the server and delete the file of the account. Then remove the section of the account from `keys.ini`.

The seed in `world.ini` sets the job board and the target networks. As a result, a job has the same network after a restart. Keep the file. If you lose it, the server makes a new seed. Then the jobs on the board and the accepted jobs change.

Do not show the seed to players. With the seed and the server code, a person can make the same targets and find each goal file before a run.

The ICE selects its actions with a different seed, which the server takes from the clock when it starts. As a result, the ICE acts in a different order after each start.

## Try the city offline

Use this command to walk Port Ostra alone, without a network:

    ttymud-server play

The game asks for a handle and a starter kit, as on a server. To give the handle on the command line, put it after `play`, for example `ttymud-server play nyx`. The offline mode does not save anything. The commands for recovery codes and device links work only on a server.

Type `quit` to stop. The offline mode uses real time, as the server does. As a result, the ICE acts on each tick of two seconds.
