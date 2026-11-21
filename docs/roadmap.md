# Roadmap

TTYmud grows in small milestones. At the end of each milestone, the game works and all tests pass. This roadmap changes when the design changes.

## Version 0.1: the first playable version

### M0: Foundation (done)

- The Cargo workspace with the crates `game`, `wire`, `server`, and `client`, and the tool `stelint`.
- Workspace lints, and settings for rustfmt and clippy.
- The STE check for the documents.
- Continuous integration on Linux, macOS, and Windows.

### M1: Offline core (done)

- Hosts, routes, files, and movement.
- The command parser, with pipelines.
- Help, `?`, and suggestions for unknown commands.
- Styled text spans.
- A test harness that runs the world without a network.
- The offline play mode: `ttymud-server play`.

### M2: Secure connection (done)

- The wire protocol and its encoding.
- The Noise transport with `snow`.
- Identity keys and known server keys. The server gives each key a temporary name.
- The client in line mode and edit mode, with tab completion.
- `say`, with the text sanitizer, rate limits, and connection limits.

### M3: First hack (done)

- Game ticks in the server and in offline play, and a Refresh message for the prompt.
- Trace and integrity, with noise from programs.
- ICE with telegraphs: Sentry, Warden, Hound, and Scorch.
- Programs, RAM, and cycles: `scan`, `probe`, `crack`, `scrub`, and `patch`.
- Pipelines as combos: one setup cycle, one connection, the weak point, and checks of the items.
- The tutorial contract from Pell: the network of a dental office, with steps and hints.
- The four ends of a run: clean exit, jack out, traced, and crash.
- `status`, `jackout`, `repair`, and `contract`.

### M4: Handles and heat (done)

- Accounts with one handle each. The player selects the handle name and a starter kit in the game.
- The four starter kits, with the programs `firewall`, `mask`, `proxy`, and `pretext`.
- `load` and `unload` at the home host, which is private.
- Heat from the security level of the target, and heat that cools in real time.
- Burns, the burn list, know-how, and the offshore stash.
- A disconnect during a run counts as a jack out.
- Storage in plain text files, with a storage thread.

### M5: Contracts (done)

- The world seed in `world.ini`.
- The target generator: corps and host templates in the content file, and targets with an edge, a LAN, and a core.
- Security levels for each host, and ICE versions that follow them.
- Access levels: guest, user, and admin. Credentials in the targets, and the `exploit` program.
- Fixers with a daily job board, `jobs` and `accept`, and reputation with each fixer.
- Credits, contract pay, fees at heat 4, and repairs that cost credits above 50 integrity.
- Shops with programs, RAM, and CPUs, and a fence that buys stolen data.

M9 added the vault layer, with bridges.

### M6: Chat relay (done)

- Channels from the content file, with `#<channel> <text>`, `channels`, `join`, and `leave`.
- `who`, and private messages with `tell` and `reply`.
- Ignore and block, which the server keeps with the account.
- Reports with the chat of the last 15 minutes, for the moderators.
- Moderators that the operator selects, with mute, kick, ban, and an audit log.
- Recovery codes, link codes for more devices, and key revocation.

M9 added crew channels.

## Version 0.2: the city comes alive

### M7: HUD (done)

- A Status message with the state of the handle.
- Terminal detection, display layers, color depths, and the ASCII fallback.
- The colors of the terminal theme for the styles of the text.
- The HUD, with two status bars, an output pane with scrollback, a chat pane, and the input line.
- A settings file, and the verbose setting for screen readers.

### M8: The city reacts (done)

- Alert levels for each corporation, from the noise, traces, and breaches of all players. A higher alert adds trace, analysts, and ICE versions.
- SOC shifts on the UTC clock, with analysts who read the logs of a run and lock stolen accounts. `pretext` tricks an analyst who is not strict.
- Exploit decay: software families in the templates, and patches that stay until somebody finds a new hole.
- The news feed with `news` and `news.txt`, and the newest headline in the Status message and the HUD.
- Rumors from the fixers with `rumors`: true, old, or false.

### M9: Crews and heists (done)

- Crews of up to 4 handles with a leader, invites, the `#crew` channel, and crew reputation, in `crews.ini`.
- Group runs: crewmates on the same job share one copy of the network, and ICE and analysts act against one player at a time.
- `scrub <handle>` for a crewmate, and the `decoy` program, which Breakers know.
- Heists on the board for crews of 2 to 4, with stages, pay for each crewmate who enters, and crew reputation.
- Vaults with bridges: the sync of a maintenance laptop, a USB drive from a tricked admin, or an old modem line.
- The crewmates in your copy and their traces in the Status message and the HUD.

### M10: Economy (done)

- Versions 1 to 3 for six programs, and the quiet mod.
- Blueprints as know-how: in job targets, in heist vaults, and in shops. The `compile` command at the home host.
- Data prices for each file name. Each sale lowers the price, and the price comes back with time.
- The police auction of the rigs of burned handles, with `lots` and `bid`, in `auction.ini`.

### M11: PvP by consent (done)

