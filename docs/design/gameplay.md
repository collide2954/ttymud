# Gameplay

This document gives the rules of play: hosts, commands, pipelines, danger, ICE, programs, roles, and progression. The numbers are first values. Tests and play will change them.

## Hosts

A host is a computer in the game world. A host is the TTYmud equivalent of a MUD room. Each host has these parts:

- A name and an address, for example `mail-gw` and `10.4.0.12`.
- A description.
- Services on ports, for example `ssh` on port 22. A service is a way into the host.
- Routes to other hosts. A route is the TTYmud equivalent of an exit.
- Files: data, credentials, logs, and lore. Files are the items of the host.
- ICE, NPCs, and other players.

A handle has a privilege level on each host. A higher privilege level lets the handle do more on the host.

| Privilege | What it lets you do |
| --- | --- |
| guest | See the public services. Read the public files. |
| user | Read most files. Run local recon. |
| admin | Read the logs. Stop sensors. Add accounts. |
| root | Install a backdoor. Delete all logs. Use the host as a proxy. |

## Movement

To move to a host, connect to one of its services. A service can ask for credentials, and ICE can guard it. To get past ICE, crack it, go around it, or use stolen credentials.

Each connection adds a hop to your connection chain. The `back` command moves you back one hop. If you go to a host that is already on your chain, the chain becomes shorter. A trace must follow your chain back to your rig. A long chain makes a trace slower, but each hop adds latency to your actions.

In a hub, movement is free and safe. Use `go <place>` or `cd <place>`. You can type the start of a name if no other route starts the same way. For example, `go night` moves you to `night-market`.

## Commands

Commands look like shell commands. Many commands have a shell name and a MUD name. Both names always work. The `look` command describes the host in words for MUD players. The `ls` command gives a short list for shell users.

| Command | Other names | Action |
| --- | --- | --- |
| `look` | `l` | Show the name and the description of the host, its routes, and its files. |
| `ls` | | List the routes and files here. Each route ends in `/`. |
| `go <place>` | `cd`, `connect`, `ssh` | Move to a host through a route. |
| `back` | `exit`, `cd ..` | Move back one hop. |
| `read <file>` | `cat` | Show the text of a file here or on your rig. |
| `download <file>` | `get` | Copy a file from this host to your rig. |
| `inventory` | `inv`, `i` | Show the files on your rig. |
| `grep <pattern>` | | Keep only the items that contain the pattern. |
| `help [command]` | `man` | Show help for all commands or for one command. |
| `?` | | Show the commands that you can use here. |
| `status` | `st` | Show trace, integrity, cycles, and RAM. Heat comes in M4. |
| `say <text>` | `'` | Talk to all players on the same host. |
| `jackout` | | Disconnect from the target immediately. |
| `repair` | | Restore all integrity. This command works only at your home host. |
| `contract [drop]` | `job` | Show your contract and its next step, or stop the contract. |

