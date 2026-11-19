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

## Privacy

The connections are encrypted, but the server can read all text that players send, also chat. Tell your players this. The server does not write chat to disk. It keeps the IP address of each connection in memory only.

## Game data

The server keeps the game data in plain text files in the data directory:

- `accounts/`: one file for each account. The file has the handle, the rig, the contract, the know-how, and the stash of the account. The name of the file is the public key of the account in hexadecimal.
- `burns.ini`: the burn list.

The server writes a file each time its data changes. A storage thread writes the files, so a slow disk does not stop the game. The server writes each file to a temporary file first, and then it renames the file. As a result, a crash cannot leave a damaged file.

When the server starts, it reads all the files. If a file has an error, the server shows the file, the line, and the error. Then it stops.

> CAUTION: Stop the server before you change a file in the data directory. If the server runs, it can write over your change.

To make a backup, stop the server and copy the data directory. To delete an account, stop the server and delete the file of the account.

The ICE selects its actions with random numbers. The server takes the seed from the clock when it starts. As a result, the ICE acts in a different order after each start. A seed in the data directory comes in M5.

## Try the city offline

Use this command to walk Port Ostra alone, without a network:

    ttymud-server play

The game asks for a handle and a starter kit, as on a server. To give the handle on the command line, put it after `play`, for example `ttymud-server play nyx`. The offline mode does not save anything.

Type `quit` to stop. The offline mode uses real time, as the server does. As a result, the ICE acts on each tick of two seconds.