- Darknet zones that all handles share, with consent by a second `go`, and no trace.
- `probe` and `crack` against the rigs of other handles, with `firewall` and `patch` as defenses. Crewmates cannot attack each other.
- A crash in a darknet leaves the valuable files on the host and wakes the handle at home.
- `tip <handle>`: 1 heat, one tip in 24 hours, and never the fifth heat.
- A darknet fence that pays 150% with its own prices, and blueprints for version 3 in shops.
- Arena networks with two bases, duels with wagers, and capture-the-flag matches between crews.
- The Undercity darknet and the Grid arena in Port Ostra.

## Version 0.3: the rig works for you

### M12: Player scripts (done)

- Scripts as files on the rig that end in `.sh`, with `echo` and the `>` and `>>` arrows, `cat`, and `rm`.
- `sh <script> [words]`, with the words in `$1` to `$9`. A script runs one command on each tick and waits for the cycles of each command.
- `&&` and `||` between pipelines, `wait [ticks]`, and notes that start with `#`. The script stops when the last command of a line fails.
- 1 GB of RAM while a script runs, one script at a time, and `ps` and `kill`.
- A check of each line before the script starts. A script cannot chat, send challenges, or start a script.
- A crash, a trace, a burn, or a disconnect stops the script.
- The sample script `break.sh` at the Night Market of Port Ostra.

### M13: SSH gateway (done)

- The `ssh` feature of the server, with the `russh` crate. The default build stays without Tokio and without SSH code.
- `ttymud-server run --ssh <address>`, and an Ed25519 host key in `ssh_host.key`.
- Public keys only. The SHA-256 digest of an SSH key is an account key, and `link` adds an SSH key to an account.
- All display layers over SSH: the server runs the display of the client for the SSH terminal, with a decoder for the key bytes.
- Words after the address for the layer, ASCII or UTF-8, and verbose output.
- The same connection limits and rate limits as the Noise port, and 10 seconds for a login.
- A client that quits even when Windows refuses to shut down its socket.

## Version 0.4: many cities

### M14: Public server list (done)

- The file `servers.ini` in the repository, with a name, a city, an address, a full key, and one line of text for each server.
- A test for each entry. A change to the list needs a pull request.
- `ttymud servers` shows the list. `ttymud connect <name>` connects to a server on the list and checks the key from the list.
- `ttymud connect <address> --key <key>` checks the full key of a different server. `ttymud-server init` and `run` show the full key.
- A Servers page on the website, from the same list.
- The ini reader moves to the `text` crate, so the client reads the list without the `game` crate.

### M15: Linked cities (done)

- The file `links.ini` in the data directory. A link starts only when both servers list each other with the full key.
- Links on the game port. The server with the lower key connects, and both sides send a Hello message. Ping messages, a rate limit, and new tries after a failure.
- The linked channel `#longline` in Port Ostra. Handles from other cities have names such as `nyx@night-owl`.
- `cities`, and `news <city>` for the headlines of a linked city.
- `ignore`, `block`, `report`, and `mute` for handles of linked cities. Private messages stay in their city.

## Version 0.5: deeper runs and your own city

### M16: New ICE, smarter analysts, and new jobs (done)

- Tarpit and Lockjaw ICE. The TAR of a Tarpit takes all cycles. The BITE of a Lockjaw freezes the last program for 6 ticks.
- A honeypot file in each target. `probe <file>` and `ls | probe` find it.
- An analyst who locks a stolen account also resets its password until the next day.
- A strict analyst who finds your trail can cut one route that leads deeper, with a telegraph 2 ticks before.
- Wipe jobs with `rm <file>`, and plant jobs with `upload <file>`. About one job in three is a wipe or a plant.
- The command `ttymud-server delete` deletes an account and its data. The records of the moderators stay.

### M17: Custom cities (done)

- The file `city.ini` in the data directory replaces Port Ostra. Without it, the server runs the Port Ostra of the binary.
- `ttymud-server city` writes a copy of Port Ostra to start from. It never writes over a city file.
- `ttymud-server check` reads the data directory as `run` does, and shows all errors with their files and lines.
- `ttymud-server play --city` walks a city file offline.
- When `world.ini` is for a different city, the message tells the fix.

### M18: A fair first job and harder runs (done)

An offline playtest found the problems that this milestone repairs.

- The first contract: one step for `probe sentry | crack`, a step for the Warden, and steps that start again after a failed run. It has no Hound, and a trace in it adds no heat.
- A working Sentry sees you enter its host, and its ALERT then takes 5 ticks. `mask` makes a Sentry lose you.
- A login with stolen credentials leaves a line in the logs, and analysts can find it. A reset password works again in the next run.
- An analyst shows only when the host has logs to find.
- No two jobs or heists on a board have the same company and goal file.
- `repair` does the free part and shows the price of the rest. `repair all` pays for it.

### M19: Release binaries (done)

- A release workflow builds five archives for each version tag: static Linux for x86_64 and ARM64, macOS for ARM64 and x86_64, and Windows for x86_64.
- Each archive has the client and the server with SSH support. Players and operators do not need Rust.
- Each release has the file `SHA256SUMS` and a GitHub build attestation.
- The workspace version is 0.5.0, and the first release is `v0.5.0`.

## Later

- Travel between linked cities.

## Open questions

- The final values for trace, heat, and prices.
- The full content of Port Ostra.
