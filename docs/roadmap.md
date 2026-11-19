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

The vault layer comes with bridges in M9.

### M6: Chat relay

- Channels, crew channels, and private messages.
- Ignore, block, report, and moderator tools.
- Recovery codes, device links, and key revocation.

## Version 0.2: the city comes alive

### M7: HUD

- Terminal detection, color depths, and the ASCII fallback.
- The HUD layer, with the status bar and the panes.

### M8: The city reacts

- SOC shifts, analysts, and alert levels.
- Exploit decay.
- The news feed and rumors.

### M9: Crews and heists

- Crews and crew reputation.
- Group runs, and heist contracts with stages.
- Bridges into vaults.

### M10: Economy

- Blueprints and the compile system.
- The evidence auction.
- Data prices with supply and demand.

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
