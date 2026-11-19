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

A target is the network of a corporation. The server makes a new target network for each contract. It uses the server seed, the corporation, and the contract number. As a result, two servers with different seeds have different targets.

In version 0.1, the target of the first contract is a network in the content file: the dental office Bright Smile Dental. The target generator comes in M5.

Each target has layers. This is defense in depth.

| Layer | Hosts |
| --- | --- |
| Edge | Web servers, mail gateways, and VPN gateways |
| LAN | Workstations, file shares, and printers |
| Core | Domain controllers, databases, and backups |
| Vault | Air-gapped servers with the most valuable data |

Deeper layers have stronger ICE, more analysts, and better loot.

The vault has no network route. To get in, a crew must make a bridge. These are some possible bridges:

- A maintenance laptop connects to the vault at times that the server selects. A crew can hack the laptop and wait for its next connection.
- A Face tricks an employee, who then puts a USB drive into a vault computer.
- A Phreak finds an old modem line that the corporation forgot.

## Security levels

Each host has a security level: low, medium, high, or top. The layer of the host and the security budget of the corporation set the level.

| Budget | Edge | LAN | Core | Vault |
| --- | --- | --- | --- | --- |
| Low | Low | Low | Medium | High |
| Medium | Low | Medium | High | High |
| High | Medium | High | High | Top |
| Top | High | High | Top | Top |

The security level sets the strength of the ICE and the heat for a trace.

In version 0.1, each target network has one security level, and the content file sets it. The security level sets only the heat for a trace. Bright Smile Dental has the security level low.

## Contracts

Fixers give contracts. A contract has a target, an objective, a payment, and a time limit. These are examples of objectives:

- Steal a file.
- Change a record, for example delete a debt.
- Put false evidence on a host.
- Stop a service for some time.
- Find the person behind an attack.

Higher reputation with a fixer opens contracts against deeper layers and stronger corporations.

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

## The SOC

Each corporation has a security operations center (SOC). Analysts are NPCs who work in the SOC. They work in shifts on the server clock. The day shift has more analysts. The night shift has fewer analysts, but the automatic ICE is stronger.

Analysts act like people. They read logs, lock accounts, reset passwords, and cut routes between hosts. Each analyst has traits, for example careful, lazy, or strict. The game shows their actions as log lines:

    [SOC] Analyst m.ortiz reads the auth log on mail-gw.

A Face can trick an analyst. A Ghost can hide from an analyst.

Each corporation has an alert level: green, yellow, orange, or red. Noise and traces from all players increase the alert level. The alert level decreases over time. A higher alert level adds analysts and ICE. For example, if a different player made noise at Arcadia Bank one hour ago, Arcadia Bank can still be on alert when you arrive.

## Exploit decay

An exploit attacks a weakness in a software family, for example version 4 of a mail server. Each corporation keeps an exposure value for each exploit. Each use of the exploit against the corporation increases the exposure. A noisy use or a trace increases it more.

When the exposure gets to the limit of the corporation, the corporation patches the weakness. After the patch, the exploit does not work against that corporation on that server. The news feed reports the patch. A corporation with a higher security budget has a lower limit, so it patches faster.

New exploits come from loot, from research, and from vendors. A player who uses a new exploit carefully can use it for a long time. As a result, the best tactics change all the time, and a secret exploit has a real value.

Sometimes a corporation makes an error and brings back an old weakness. Corporations with a low budget patch slowly.

## News feed and rumors

The news feed reports events in the city: breaches, traces, burns, patches, alert levels, bounties, and auctions. Use the `news` command to read it. The HUD also shows the latest headline.

    Kestrel Health confirms the theft of 40,000 patient records.

Fixers and NPCs in the Back Room tell rumors. A rumor gives a hint about a weak target. Some rumors are true, some are old, and some are false. A source with a higher reputation gives better rumors.

    "The night admin at Orrery leaves the VPN open on Fridays."

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

## Crews and heists

A crew is a group of players. A crew has a name, a private channel, and a reputation.

A heist is a large contract for a crew. It has more than one stage, and each stage is for a different role. In a vault heist, for example, each role has a task:

- The Face tricks an employee.
- The Phreak makes the bridge to the vault.
- The Breaker holds the ICE.
- The Ghost keeps the traces low.

Each player has a separate trace. Noise from one player increases the trace of that player and the alert level of the target. As a result, one player cannot get all of the crew traced. A Ghost can scrub the trace of a different player. A Breaker can pull ICE attacks to the Breaker.

## PvP

TTYmud is a PvE game first. PvP occurs only when the player agrees to it.

- **Duels and matches.** Two players or two crews agree to a match on a closed network, for example capture the flag. A match gives no heat and causes no loss.
- **Darknet zones.** These zones are lawless. Before you go in, the game asks you to agree. In a darknet zone, other players can steal your loot and report you to the police. Each report adds 1 heat. Darknet zones have better prices and rare items.

A player cannot burn a different player directly. Reports can add a maximum of 1 heat to a handle in each 24 hours.
