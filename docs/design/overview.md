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
| Danger | Two meters for each run: trace and integrity. Heat stays with the handle. |
| Time | Real time, with ticks of about two seconds. ICE shows its next action before it acts. |
| Death | Each trace adds heat. Heat goes down over real time. At maximum heat, the handle burns. |
| Legacy | After a burn, the account keeps its know-how and an offshore stash of 3 files. |
| Handles | One handle for each account. The player selects the name and a starter kit in the game. |
| Heat | The security level of the host sets the heat for a trace. Heat goes down by 1 for each 24 hours. |
| Burn list | A file on a hub host. In Port Ostra, it is on the Newsstand. |
| World | Hubs that authors make by hand, and targets that the server makes from a seed. |
| Progression | No levels. Rig hardware, program versions, and faction reputation. |
| Roles | No classes. The programs in RAM set the role. Starter kits help new players. |
| PvP | PvE first. PvP only by consent: duels, matches, and lawless zones. |
| Core mechanics | Pipelines as combos, exploit decay, defense in depth, and player scripts. |
| Scripts | Files on the rig that end in `.sh`. One command on each tick, 1 GB of RAM while a script runs, and `&&` and `\|\|` as in a shell. |
| World features | Heist crews, SOC shifts, auctions of seized rigs, and a news feed with rumors. |
| Chat | IRC style: same-host chat, public channels, crew channels, and private messages. |
| Chat security | Encrypted traffic, sanitized text, rate limits, ignore, block, and moderator tools. No end-to-end encryption. |
| Identity | A key pair is the account. A recovery code can replace a lost key. |
| Connection | A TTYmud client and a TTYmud server that use the Noise protocol. With the `ssh` feature, a normal SSH client can also connect. |
| Linked cities | Operators make links between servers in `links.ini`. One linked channel and the headlines go through a link. Handles, credits, and files stay in their city. |
| Interface | Adaptive layers: line mode for all terminals, and a HUD when the terminal can show it. |
| Code | A Cargo workspace with two binaries: `ttymud` and `ttymud-server`. |
| Text model | A small `ttymud-text` crate holds styles, spans, and messages. |
| Look and list | `look` describes the host in words. `ls` gives a short list. `cd` works like `go`, and `cd ..` goes back. |
| Offline mode | `ttymud-server play` lets one player walk Port Ostra without a network. It uses real time, as the server does. It saves nothing. |
| Programs | A program acts at once and costs cycles. The rig gets 1 cycle on each tick. |
| Runs | Each run is a copy of the target network. Crewmates on the same job share one copy. |
| Disconnect | A disconnect during a run counts as a jack out. |
| Home host | It is private for each handle. A handle that connects again starts there. |
| Storage | Plain text files in the data directory. A storage thread writes them. |
| Noise | Noise raises the trace at once. A working Sentry doubles it. The trace does not increase while you wait. |
| ICE | Sentry, Warden, Hound, Scorch, Tarpit, and Lockjaw, and honeypot files. |
| Job goals | Theft, wipe, or plant. A wipe deletes a record with `rm`, and a plant puts a false file on a host with `upload`. |
| RAM | A new rig has 8 GB. A starter kit fills it with five programs. `load` and `unload` work only at the home host. |
| First contract | The fixer Pell offers it to each new handle. Its steps and hints come from the content file. |
| Repair | The `repair` command at the home host. It is free up to 50 integrity. Above 50, each point costs 2 credits. |
| Job board | Each fixer posts 3 jobs each day. All players see the same jobs. One contract at a time, with no time limit. |
| Reputation | Each clean contract adds 1 reputation with its fixer. Jobs against bigger budgets need more reputation. |
| Targets | The server makes each target from the world seed: an edge, a LAN, and a core with 5 to 8 hosts. A heist target also has a vault. |
| Access | Guest, user, and admin. Stolen credentials raise the access quietly. The `exploit` program raises it with noise. |
| Money | Credits for each handle. Contracts and the fence pay. Shops sell programs, RAM, and CPUs. |
| World seed | The file `world.ini` keeps it. Targets and boards stay the same after a restart. |
| Hosting | A public GitHub repository. The default branch is `main`. |
| Documents | ASD-STE100 Simplified Technical English. A test checks the basic rules. |

## Documents

| Document | Contents |
| --- | --- |
| [gameplay.md](gameplay.md) | Hosts, commands, pipelines, danger, ICE, programs, roles, and progression. |
| [world.md](world.md) | Zones, targets, corporations, the SOC, exploit decay, news, economy, crews, PvP, and linked cities. |
| [interface.md](interface.md) | Client layers, terminal support, accessibility, and the first run. |
| [security.md](security.md) | Threats, transport, identity, chat safety, and privacy. |
| [architecture.md](architecture.md) | Crates, binaries, server threads, storage, content, protocol, and tests. |
