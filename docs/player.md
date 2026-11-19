# Player guide

This guide tells you how to connect to a TTYmud server and how to play.

## Build the client

Install Rust with [rustup](https://rustup.rs). Then use this command to build the client:

    cargo build --release -p ttymud

The binary is `target/release/ttymud`. On Windows, the binary is `target\release\ttymud.exe`.

## Connect

Use this command to connect to a server:

    ttymud connect ostra.example.net

The default port is 7331. To use a different port, add it after a colon, for example `ostra.example.net:7400`. Put an IPv6 address in brackets, for example `[2001:db8::7]:7400`.

## The first connection

At the first start, the client makes your identity key and shows its fingerprint. The key is your account. There is no password.

When you connect to a new server, the client shows the fingerprint of the server key:

    ostra.example.net:7331 is a new server for you. Its fingerprint is 9b5b:f04a:f430:799f.
    Type yes to trust it:

Compare the fingerprint with the fingerprint that the operator of the server publishes. If the two fingerprints are the same, type `yes`. The client then keeps the key of the server, and it does not ask again.

The `--accept-new` option trusts a new server without a question. Use it only for tests, or for a server that you know.

## A changed server key

If the key of a known server changes, the client stops and shows a warning. A changed key can mean that a different computer is between you and the server.

Do not continue if you are not sure. Ask the operator of the server. If the operator changed the key, remove the line for that server from the file `known_servers`. Then connect again.

## Your files

The client keeps your files in a configuration directory.

| System | Directory |
| --- | --- |
| Windows | `%APPDATA%\ttymud` |
| macOS, Linux, and other Unix systems | `$XDG_CONFIG_HOME/ttymud`, or `~/.config/ttymud` if `XDG_CONFIG_HOME` is not set |

To use a different directory, set the `TTYMUD_CONFIG` environment variable, or use the `--config` option. The directory contains these files:

- `identity.key`: your identity key. On Unix, the file has the permissions 0600.
- `known_servers`: the keys of the servers that you trust.

> CAUTION: Keep the file `identity.key` secret. A person with this file can use your account.

## Edit mode

In a terminal, the client edits your input line itself. Text from the server shows above the input line. As a result, it does not mix with the text that you type.

| Key | Action |
| --- | --- |
| Left, Right, Home, End | Move the cursor. Ctrl+B, Ctrl+F, Ctrl+A, and Ctrl+E do the same. |
| Backspace, Delete | Delete one character. |
| Ctrl+W, Alt+Backspace | Delete the word before the cursor. |
| Ctrl+U | Delete the text before the cursor. |
| Ctrl+K | Delete the text after the cursor. |
| Up, Down | Show an older or a newer line from the history. Ctrl+P and Ctrl+N do the same. |
| Tab | Complete the word before the cursor. |
| Enter | Send the line. |
| Ctrl+C | Clear the line. On an empty line, quit. |
| Ctrl+D | Delete the character at the cursor. On an empty line, quit. |

Tab completes the names of commands, routes, files, help topics, and the ICE that you know. If more than one name is possible, Tab adds the part that the names have in common. If there is no part to add, the client shows all the possible names.

## Line mode

The client uses line mode when the input or the output is not a terminal. It also uses line mode when the `TERM` environment variable is `dumb`. On Windows, it uses line mode if the console cannot use virtual terminal sequences.

In line mode, the client reads full lines and writes full lines. Line mode works on all terminals and with screen readers. To use line mode in a terminal, use the `--line` option:

    ttymud connect ostra.example.net --line

## Play

Type `help` to see the commands. Type `?` to see the commands that you can use on the current host. To talk to the players on the same host, type `say` and your message. A line that starts with `'` is also a message.

Type `quit` to leave the game. The client waits for the answers to your last commands. Then it closes the connection.

Your name is `anon-` and the first eight characters of your key in hexadecimal. Handles with names that you select come in M4. If you connect again with the same identity, the new connection replaces the old connection.

## Your first run

When you connect, the fixer Pell offers you a job. A dentist keeps patient files on an old box, and a client wants them. Type `contract` to see the job and the next step. If you make no progress for 30 seconds, the game gives you a hint.

Do these steps:

1. Go to your safehouse, where your rig is: `go safehouse`.
2. Connect to the target: `go smile-gw`. The prompt now shows your trace and your integrity.
3. Find the ICE on this host: `scan`.
4. Find the weak point of the Sentry, and crack it in the same pipeline: `probe sentry | crack`.
5. Find the host with the file `patients.db`. Then download the file: `download patients.db`.
6. Lower your trace: `scrub`.
7. Go back the way that you came in: `back`. Then the contract is complete.

## Cycles and noise

Programs cost cycles. Your rig has a maximum of 4 cycles, and it gets 1 cycle on each tick of two seconds. If a command needs more cycles than you have, it does not run. Type `status` to see your cycles.

Programs also make noise, and noise raises your trace. A Sentry on the host doubles the trace from noise. A pipeline makes less noise than the same commands on different lines.

## Telegraphs

ICE acts on its own time. Before an ICE acts, it shows its next action in a telegraph:

    hound starts to compile TRACE BURST (2 ticks).

In this example, you have two ticks to answer. You can crack the ICE, leave the host, or scrub after the attack. Type `help ice` to see what each type of ICE does.

## The end of a run

A run ends in one of four ways:

- **Clean exit.** You leave through the host where you came in. You keep the loot.
- **Jack out.** You type `jackout`. You keep the loot, but the shock costs your rig 20 integrity.
- **Traced.** The trace gets to 100%. You lose the loot of the run.
- **Crash.** The integrity of your rig gets to 0. You lose the loot of the run.

After a crash, your rig cannot connect to a target. Go to your safehouse and type `repair`.

> CAUTION: Do not disconnect if you want to keep your files. In version 0.1, the server does not save your rig.
