# World

This document tells how the world of TTYmud works. It gives the zones, the targets, the corporations, and the systems that change the world.

## Zones

The world has three types of zones.

| Zone | Made by | Danger |
| --- | --- | --- |
| Hub | Authors, by hand | None. No trace and no PvP. |
| Target | The server, from a seed | Trace, ICE, and analysts. |
| Darknet | Authors, by hand | Lawless. Other players can attack you. |

## Hubs

A hub is a safe place that does not change. Players learn the hubs and meet there. Port Ostra, the default city, has these hubs:

- **The Relay.** The public place where players meet. New players start here.
- **The Night Market.** Shops for hardware and programs.
- **The Back Room.** A bar where fixers give contracts and tell rumors.
- **The Safehouse.** The home of your handle. Load programs, repair your rig, and use your stash here. The safehouse is private, so other players do not see you there.
- **The Evidence Locker.** The police auction for seized rigs.
- **The Newsstand.** The news feed and the burn list.

Hubs are content files. A server operator can change the hubs or add new hubs.

## Targets

A target is the network of a corporation. The server makes a new target network for each job on the job board. It uses the server seed, the day, the fixer, and the number of the job. As a result, two servers with different seeds have different targets. The same job also has the same network after a restart.

A target for a job has an edge, a LAN, and a core. A target for a heist also has a vault. The first contract of a new handle uses a network that an author made by hand: the dental office Bright Smile Dental.

Each target has layers. This is defense in depth.

| Layer | Hosts |
| --- | --- |
| Edge | Web servers, mail gateways, and VPN gateways |
| LAN | Workstations, file shares, and printers |
| Core | Domain controllers, databases, and backups |
| Vault | Air-gapped servers with the most valuable data |

Deeper layers have stronger ICE, more analysts, and better loot.

The vault has no network route. To get in, a crew must make a bridge. Each heist target has two bridges:

- A maintenance laptop connects to the vault at times. A crew can hack the laptop and wait for its next connection.
- A Face tricks an employee, who then puts a USB drive into a vault computer. Or, in a different target, a Phreak finds an old modem line that the corporation forgot.

