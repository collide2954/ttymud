# Security

This document gives the security design of TTYmud. Security is a pillar of TTYmud. All features must agree with this document.

## Threats

TTYmud protects players and servers against these threats.

| Threat | Protection |
| --- | --- |
| An attacker reads or changes network traffic. | Encryption with the Noise protocol. |
| An attacker uses the account of a different person. | Key pairs. There are no passwords. |
| An attacker steals the data of a server. | The server keeps no passwords and no private keys of players. |
| A false server pretends to be a known server. | The client keeps the key of each server and stops if the key changes. |
| A player attacks the terminal of a different player with escape codes. | The server removes control characters. The client shows only style marks. |
| A player uses Unicode tricks to fake a name or to hide text. | Strict rules for names. The server removes bidi controls and invisible characters. |
| A player sends too many messages or commands. | Rate limits. |
| A player harasses a different player. | Ignore, block, report, and moderator tools. |
| An attacker opens too many connections. | Connection limits and handshake time limits. |

TTYmud does not protect against these threats:

- The server operator can read all game data and all chat, also private messages. Before the first private message of a player, the client tells the player this.
- A person with access to your computer can copy your identity key.

## Transport

The client and the server use the Noise protocol framework with the pattern `Noise_XX_25519_ChaChaPoly_BLAKE2s`. The `snow` crate supplies the protocol. TTYmud does not use its own cryptography.

- The XX pattern authenticates the static keys of both sides.
- The prologue is `ttymud/1`. It contains the protocol name and the protocol version. As a result, a downgrade attack fails.
- Each frame has a length prefix of two bytes. The maximum frame size is 65535 bytes.

## Server keys

The `ttymud-server init` command makes the static key of the server. The server keeps the key in the file `server.key` in its data directory.

The client keeps the key of each server that it connects to, as SSH does. Before the client trusts a new server, it shows the fingerprint of the server key and asks the player. If the key of a known server changes, the client stops and shows a warning.

A server operator can publish the fingerprint of the server key. In a later version, a server address will contain the fingerprint. Then the client will also check the key at the first connection.

## Identity

The client makes a static key pair at its first run. The public key identifies the account. The Noise handshake proves that the client has the private key. There is no password.

- **Recovery code.** When the server makes a new account, it shows a recovery code one time. The code contains 128 bits of random data. The server keeps only a hash of the code. With the code, a new key can replace a lost key.
- **More devices.** On a connected device, the `link` command gives a link code for one use. A new device uses the code to add its key to the account.
- **Revocation.** The `keys` command shows all keys of the account. The `revoke` command removes a key.

The client keeps the private key in the configuration directory of the user. On Unix, the key file has the permissions 0600.

> CAUTION: Keep your identity key and your recovery code secret. A person with one of them can use your account.

## Names

Handle names and crew names contain only ASCII letters, digits, hyphens (`-`), and underscores (`_`). A name has 3 to 16 characters. The server compares names without case, so "Nyx" and "nyx" are the same name. As a result, nobody can fake a name with similar characters.

A handle name cannot start with `anon-`. The server uses these names for players without a handle.

## Text sanitizer

All text from players goes through one sanitizer on the server before other players see it. The sanitizer does these steps:

1. It removes all control characters, also escape (ESC) and the C1 controls.
2. It removes bidi control characters, for example U+202E.
3. It removes invisible characters, for example U+200B and U+FEFF.
4. It keeps a maximum of two combining marks after each base character.
5. It limits the message length to 400 characters.

The client also removes control characters from all text before it shows the text. This gives a second layer of protection.

## Chat safety

- **Rate limits.** Each account has a token bucket for chat and a token bucket for commands.
- **New accounts.** A new account cannot send private messages for its first 10 minutes. This makes spam more difficult.
- **Repeated messages.** If a player sends the same message again within 30 seconds, the server blocks it.
- **Ignore and block.** `ignore <handle>` hides the messages of a player. `block <handle>` also stops the private messages of that player.
- **Reports.** `report <handle>` sends the recent messages of that player to the moderators. The server keeps the last 15 minutes of chat in memory only, so that a moderator can confirm a report.
- **Moderators.** Moderators can mute, kick, and ban accounts. A ban applies to the account key and, as an option, to an IP address. The server writes all moderator actions to an audit log.

## Privacy

- By default, the server does not write chat to disk. If the operator turns on chat logs, the server tells each player at login.
- By default, the server keeps IP addresses in memory only.
- An operator can delete an account and all of its data with one command.

## Protection of the server

- All TTYmud crates forbid unsafe code.
- The server runs as a normal user, without root privileges.
- The server reads and writes files only in its data directory.
- The server does not start other programs.
- The server checks all content files and configuration files when it starts. It limits the size of each input.
- The server limits the number of connections in total and from each IP address. It closes each handshake that takes more than 10 seconds.

## Status

Version 0.1 has these parts of the design:

- The Noise transport, with the prologue `ttymud/1` and frames with a length prefix of two bytes.
- Server keys, and the list of known servers in the client. The client asks the player before it trusts a new server.
- Identity keys. On Unix, all key files have the permissions 0600.
- The text sanitizer on the server. The client also removes unsafe characters from all text from the server, and its line editor never keeps control characters.
- Rate limits for each connection. The command bucket holds 20 tokens and gets 10 tokens each second. The chat bucket holds 5 tokens and gets 1 token every 2 seconds.
- Connection limits: 256 in total, and 8 from each IP address. A handshake has a maximum of 10 seconds.
- One connection for each key. A new connection with the same key replaces the old connection.
- Handle names with the rules in the [names](#names) section.
- Save files in plain text. The storage thread writes only the files `burns.ini` and `world.ini`, and the files in `accounts/`. It refuses all other paths.
- A secret world seed. The client never gets the seed, because a player with the seed can make the targets before a run.

Until a player selects a handle, the Welcome message gives the name `anon-` and the first eight hexadecimal characters of the public key. Other players do not see a player without a handle. The server always uses the full key to identify an account. Recovery codes, device links, key revocation, and the chat safety tools come in M6.
