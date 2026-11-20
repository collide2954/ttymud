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

### M11: PvP by consent

- Duels and capture-the-flag matches.
- Darknet zones.

## Version 0.3 and later

- **M12: Player scripts.** A small language like a shell, with the same commands and pipelines. Scripts run on the rig and use its RAM and cycles.
- An SSH gateway as an optional Cargo feature.
- A public list of servers.
- Links between cities.

## Open questions

- The syntax of the script language.
- The final values for trace, heat, and prices.
- The full content of Port Ostra.
- The format of a server address with a fingerprint.
