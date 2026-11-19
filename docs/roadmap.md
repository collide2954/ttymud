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

### M3: First hack

- Trace and integrity.
- ICE with telegraphs.
- Programs, RAM, and cycles: `scan`, `probe`, `crack`, `scrub`, and `patch`.
- Pipelines as combos.
- The tutorial contract: the network of a dental office.
- The four ends of a run: clean exit, jack out, traced, and crash.

### M4: Handles and heat

- Handles under accounts, and the four starter kits.
- Heat, burns, the burn list, know-how, and the offshore stash.
- Storage in plain text files.

### M5: Contracts

- The target generator, with the server seed.
- Defense in depth: the edge, LAN, core, and vault layers, and privilege levels.
- Fixers, contracts, loot, credits, and shops.

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