The [gameplay](gameplay.md#heists) document gives the rules.

### Generated targets in version 0.1

The server makes each target from the corp sections and the template sections of the content file:

- 1 or 2 edge hosts. Each edge host is a way in from your home host, and each one has a Sentry.
- 2 or 3 LAN hosts, and a Hound on one of them. Above low security, a second LAN host has a Sentry.
- 2 or 3 core hosts, each with a Scorch. A Warden guards one route from the LAN into the core.
- ICE of version 1, 2, 3, or 4 for the security levels low, medium, high, and top.
- The data files of the corporation on the LAN hosts and the core hosts. The goal of the job is on a core host.
- Two credentials files. The first gives user access, and the second gives admin access.

For a heist, the server adds these parts to the target:

- A maintenance laptop on the LAN, from a template with the key `laptop = yes`.
- A vault from a vault template, with a Sentry and a Scorch. The goal of the heist moves into the vault.
- An IT admin on a LAN host, or an old modem line on a core host.

The [gameplay](gameplay.md#access-in-version-01) document tells how access works.

## Security levels

Each host has a security level: low, medium, high, or top. The layer of the host and the security budget of the corporation set the level.

| Budget | Edge | LAN | Core | Vault |
| --- | --- | --- | --- | --- |
| Low | Low | Low | Medium | High |
| Medium | Low | Medium | High | High |
| High | Medium | High | High | Top |
| Top | High | High | Top | Top |

The security level sets the strength of the ICE and the heat for a trace.

In version 0.1, each host of a generated target gets its security level from this table. The level sets the heat for a trace and the version of the ICE. A network that an author makes by hand has one security level for all its hosts. Bright Smile Dental has the security level low.

## Contracts

Fixers give contracts. A contract has a target, an objective, a payment, and a time limit. These are examples of objectives:

- Steal a file.
- Change a record, for example delete a debt.
- Put false evidence on a host.
- Stop a service for some time.
- Find the person behind an attack.

Higher reputation with a fixer opens contracts against deeper layers and stronger corporations.

In version 0.1, the only objective is to steal a file, and a contract has no time limit. Each clean contract adds 1 reputation with its fixer. The [gameplay](gameplay.md#jobs) document gives the pay and the necessary reputation for each budget.

## Corporations

Each corporation has a business, a security budget, a software stack, and a SOC. Port Ostra has these corporations:

| Corporation | Business | Security budget |
| --- | --- | --- |
| Mosaic Media | News | Low |
| Kestrel Health | Hospitals and medical records | Medium |
| Orrery Logistics | The port and freight | Medium |
| Arcadia Bank | Accounts and loans | High |
| Civic Grid | Power and water for the city | High |
| Greyward Defense | Weapons and private security | Top |

In Port Ostra, two fixers work in the Back Room. Pell works against Mosaic Media, Kestrel Health, and Orrery Logistics. Wren works against Mosaic Media, Arcadia Bank, Civic Grid, and Greyward Defense. Wren does not work with handles at heat 4 or more.

## The SOC

Each corporation has a security operations center (SOC). Analysts are NPCs who work in the SOC. They work in shifts on the server clock. The day shift has more analysts. The night shift has fewer analysts, but the automatic ICE is stronger.

Analysts act like people. They read logs and lock stolen accounts. Later, they will also reset passwords and cut routes between hosts. Each analyst has a habit: careful, lazy, or strict. The game shows their actions as log lines:

    [SOC] Analyst m.ortiz reads the logs of mail-gw and finds your trail. Trace: 12% -> 20%.

A Face can trick an analyst with `pretext`. A Ghost can hide from an analyst with `mask`. The [gameplay](gameplay.md#the-soc) document gives the rules.

Each corporation has an alert level: green, yellow, orange, or red. Noise and traces from all players increase the alert level. The alert level decreases over time. A higher alert level adds analysts and ICE. For example, if a different player made noise at Arcadia Bank one hour ago, Arcadia Bank can still be on alert when you arrive.

## Exploit decay

An exploit attacks a weakness in a software family, for example version 3 of a mail server. Each corporation keeps an exposure value for each family. Each use of `exploit` against the family increases the exposure. Later, a noisy use or a trace will increase it more.

When the exposure gets to the limit of the corporation, the corporation patches the family. After the patch, `exploit` does not work on that family at that corporation. The news feed reports the patch. A corporation with a higher security budget has a lower limit, so it patches faster. Its patch also stays for a longer time.

When the patch gets old, somebody finds a new hole in the family. Then the exploit works again, and the exposure starts at 0. Corporations with a low budget patch slowly, and their patches stay for a short time. The [gameplay](gameplay.md#exploit-decay) document gives the limits.

Later, new exploits will come from loot, from research, and from vendors. A player who uses a new exploit carefully can use it for a long time. As a result, the best tactics change all the time, and a secret exploit has a real value.

## News feed and rumors

The news feed reports events in the city: breaches, traces, burns, patches, and alert levels. Later, it will also report bounties and auctions. Use the `news` command to read it. The HUD also shows the latest headline.

    2026-03-14 22:13 Kestrel Health loses patients.db in a breach.

The fixers in the Back Room tell rumors. A rumor gives a hint about a target. Some rumors are true, some are old, and some are false. A fixer gives more true rumors to a handle with a higher reputation. Later, other NPCs will also tell rumors.

    Pell: I hear that the hole in Gatehouse VPN 4 at Orrery Logistics is still open.

## Evidence auctions

When a handle burns, the police seize its rig and items. The items go to the Evidence Locker. For some days of real time, players can bid credits on the items. Each item shows its history, for example "seized from the handle nyx". The game deletes the items that nobody buys.

This system keeps good items in the game and removes credits from the game.

## Economy

The currency is the credit (cr).

| Sources of credits | Sinks for credits |
| --- | --- |
| Contract payments | Hardware and programs |
| Sales of stolen data | Rig repairs |
| Sales of items | Cleaners who decrease heat |
| Sales at auctions | Fees for the offshore stash |
| | Compile costs |
| | Bribes |
| | Auction bids |

The price of stolen data changes with supply and demand. If many players sell medical records, the price of medical records goes down.

In version 0.2, credits come from contracts and from the fence. The fence pays less for a file name after each sale, and the price comes back with time. Credits go to repairs, programs, blueprints, compiles, hardware, and bids at the police auction. The [gameplay](gameplay.md#money) document gives the prices.

## Crews and heists

A crew is a group of up to 4 handles. A crew has a name, a private channel, and a reputation. Crewmates on the same job share one copy of the target network.

A heist is a large contract for a crew of 2 to 4. It has more than one stage, and each role can help in a different stage. In a vault heist, for example, each role has a task:

- The Face tricks an employee, or the Phreak uses an old modem line, to make a bridge to the vault.
- The Breaker holds the ICE with `decoy`.
- The Ghost keeps the traces low with `scrub <handle>`.

Each player has a separate trace. Noise from one player increases the trace of that player and the alert level of the target. As a result, one player cannot get all of the crew traced. A Ghost can scrub the trace of a different player. A Breaker can pull ICE attacks to the Breaker. The [gameplay](gameplay.md#crews) document gives the rules.

## PvP

TTYmud is a PvE game first. PvP occurs only when the player agrees to it.

- **Duels and matches.** Two players or two crews agree to a match on a closed network, for example capture the flag. A match gives no heat and causes no loss.
- **Darknet zones.** These zones are lawless. Before you go in, the game asks you to agree. In a darknet zone, other players can steal your loot and report you to the police. Each report adds 1 heat. Darknet zones have better prices and rare items.

A player cannot burn a different player directly. Reports can add a maximum of 1 heat to a handle in each 24 hours.
