# Design overview

This document gives the idea of TTYmud, the design pillars, and the main design decisions. It gives the planned design. Some parts do not exist yet.

## The game

TTYmud is a multiplayer online text game for the terminal. It is a MUD (multi-user dungeon) with a hacker setting. The networks of corporations are the dungeons. Hostile security software and security analysts are the monsters.

You play a hacker in a near-future city. You take contracts from fixers. You hack into a target network, find the data, and leave before the security team traces you. With the money, you make your rig better and take more dangerous contracts.

The danger is real. Each time a target traces you, the police learn more about your handle. When they know too much, your handle burns. You lose the handle and almost all that it has. You keep your know-how, and you start again with a new handle.

## Tone

The tone is gritty cyberpunk. The technology feels real, and there is no magic. The game uses real security ideas: firewalls, intrusion detection, privilege levels, log files, and air gaps. The humor is dry and dark.

## Setting

Twenty years ago, a chain of satellite collisions filled the sky with debris. People call this event the Cascade. After the Cascade, satellites stopped working, and the long links between cities failed. Each city now has its own network of old cables, radio relays, and corporate fiber. The corporations that own the cables own the city.

Each TTYmud server is one city. The default city is Port Ostra, an old port city that is wet and in debt. A server operator can change the city or write a new city.

## Pillars

These pillars guide all design decisions. If a feature does not agree with a pillar, change the feature or remove it.

1. **Learn as you play.** Commands look and work like shell commands. One command always works, and pipelines give more power to experts. A shell user can play immediately. A new player learns a real shell skill.
2. **Fair danger.** ICE shows its next action before it acts. Each loss has a cause that the player can see.
3. **Never the same run twice.** The server makes targets from a seed. Corporations patch the exploits that players use too much. Analysts work in shifts.
4. **Each choice has a cost.** Speed makes noise, and stealth takes time. More programs use more RAM. The best loot is deep in a network, where a trace gives more heat.
5. **The world remembers.** The news reports breaches. Burned handles go on a public list. The police sell seized rigs at auction.
6. **Works on all terminals.** Line mode works on all terminals and with screen readers. More layers add features when the terminal can show them.
7. **Safe by default.** The Noise protocol encrypts all traffic. Accounts use keys, not passwords. The server removes escape codes and Unicode tricks from all player text.
8. **Light and open.** TTYmud has very few dependencies. Anyone can run a server. The license is Apache 2.0.

## Core loop

1. Get a contract from a fixer in a hub.
2. Prepare for the run. Load programs into the RAM of your rig. Buy tools. Find a crew.
3. Connect to the target network. Move from host to host. Use your programs to scan, probe, and crack. Make as little noise as possible.
4. Find the objective. Download the data, or do the sabotage.
5. Leave the network. Use a clean exit, or jack out if the trace gets too high.
6. Collect the payment. Sell the data, and use the credits for your rig and programs.
7. Watch your heat. When your heat is high, stop and wait. Heat goes down over time.

A run takes 10 to 30 minutes. A player can do a full run in a short session.

## Decisions

This table records the main design decisions. When a decision changes, change this table.

| Topic | Decision |
| --- | --- |
| Tone | Gritty cyberpunk. No fantasy layer. |
| Danger | Two meters: trace and integrity. |
| Time | Real time, with ticks of about two seconds. ICE shows its next action before it acts. |
| Death | Each trace adds heat. Heat goes down over real time. At maximum heat, the handle burns. |
| Legacy | After a burn, the player keeps know-how and a small offshore stash. |
| World | Hubs that authors make by hand, and targets that the server makes from a seed. |
| Progression | No levels. Rig hardware, program versions, and faction reputation. |
| Roles | No classes. The programs in RAM set the role. Starter kits help new players. |
| PvP | PvE first. PvP only by consent: duels, matches, and lawless zones. |
| Core mechanics | Pipelines as combos, exploit decay, defense in depth, and player scripts (later). |
| World features | Heist crews, SOC shifts, auctions of seized rigs, and a news feed with rumors. |
| Chat | IRC style: same-host chat, public channels, crew channels, and private messages. |
| Chat security | Encrypted traffic, sanitized text, rate limits, ignore, block, and moderator tools. No end-to-end encryption. |
| Identity | A key pair is the account. A recovery code can replace a lost key. |
| Connection | A TTYmud client and a TTYmud server that use the Noise protocol. |
| Interface | Adaptive layers: line mode for all terminals, and a HUD when the terminal can show it. |
| Code | A Cargo workspace with two binaries: `ttymud` and `ttymud-server`. |
| Text model | A small `ttymud-text` crate holds styles, spans, and messages. |
| Look and list | `look` describes the host in words. `ls` gives a short list. `cd` works like `go`, and `cd ..` goes back. |
| Offline mode | `ttymud-server play` lets one player walk Port Ostra without a network. It uses real time, as the server does. |
| Programs | A program acts at once and costs cycles. The rig gets 1 cycle on each tick. |
| Runs | Each run is a private copy of the target network. Crews share a copy later. |
| Noise | Noise raises the trace at once. A working Sentry doubles it. The trace does not increase while you wait. |
| ICE in version 0.1 | Sentry, Warden, Hound, and Scorch. The other types come later. |
| RAM in version 0.1 | A fixed rig with 8 GB and five programs. Loads and starter kits come in M4. |
| First contract | The fixer Pell offers it on connect. Its steps and hints come from the content file. |
| Repair | The `repair` command at the home host. It is free until credits come in M5. |
| Hosting | A public GitHub repository. The default branch is `main`. |
| Documents | ASD-STE100 Simplified Technical English. A test checks the basic rules. |

## Documents

| Document | Contents |
| --- | --- |
| [gameplay.md](gameplay.md) | Hosts, commands, pipelines, danger, ICE, programs, roles, and progression. |
| [world.md](world.md) | Zones, targets, corporations, the SOC, exploit decay, news, economy, crews, and PvP. |
| [interface.md](interface.md) | Client layers, terminal support, accessibility, and the first run. |
| [security.md](security.md) | Threats, transport, identity, chat safety, and privacy. |
| [architecture.md](architecture.md) | Crates, binaries, server threads, storage, content, protocol, and tests. |
