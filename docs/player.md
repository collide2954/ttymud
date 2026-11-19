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

Tab completes the names of commands, routes, files, and help topics. If more than one name is possible, Tab adds the part that the names have in common. If there is no part to add, the client shows all the possible names.

## Line mode

The client uses line mode when the input or the output is not a terminal. It also uses line mode when the `TERM` environment variable is `dumb`. On Windows, it uses line mode if the console cannot use virtual terminal sequences.

In line mode, the client reads full lines and writes full lines. Line mode works on all terminals and with screen readers. To use line mode in a terminal, use the `--line` option:

    ttymud connect ostra.example.net --line

## Play

Type `help` to see the commands. Type `?` to see the commands that you can use on the current host. To talk to the players on the same host, type `say` and your message. A line that starts with `'` is also a message.

Type `quit` to leave the game. The client waits for the answers to your last commands. Then it closes the connection.

Your name is `anon-` and the first eight characters of your key in hexadecimal. Handles with names that you select come in M4. If you connect again with the same identity, the new connection replaces the old connection.