The programs are also commands. The [programs](#programs) section gives them.

If a command has an error, the game tells you the cause. It also gives the most likely correct command:

    > crak sentry
    Unknown command: crak. Did you mean crack?

In edit mode, the Tab key completes commands, routes, files, and help topics. Later, it will also complete handles.

## Pipelines

A pipeline joins commands with the pipe character (`|`). The output of each command goes into the next command. Examples:

    probe sentry | crack
    scan | probe
    ls | grep .db | download

Only the last command in a pipeline shows its output. Each other command gives its items to the next command.

A single command always works. A pipeline is a combo for skilled players. A pipeline has these advantages:

- It costs fewer cycles, because its commands share one setup cost.
- It makes less noise, because it opens fewer connections.
- It can give a bonus. For example, `crack` hits the weak point that `probe` found in the same pipeline.

The server checks a pipeline before it runs the pipeline. If the output of a command does not fit the next command, the server stops the pipeline and tells you why. A stopped pipeline costs nothing.

    > ls | crack
    crack needs ICE, but ls gives routes and files.

Each command gives one or more kinds of items: routes, files, lines, or ICE. The `grep` command gives the same kinds of items that it gets.

## Danger

Three values show how much danger you are in.

**Trace** is a value from 0% to 100% for the current run. Noise from your actions increases the trace. A working Sentry on the host doubles the trace from noise. Some ICE attacks also increase the trace. The `scrub` program deletes log lines and decreases the trace. When the trace gets to 100%, the target traces you.

The trace does not increase while you wait. The pressure comes from ICE. Later, it also comes from the SOC.

**Integrity** is the health of your rig, from 100 to 0. Black ICE attacks decrease integrity. The `patch` program repairs some integrity during a run. When integrity gets to 0, your rig crashes.

At your home host, the `repair` command restores all integrity. In version 0.1, a repair is free. Later, a repair costs credits.

**Heat** shows how much the police know about your handle, from 0 to 5. Heat comes in M4. Each trace adds heat. The security level of the host where the trace gets to 100% sets the quantity of heat. The [world](world.md) document gives the security levels.

| Security level | Heat for one trace |
| --- | --- |
| Low | 1 |
| Medium | 2 |
| High | 3 |
| Top | 5 |

Heat decreases by 1 for each 24 hours of real time without a trace. A cleaner NPC can also decrease heat for credits. At heat 5, your handle burns.

Heat also has effects before a burn:

- At heat 3 or more, the SOC of a target recognizes you faster.
- At heat 4 or more, fixers ask for a higher fee, and some fixers do not give you contracts.

## How a run ends

| End | Cause | Result |
| --- | --- | --- |
| Clean exit | You leave through the host where you came in. | You keep the loot. You get no heat. |
| Jack out | You use `jackout`. | You disconnect immediately and keep the loot. Your rig loses 20 integrity. If the trace is 75% or more, you get 1 heat. |
| Traced | The trace gets to 100%. | You disconnect and lose the loot of this run. You get heat. |
| Crash | The integrity gets to 0. | You disconnect and lose the loot of this run. You must repair the rig. You get no heat. |

When you disconnect, you go back to the hub host where the run started. If the shock of a jack out takes the last integrity, your rig crashes. A rig without integrity cannot connect to a target. In version 0.1, no end of a run gives heat, because heat comes in M4.

Each run is a private copy of the target network. Other players do not see you in a target, and your actions do not change their copy. Crews will share a copy in M9.

## ICE

ICE is hostile security software. Each ICE has a type, a version, an integrity value, and a set of actions.

| Type | Function |
| --- | --- |
| Sentry | A sensor. It sees noise and increases the trace. |
| Warden | A firewall. It blocks a service or a route. |
| Hound | A tracker. It attacks the trace directly. |
| Scorch | Black ICE. It attacks the integrity of your rig. |
| Tarpit | It makes your actions slower. |
| Lockjaw | It freezes one of your programs for some ticks. |
| Honeypot | A false host or file. If you touch it, the trace increases quickly. The `probe` program can find it. |

Before each action, an ICE selects the action from its set and shows it. This message is a telegraph:

    hound starts to compile TRACE BURST (2 ticks).

The telegraph gives you time to answer. You can crack the Hound before it acts. You can add a proxy so that the attack does less. You can also scrub logs after the attack. Each ICE has a different set of actions and a different order. As a result, a fight is fair but not predictable.

### ICE in version 0.1

Version 0.1 has four types of ICE. Tarpit, Lockjaw, and Honeypot come later. Each version above 1 adds 10 integrity.

| Type | Integrity | Behavior | Actions |
| --- | --- | --- | --- |
| Sentry | 20 | Noise on its host starts an alert. | ALERT (3 ticks) wakes the Hounds of the network. If no Hound sleeps, the trace increases by 10%. |
| Warden | 40 | It blocks one route until it breaks. | None. |
| Hound | 30 | An alert wakes it. Then it follows you from host to host. | TRACE BURST (2 ticks) adds 8% trace. DEEP TRACE (4 ticks) adds 20% trace. |
| Scorch | 30 | Noise on its host wakes it. It attacks only while you are on its host. | BURN (2 ticks) does 20 damage. SPIKE (1 tick) does 8 damage. |

An awake ICE selects its next action at random from its set. The seed of the server sets the random numbers. If you leave the host of a Scorch, its attack stops.

You do not know the ICE on a host before you scan the host. The `look` command shows only the number of ICE programs that you do not know. A telegraph from an ICE that you do not know starts with "Unknown ICE".

When the integrity of an ICE gets to 0, the ICE breaks. A broken ICE does not act. A broken Warden opens its route.

## Programs

A program is software that you run to do an action. Each program has these properties:

- A name and a version, for example `crack v2.1`. A higher version is stronger.
- A RAM cost in GB. A program must be in RAM before you can run it.
- A cycle cost. The CPU of your rig makes new cycles on each tick.
- A noise value. Noise increases the trace.
- Mods. A mod changes a program, for example less noise for more cycles.

Before a run, load programs into RAM in your safehouse. Your RAM is limited, so you must select the programs. You can also load and unload programs during a run, but each change takes ticks.

| Group | Programs |
| --- | --- |
| Recon | `scan`, `probe`, `sniff` |
| Intrusion | `crack`, `exploit`, `brute`, `inject` |
| Stealth | `scrub`, `spoof`, `mask`, `proxy` |
| Defense | `firewall`, `patch`, `decoy` |
| Social | `phish`, `pretext`, `bribe` |
| Data | `decrypt`, `exfil`, `backdoor` |

Built-in commands, for example `look` and `say`, do not use RAM or cycles.

### Programs in version 0.1

In version 0.1, each rig has 8 GB of RAM and these five programs. Starter kits and the `load` command come in M4.

| Program | RAM | Cycles | Noise | Action |
| --- | --- | --- | --- | --- |
| `scan` | 1 GB | 1 | 1 | It shows the ICE on this host. After the scan, you know the names of the ICE. |
| `probe` | 1 GB | 1 | 1 | It shows the weak point of an ICE. |
| `crack` | 2 GB | 2 | 2 | It does 15 damage to an ICE. After a probe in the same pipeline, it does 30 damage. |
| `scrub` | 2 GB | 2 | 0 | It decreases the trace by 15%. |
| `patch` | 2 GB | 2 | 0 | It repairs 20 integrity. |

A rig has a maximum of 4 cycles. It gets 1 cycle on each tick. The programs work only in a target network. In a target network, a download also makes 2 noise.

A command line that runs programs costs 1 more cycle for the setup. If the programs make noise, the connection adds 1 more noise. As a result, `probe sentry | crack` costs 4 cycles and makes 4 noise. The same two commands on two lines cost 5 cycles and make 5 noise.

If you do not have enough cycles, the command does not run. It costs nothing.

## Roles

TTYmud has no classes. The programs in your RAM set your role. A new handle selects one starter kit. The kit gives a rig and the programs for one role. After that, the player can change role at any time.

| Kit | Role | Starter programs |
| --- | --- | --- |
| Breaker | Breaks ICE. Pulls ICE attacks away from the crew. | `crack`, `firewall`, `probe` |
| Ghost | Stays hidden. Scrubs logs to decrease the trace of other crew members. | `scrub`, `mask`, `spoof` |
| Phreak | Controls routes and proxies. Finds ways around ICE and air gaps. | `scan`, `proxy`, `sniff` |
| Face | Tricks NPCs with social engineering. Gets credentials and distracts analysts. | `phish`, `pretext`, `probe` |

## Progression

There are no levels. A handle gets stronger in three ways.

1. **Rig.** Better hardware. The CPU gives more cycles. RAM holds more programs. The NIC makes downloads faster. Storage holds more loot.
2. **Software.** Higher program versions and better mods. You find, buy, or compile programs. A patch increases the minor version, for example from v2.1 to v2.2. A blueprint and parts let you compile a new major version, for example v3.0.
3. **Reputation.** Each faction keeps a reputation value for your handle. Higher reputation opens harder targets, better contracts, and better vendors.

## Burn and legacy

When the heat gets to 5, the handle burns. Then these events occur:

- The handle goes on the burn list of the server. Nobody can use the name again on that server.
- The police seize the rig and the items. The items go to the evidence auction.
- The handle loses its credits, reputation, and contacts.

The player keeps these things:

- **Know-how.** The account keeps all blueprints that the handle learned. A new handle can compile these programs again, so the rebuild is faster.
- **The offshore stash.** Before a burn, a handle can put credits and a few items into an offshore stash. The stash has a limit, and each deposit has a fee. After a burn, the next handle gets the stash.

Then the player makes a new handle and selects a starter kit.
