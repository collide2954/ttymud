# Gameplay

This document gives the rules of play: hosts, access, commands, pipelines, danger, ICE, programs, roles, money, jobs, and progression. The numbers are first values. Tests and play will change them.

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

### Access in version 0.1

In version 0.1, there are three access levels: guest, user, and admin. You arrive on each host of a target as guest. In a generated target, the data files on LAN hosts need user access, and all files on core hosts need admin access. The networks that an author makes by hand have no locked files.

To raise your access, read the credentials that the company left in its network. A credentials file on an edge host gives user access in the whole network. A second credentials file on a LAN host needs user access, and it gives admin access. You can read a credentials file on its host or from your rig. Your access goes back to guest when the run ends.

The `exploit` program raises your access on one host by one level. It makes much noise, so use it when you cannot find the credentials. The `look` and `ls` commands mark each file that needs more access, for example `payroll.db (needs user)`.

## Movement

To move to a host, connect to one of its services. A service can ask for credentials, and ICE can guard it. To get past ICE, crack it, go around it, or use stolen credentials.

Each connection adds a hop to your connection chain. The `back` command moves you back one hop. If you go to a host that is already on your chain, the chain becomes shorter. A trace must follow your chain back to your rig. A long chain makes a trace slower, but each hop adds latency to your actions.

In a hub, movement is free and safe. Use `go <place>` or `cd <place>`. You can type the start of a name if no other route starts the same way. For example, `go night` moves you to `night-market`.

## Home host

The home host is the hub host where your rig is. In Port Ostra, it is the safehouse. At your home host, you can repair your rig, change the programs in RAM, and use your stash.

The home host is private. Other players do not see you there, and they do not see your messages. When you connect again, your handle starts at the home host.

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
| `upload <file>` | | Copy a file from your rig to this host in a target network. |
| `inventory` | `inv`, `i` | Show the files on your rig. |
| `grep <pattern>` | | Keep only the items that contain the pattern. |
| `help [command]` | `man` | Show help for all commands or for one command. |
| `?` | | Show the commands that you can use here. |
| `status` | `st` | Show trace, access, heat, credits, integrity, cycles, RAM, and the programs in RAM. |
| `say <text>` | `'` | Talk to all players on the same host. |
| `jackout` | | Disconnect from the target immediately. |
| `repair [all]` | | Restore integrity at your home host. A repair up to 50 integrity is free. `repair all` also pays for the points above 50. |
| `load <program>` | | Put a program into RAM. This command works only at your home host. |
| `unload <program>` | | Remove a program from RAM. This command works only at your home host. |
| `compile [program] [quiet\|plain]` | | Show your blueprints, compile the next version of a program, or add or remove the quiet mod. This command works only at your home host. |
| `stash [file]` | | Show your stash. At your home host, move a file from your rig into the stash. |
| `unstash <file>` | | Move a file from your stash to your rig. This command works only at your home host. |
| `contract [drop]` | `job` | Show your contract and its next step, or stop the contract. |
| `jobs` | | Show the jobs of the fixers on this host. |
| `accept <number>` | | Take a job from a fixer on this host. |
| `shop` | | Show what the shop on this host sells. |
| `buy <item>` | | Buy a program, a blueprint, more RAM, or a faster CPU at a shop. |
| `sell <file>` | | Sell stolen data to the fence of a shop. Without a file, show the prices of your files. |
| `lots` | | Show the lots of the police auction. This command works only at the auction host. |
| `bid <lot> <credits>` | | Bid on a lot of the police auction. This command works only at the auction host. |
| `echo <text> [> script]` | | Show the text. With `>` or `>>`, write it to a script on your rig. |
| `rm <file>` | | Delete a script from your rig, or a file on this host in a target network. |
| `sh <script> [words]` | | Run a script from your rig. The [scripts](#scripts) section gives the rules. |
| `ps` | | Show the script that runs on your rig. |
| `kill` | | Stop the script that runs on your rig. |
| `wait [ticks]` | | In a script, wait for 1 to 30 ticks. |

The programs are also commands. The [programs](#programs) section gives them.

If a command has an error, the game tells you the cause. It also gives the most likely correct command:

    > crak sentry
    Unknown command: crak. Did you mean crack?

In edit mode, the Tab key completes commands, routes, files, scripts, programs, ICE, analysts, handles, crewmates, channels, and help topics.

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

Each command gives one or more kinds of items: routes, files, lines, or ICE. The `grep` command gives the same kinds of items that it gets. If nothing matches, `grep` fails, and the pipeline stops.

## Scripts

A script is a file on the rig with one command line on each line. Its name ends in `.sh`. A player writes a script with `echo <text> > <script>` and `echo <text> >> <script>`. The player runs it with `sh <script> [words]`, and the words go into `$1` to `$9`.

| Rule | Value |
| --- | --- |
| Speed | One command on each tick. A command can be a pipeline. |
| Cycles | A command waits until the rig has its cycles. A command that needs more cycles than the CPU holds fails. |
| RAM | 1 GB while the script runs. Only one script runs at a time. |
| Size | 20 lines in a script, and 10 scripts on a rig. |
| Wait | `wait [ticks]` lets 1 to 30 ticks go by. |
| Notes | A line that starts with `#`. |

A line can join pipelines with `&&` and `||`, as in a shell. `&&` runs the next pipeline only if the last pipeline worked, and `||` runs it only if the last pipeline failed. A pipeline fails when one of its commands gives an error. As in a shell with `set -e`, the script stops when the last pipeline of a line runs and fails. At the prompt, `&&` and `||` give an error.

Before a script starts, the server checks each line: the syntax, the command names, and the pipelines. A script cannot use the commands that send messages to other handles or to the moderators: `say`, `tell`, `reply`, `report`, `crew`, `duel`, and `match`. It also cannot use the moderator commands, the account commands, or `sh`. As a result, a script cannot fill the chat with messages, and it cannot start a loop.

A crash, a trace, a burn, or a disconnect stops the script. The `kill` command also stops it, and `ps` shows its line. The server does not save a running script. After a restart, the script file is still on the rig, but it does not run.

## Danger

Three values show how much danger you are in.

**Trace** is a value from 0% to 100% for the current run. Noise from your actions increases the trace. A working Sentry on the host doubles the trace from noise. Some ICE attacks also increase the trace. The `scrub` program deletes log lines and decreases the trace. When the trace gets to 100%, the target traces you.

The trace does not increase while you wait. The pressure comes from ICE and from the analysts of the SOC.

**Integrity** is the health of your rig, from 100 to 0. Black ICE attacks decrease integrity. The `patch` program repairs some integrity during a run. When integrity gets to 0, your rig crashes.

At your home host, the `repair` command restores integrity up to 50 for free, and it shows the price of a full repair. The `repair all` command also pays 2 credits for each point above 50. If you do not have enough credits, the repair stops at the integrity that you can pay for.

**Heat** shows how much the police know about your handle, from 0 to 5. Each trace adds heat. A trace in the first contract of a handle adds no heat. The security level of the host where the trace gets to 100% sets the quantity of heat. In a network that an author makes by hand, all hosts have the security level of the network. The [world](world.md) document gives the security levels.

| Security level | Heat for one trace |
| --- | --- |
| Low | 1 |
| Medium | 2 |
| High | 3 |
| Top | 5 |

Heat decreases by 1 for each 24 hours of real time without new heat. New heat starts the 24 hours again. Heat also decreases while you are offline. Later, a cleaner NPC will decrease heat for credits. At heat 5, your handle burns.

Heat also has effects before a burn:

- At heat 3 or more, the SOC of a target recognizes you faster. In version 0.1, the ALERT of a Sentry takes 2 ticks, not 3.
- At heat 4 or more, fixers keep 25% of the pay as a fee. Some fixers do not give jobs to handles with too much heat. In Port Ostra, Wren does not work with handles at heat 4 or more.

## How a run ends

| End | Cause | Result |
| --- | --- | --- |
| Clean exit | You leave through the host where you came in. | You keep the loot. You get no heat. If the goal file of your contract is on your rig, the contract is complete. |
| Jack out | You use `jackout`, or you disconnect from the server. | You disconnect immediately and keep the loot. Your rig loses 20 integrity. If the trace is 75% or more, you get 1 heat. |
| Traced | The trace gets to 100%. | You disconnect and lose the loot of this run. The security level of the target sets your heat. |
| Crash | The integrity gets to 0. | You disconnect and lose the loot of this run. You must repair the rig. You get no heat. |

When you disconnect, you go back to the hub host where the run started. If the shock of a jack out takes the last integrity, your rig crashes. A rig without integrity cannot connect to a target. If you disconnect from the server during a run, the game jacks you out. You get the shock, and you get heat if the trace is 75% or more.

Each run is a copy of the target network. Players outside your crew do not see you in a target, and your actions do not change their copy. Crewmates on the same job share one copy. The [group runs](#group-runs) section gives the rules.

## ICE

ICE is hostile security software. Each ICE has a type, a version, an integrity value, and a set of actions.

| Type | Function |
| --- | --- |
| Sentry | A sensor. It sees noise and increases the trace. |
| Warden | A firewall. It blocks a service or a route. |
| Hound | A tracker. It attacks the trace directly. |
| Scorch | Black ICE. It attacks the integrity of your rig. |
| Tarpit | It takes your cycles, so your actions are slower. |
| Lockjaw | It freezes one of your programs for some ticks. |
| Honeypot | A false file. If you touch it, the trace increases quickly. The `probe` program can find it. |

Before each action, an ICE selects the action from its set and shows it. This message is a telegraph:

    hound starts to compile TRACE BURST (2 ticks).

The telegraph gives you time to answer. You can crack the Hound before it acts. You can add a proxy so that the attack does less. You can also scrub logs after the attack. Each ICE has a different set of actions and a different order. As a result, a fight is fair but not predictable.

### ICE types

The game has six types of ICE, and honeypot files. Each version above 1 adds 10 integrity.

| Type | Integrity | Behavior | Actions |
| --- | --- | --- | --- |
| Sentry | 20 | It starts an alert when you enter its host or make noise there. | ALERT (3 ticks) wakes the Hounds of the network. If no Hound sleeps, the trace increases by 10%. |
| Warden | 40 | It blocks one route until it breaks. | None. |
| Hound | 30 | An alert wakes it. Then it follows you from host to host. | TRACE BURST (2 ticks) adds 8% trace. DEEP TRACE (4 ticks) adds 20% trace. |
| Scorch | 30 | Noise on its host wakes it. It attacks only while you are on its host. | BURN (2 ticks) does 20 damage. SPIKE (1 tick) does 8 damage. |
| Tarpit | 25 | The same as Scorch. | TAR (2 ticks) takes all your cycles. |
| Lockjaw | 30 | The same as Scorch. | BITE (2 ticks) freezes the program that you used last. For 6 ticks, you cannot run that program. |

An awake ICE selects its next action at random from its set. The seed of the server sets the random numbers. If you leave the host of a Scorch, a Tarpit, or a Lockjaw, its attack stops.

A target of a company with a medium or higher budget has a Lockjaw on one LAN host and a Tarpit on one core host.

Each target also has one honeypot: a bait file on a LAN host or a core host, with a name such as `passwords.txt`. If you read or download it, the trace increases by 30%. Type `probe` and the name of a file to find out if it is a honeypot. The `ls | probe` pipeline checks all files on the host.

You do not know the ICE on a host before you scan the host. The `look` command shows only the number of ICE programs that you do not know. A telegraph from an ICE that you do not know starts with "Unknown ICE".

When the integrity of an ICE gets to 0, the ICE breaks. A broken ICE does not act. A broken Warden opens its route.

## Alert levels

Each corporation has an alert level from 0 to 100 points. The points set the level:

| Points | Level |
| --- | --- |
| 0 to 24 | Green |
| 25 to 49 | Yellow |
| 50 to 74 | Orange |
| 75 to 100 | Red |

The actions of all players raise the points. Each point of noise in a network of the corporation adds 1 point. A trace adds 25 points, and a completed contract against the corporation adds 15 points. The points go down by 1 every 5 minutes. As a result, the change from red to green takes about 6 hours.

The alert level at the start of a run sets how difficult the run is. Each level above green adds 25% to the trace from noise, 1 analyst, and 1 version to each ICE. The maximum ICE version is 9. During a run, a change of the level changes only the trace from noise.

## The SOC

Each corporation has a SOC with a pool of 6 analysts. The world seed sets their names and their habits. The analysts work in shifts on the UTC clock:

| Shift | Time | Analysts | ICE |
| --- | --- | --- | --- |
| Day | 08:00 to 20:00 | 2 | Normal |
| Night | 20:00 to 08:00 | 1 | 1 version stronger |

Each alert level above green adds 1 analyst to the shift. The `scan` command shows the analysts on shift and their habits.

The noise of your actions goes into the logs of the host. A login with stolen credentials also leaves a line in the logs of the host where you read them. Each analyst reads the logs of one host at a time. Two ticks before the read, a telegraph gives the name of the host. The telegraph comes only when the host has logs to find, or at once when you leave logs on a host that an analyst watches. The habit of the analyst sets the host and the time between two reads:

| Habit | Host | Time between reads |
| --- | --- | --- |
| Careful | The host with the most noise in the logs. | 8 ticks |
| Strict | The host where you are. | 6 ticks |
| Lazy | A random host of the network. | 12 ticks |

If the analyst finds your logs, the trace increases by 5% and by the noise in the logs. Then the logs of that host are empty. If your access comes from stolen credentials, a careful or a strict analyst also locks the account. Your access in the network goes back to guest, but a shell from `exploit` stays open. The analyst also resets the stolen password. Until the next day of UTC time, those credentials do not work in that network.

A strict analyst who finds your noise can also cut one route from that host to a host that is deeper in the network. A telegraph comes 2 ticks before the cut. The cut stops movement to the deeper host until the copy of the network ends. The routes back to the entry stay open. Each analyst cuts one route at most in each copy.

Three programs work against the SOC:

- `scrub` deletes your logs on the host where you are.
- While `mask` is up, noise and logins do not go into the logs.
- `pretext <analyst>` tricks an analyst who is not strict. Then the analyst does not watch your run.

## Programs

A program is software that you run to do an action. Each program has these properties:

- A name and a version, for example `crack v2`. A higher version is stronger.
- A RAM cost in GB. A program must be in RAM before you can run it.
- A cycle cost. The CPU of your rig makes new cycles on each tick.
- A noise value. Noise increases the trace.
- Mods. A mod changes a program, for example less noise for more cycles.

Before a run, load programs into RAM at your home host. Your RAM is limited, so you must select the programs. You cannot change the programs in RAM during a run.

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

In version 0.1, a new rig has 8 GB of RAM, and its CPU holds 4 cycles. A shop sells more RAM and faster CPUs. Your starter kit fills the RAM with five programs. To load a different program, unload a program first. You can load only the programs that your account knows.

| Program | RAM | Cycles | Noise | Action |
| --- | --- | --- | --- | --- |
| `scan` | 1 GB | 1 | 1 | It shows the ICE and the software of this host, and the analysts on shift. After the scan, you know the names of the ICE. |
| `probe` | 1 GB | 1 | 1 | It shows the weak point of an ICE, or if a file is a honeypot. |
| `crack` | 2 GB | 2 | 2 | It does 15 damage to an ICE. After a probe in the same pipeline, it does 30 damage. |
| `scrub` | 2 GB | 2 | 0 | It decreases the trace by 15% and deletes your logs on this host. With a handle, it does this for a crewmate on your host. |
| `patch` | 2 GB | 2 | 0 | It repairs 20 integrity. |
| `firewall` | 2 GB | 2 | 0 | It stops the next ICE attack on your integrity. Only one firewall can be up. |
| `decoy` | 2 GB | 2 | 1 | For 10 ticks, the ICE on this host attacks you before your crewmates. |
| `mask` | 2 GB | 2 | 0 | For 5 ticks, ICE and analysts do not see your noise, and noise adds only half the trace. Sentries do not see you enter, and a Sentry on your host stops its ALERT against you. |
| `proxy` | 2 GB | 3 | 1 | Until the run ends, ICE attacks on your trace do half damage. On the host of a modem line, it also opens a route to the vault. |
| `pretext` | 2 GB | 2 | 2 | It tricks a Warden, an analyst who is not strict, or the IT admin of a vault. Until the run ends, the Warden lets you through, or the analyst does not watch you. |
| `exploit` | 2 GB | 3 | 3 | It raises your access on this host by one level. It does not work on software that the corporation patched. |

A new rig holds a maximum of 4 cycles. It gets 1 cycle on each tick. The programs work only in a target network. In a target network, a download also makes 2 noise.

A command line that runs programs costs 1 more cycle for the setup. If the programs make noise, the connection adds 1 more noise. As a result, `probe sentry | crack` costs 4 cycles and makes 4 noise. The same two commands on two lines cost 5 cycles and make 5 noise.

If you do not have enough cycles, the command does not run. It costs nothing.

### Versions and mods

Six programs have versions 1 to 3. A higher version makes the main effect of the program stronger. It does not change the RAM, the cycles, or the noise.

| Program | Version 1 | Version 2 | Version 3 |
| --- | --- | --- | --- |
| `crack` | 15 damage | 20 damage | 25 damage |
| `scrub` | 15% less trace | 20% less trace | 25% less trace |
| `patch` | 20 integrity | 25 integrity | 30 integrity |
| `firewall` | 1 attack | 2 attacks | 3 attacks |
| `mask` | 5 ticks | 6 ticks | 7 ticks |
| `decoy` | 10 ticks | 12 ticks | 14 ticks |

A version needs a blueprint. A blueprint is know-how, so the account keeps it after a burn. At the home host, `compile <program>` makes the next version, one version at a time. Version 2 costs 1000 cr, and version 3 costs 3000 cr. The `compile` command without a program shows the blueprints and the versions that you can compile.

A file with a name like `crack-v2.blueprint` is a blueprint. A handle who reads it learns the blueprint. About one job target in three has a blueprint for version 2 on a core host. Each heist vault has a blueprint for version 3. A shop can also sell blueprints for version 2.

The quiet mod makes a program make 1 less noise, and the program needs 1 more cycle. Type `compile <program> quiet` to add it for 400 cr, and `compile <program> plain` to remove it. A program can have one mod. A program without noise cannot have the quiet mod.

The versions and the mods are part of the rig. A burn takes them, and the police auction them.

## Exploit decay

A host template can give a software family, for example `Deskline OS 11`. The hosts from the template run that software, and `scan` shows it. Two templates can have the same family.

Each corporation counts the uses of `exploit` against each family. The count is for all players. When the count gets to the limit of the corporation, the corporation patches the family:

| Budget | Uses before a patch | Time of the patch |
| --- | --- | --- |
| Low | 12 | 2 days |
| Medium | 8 | 4 days |
| High | 5 | 7 days |
| Top | 3 | 14 days |

The use that gets to the limit still opens its shell. After the patch, `exploit` does not work on that family at that corporation, and it costs nothing. The `scan` command shows the patch, for example `Software: Deskline OS 11 (patched)`. When the time of the patch ends, somebody finds a new hole, and the count starts at 0 again. On a host without software, `exploit` always works.

## Roles

TTYmud has no classes. The programs in your RAM set your role. A new handle selects one starter kit, and the kit fills the RAM with the programs for one role. A handle can load only the programs that its account knows. A shop sells the other programs. At the home host, a handle can compile stronger versions of some programs.

| Kit | Role | Programs |
| --- | --- | --- |
| Breaker | Breaks ICE and takes the hits. | `scan`, `probe`, `crack`, `patch`, `firewall`. It also knows `decoy`. |
| Ghost | Stays hidden from Sentries. | `scan`, `probe`, `crack`, `scrub`, `mask` |
| Phreak | Slows the trace. | `scan`, `probe`, `crack`, `scrub`, `proxy` |
| Face | Talks past Wardens. | `scan`, `probe`, `crack`, `patch`, `pretext` |

In a crew, the roles help each other. A Ghost scrubs the logs of a crewmate, and a Breaker pulls ICE attacks away from the crew with `decoy`. In a heist, a Face or a Phreak can open a fast way into the vault.

## Crews

A crew is a group of up to 4 handles. Type `crew create <name>` to make a crew, and you lead it. A crew name follows the rules for a handle name. The leader uses these commands:

- `crew invite <handle>` invites a handle that is online. The handle types `crew join <crew>` to join.
- `crew kick <handle>` removes a member.
- `crew lead <handle>` makes a different member the leader.
- `crew drop` ends the heist of the crew.

Each member can type `crew` to see the crew, and `crew leave` to leave it. When the leader leaves, the next member leads the crew. When the last member leaves, the crew ends. A burn takes the handle out of its crew.

Type `#crew <text>` to talk to your crewmates on all hosts. Crew chat follows the same rules as the other chat: ignores, blocks, mutes, rate limits, and reports.

## Group runs

Crewmates on the same job share one copy of the target network. A crewmate who enters the network joins the copy of the crew. In the copy, crewmates see each other on hosts, and `say` reaches them. Players outside the crew get their own copy.

The ICE, the analysts, and the logs are the same for the whole crew. When one crewmate breaks an ICE or tricks a Warden, the crew can pass. When one crewmate scans a host, the crew knows its ICE. Each crewmate keeps a separate trace, integrity, access, and loot.

The ICE acts against one player at a time. The other crewmates on the host see each telegraph with the name of the target.

- A Sentry sends its ALERT about the player whose entry or noise it saw. The Hounds hunt that player.
- A Scorch attacks the player whose noise woke it. If that player leaves its host, it turns to a different crewmate on its host.

The noise of each player goes into the logs under the name of that player. An analyst who finds noise traces the player who made it. A strict analyst reads the host of the crewmate with the highest trace.

Two programs help a crew. With a handle, `scrub` lowers the trace of a crewmate on your host and deletes the logs of that crewmate there. The `decoy` program makes the ICE on your host attack you for 10 ticks. The copy stays while a crewmate is in it.

## Money

The currency is the credit (cr). Each handle has its own credits, and a new handle starts with 0 cr. Type `status` to see your credits.

- **Contracts pay.** The fixer pays when you make a clean exit with the goal file. The fixer keeps the goal file.
- **The fence buys stolen data.** A data file from a LAN host has a value of 40 cr. A data file from a core host has a value of 100 cr. The budget of the company multiplies the value by 1, 2, 4, or 8. The fence buys each file one time in each job, and its price falls after each sale.
- **A shop sells programs, blueprints, and hardware.** A program and a blueprint go into the know-how of your account. A burn takes the hardware, but not the know-how.
- **Compiles cost credits.** Version 2 of a program costs 1000 cr, version 3 costs 3000 cr, and the quiet mod costs 400 cr.
- **The police auction** the valuable items of each burned rig.
- **Repairs cost credits** above 50 integrity.

The content file sets the prices of a shop. These are the prices in Port Ostra:

| Item | Price |
| --- | --- |
| `scrub`, `patch` | 300 cr |
| `firewall`, `mask`, `pretext` | 400 cr |
| `proxy`, `decoy` | 500 cr |
| `exploit` | 800 cr |
| Blueprints for `scrub` v2 and `patch` v2 | 500 cr |
| Blueprints for `crack` v2 and `mask` v2 | 600 cr |
| RAM: 12 GB, then 16 GB | 900 cr, then 1800 cr |
| CPU: 5 cycles, then 6 cycles | 1200 cr, then 2400 cr |

### Data prices

The fence pays a price for each file name, for example `payroll.db`. The price is a percent of the value of the file. Each sale of a name lowers its price by 10%, to a minimum of 30%. The price goes up again by 10% every 2 hours, to 100%. All players share the prices, and `world.ini` keeps them. Type `sell` without a file to see the price of each file on your rig.

### The police auction

When a handle burns, the police auction the valuable items of its rig. The `auction` key of the city section gives the auction host. In Port Ostra, it is the Evidence Locker, next to the Night Market. Each item is a lot:

- Each data file and each blueprint file.
- RAM above 8 GB, and a CPU above 4 cycles.
- Each program version and each mod.

At the auction host, `lots` shows the lots, and `bid <lot> <credits>` bids on a lot. The first bid is 100 cr or more. A new bid must be 10% more than the top bid. The police hold the credits of the top bid. When somebody bids more, the last bidder gets the credits back, also when the bidder is offline.

Each lot is open for 3 days. Then the top bid wins, and the lot goes onto the rig of the winner. A file replaces a file with the same name. RAM and a CPU replace the hardware only if they are better. A won version also teaches its program. The police keep the credits, and a lot without a bid goes away.

The news reports each sale, but not the name of the winner. The server keeps the lots and the bids in `auction.ini`.

## Jobs

Fixers post jobs on a job board. Each day of UTC time, each fixer posts 3 jobs against the companies that the fixer works with. All players see the same jobs. Each player or crew gets a separate copy of the network, so many players can do the same job.

Type `jobs` on the host of a fixer to see the board. Type `accept <number>` to take a job. You can have one contract at a time. When you take a job, the routes into its network start at your home host. A job has no time limit. You cannot take a job again on the day that you finish it.

| Budget of the company | Pay | Necessary reputation |
| --- | --- | --- |
| Low | About 300 cr | 0 |
| Medium | About 700 cr | 2 |
| High | About 1500 cr | 5 |
| Top | About 3000 cr | 9 |

The pay of a job can be up to 20% more or less. Each clean contract adds 1 reputation with its fixer. The first job of each fixer is always against its company with the lowest budget. As a result, a new handle can always find work. No two jobs on one board have the same company and the same goal file.

A job has one of three goals. The first job of each fixer is always a theft. About one job in three is a wipe or a plant.

| Goal | What you do | When the job is complete |
| --- | --- | --- |
| Theft | Download the goal file. | You make a clean exit with the file on your rig. The fixer keeps the file. |
| Wipe | Type `rm <file>` on the host of the goal file. You need the access that the file needs. | You make a clean exit in the same copy of the network. |
| Plant | The fixer gives you a file. Type `upload <file>` on the host that the job names. You need the access that the files of that host need. | You make a clean exit in the same copy of the network. |

A wipe or a plant changes only your copy of the network. If you jack out, the next copy has the old files again. An upload copies the file, so you still have it for the next run. When a job is complete, the fixer takes the file of the job from your rig, if you have it. As a result, you cannot sell a copy of a file that you wiped.

## Heists

Each day, each fixer also posts one heist for a crew of 2 to 4 handles. The board shows the heists after the jobs. Only the leader of a crew can accept a heist. The heist is the contract of the crew. It does not replace the contract of a member.

| Budget of the company | Pay for the crew | Necessary crew reputation |
| --- | --- | --- |
| Low | About 900 cr | 0 |
| Medium | About 2100 cr | 2 |
| High | About 4500 cr | 5 |
| Top | About 9000 cr | 9 |

The goal of a heist is in the vault of the target. The vault has no route, and its ICE has the high or top security level. A heist target has a maintenance laptop on the LAN, and one more bridge:

- The laptop syncs with the vault for 3 ticks every 20 ticks. Two ticks before the sync, a telegraph tells the players on the laptop. During the sync, the laptop has a route to the vault.
- An IT admin works on a LAN host. A Face can type `pretext <admin>` there. Then the admin puts a USB drive into the vault, and that host has a route to the vault.
- An old modem line is on a core host. A Phreak can type `proxy` there. Then that host has a route to the vault.

Each target has the admin or the modem line, but not both. The USB drive and the modem line stay open until the copy ends. If the sync ends while you are in the vault, you must wait for the next sync, use a different bridge, or jack out. The `scan` command shows the laptop, the admin, and the modem line. The `contract` command shows the stages and the bridges of the heist.

When a crewmate makes a clean exit with the goal file, the heist is complete. The pay goes in equal parts to the crewmates who entered the target during the heist. The crew gets 1 reputation, and the alert level of the company goes up by 30 points. The news reports the heist. A crew can do the heist of a fixer one time each day.

## News

The news feed reports the events of the city:

- A breach: a handle completes a contract against a corporation.
- A trace in a network of a corporation.
- A burn. This is the only headline with the name of a handle.
- A patch, and the new hole after the patch.
- A change of an alert level.

The news reports a higher level immediately. It reports a lower level only when the points are more than 5 below the limit of the level. As a result, a level that goes up and down at its limit does not fill the news.

Type `news` on any host to see the 10 newest headlines, with the UTC time. The `news` key of the city section gives a host with the same headlines in the file `news.txt`. In Port Ostra, the file is on the Newsstand. The Status message carries the newest headline, and the HUD shows it.

## Rumors

Type `rumors` on the host of a fixer to hear rumors. Each fixer tells each handle 3 rumors each UTC day. A rumor is about a corporation of the fixer: its alert level, a patch, or an analyst and the shift of the analyst. Rumors are free.

A rumor can be true, old, or false. A true rumor tells the state of the world now. An old rumor tells the state before the last change, for example the alert level after its last increase. A false rumor tells a different state. With more reputation, more rumors are true:

| Reputation with the fixer | True rumors |
| --- | --- |
| 0 | 50% |
| 1 | 60% |
| 2 | 70% |
| 3 | 80% |
| 4 or more | 90% |

Half of the other rumors are old, and half are false.

## Progression

There are no levels. A handle gets stronger in three ways.

1. **Rig.** Better hardware. The CPU gives more cycles, and RAM holds more programs. The NIC makes downloads faster. Storage holds more loot. In version 0.2, a shop sells RAM and CPUs, and the police auction sells used hardware.
2. **Software.** Higher program versions and better mods. Find or buy a blueprint, and then compile the next version. In version 0.2, six programs have versions 1 to 3, and the quiet mod is the only mod.
3. **Reputation.** Each faction keeps a reputation value for your handle. Higher reputation opens harder targets, better contracts, and better vendors. In version 0.1, each fixer keeps a reputation value for your handle.

## Burn and legacy

When the heat gets to 5, the handle burns. Then these events occur:

- The handle goes on the burn list of the server. Nobody can use the name again on that server.
- The police seize the rig and its files. They auction the data, the blueprints, the better hardware, and the program versions and mods.
- The handle loses its credits, its reputation with each fixer, and its contract. Later, it will also lose its contacts.

The burn list is the file `burn-list.txt` on a hub host. In Port Ostra, the file is on the Newsstand. It gives each burned handle and the date of the burn.

The account keeps these things:

- **Know-how.** The account keeps all the programs and the blueprints that its handles knew. A new handle can load these programs and compile these versions, so the rebuild is faster.
- **The offshore stash.** At your home host, the `stash` command moves a file from your rig into the stash. The stash holds 3 files. After a burn, the next handle gets the stash. Later, the stash will also hold credits, and each deposit will have a fee.

Then the player makes a new handle and selects a starter kit. The new handle starts with heat 0 and a new rig.

## PvP by consent

TTYmud is PvE first. A handle can attack a different handle only in a darknet, in a duel, or in a crew match. Each of them needs the consent of the player.

### Attacks on a rig

In a darknet or an arena, `probe` and `crack` work on the rig of a handle on the same host. The `probe` command shows the integrity, the firewall, and the programs of the rig, and it tells the handle. A crack after a probe in the same pipeline does double damage. The `firewall` program stops cracks from handles as it stops ICE. The versions of the programs count, so `crack` v3 does 25 damage. Crewmates cannot attack each other.

### Darknets

A darknet is a network with `zone = darknet`. All handles share one copy of it. Before each entry from the city, the game shows the risks, and a second `go` to the same host in 60 seconds enters. Noise in a darknet gives no trace and no heat.

A crash in a darknet leaves the data files and the blueprint files of the rig on the host. Any handle can download them, and a download takes the file away. The handle wakes up at its home host with 0 integrity. A darknet keeps the newest 100 dropped files.

The `tip <handle>` command adds 1 heat to a different handle in the same darknet. A tip never takes the heat from 4 to 5, so a tip cannot burn a handle. A handle gets one tip in 24 hours, and its save keeps the time of the last tip. The tip does not name the sender.

The fence of a shop in a darknet pays 150% of the value of a file. It keeps its own prices, which go down and come back as in the city. A shop can sell blueprints for version 3.

### Arenas

An arena is a network with `zone = arena` and two bases. It has no route from the city. Each fight gets its own copy of the arena, and each side starts on a base. A fight gives no heat and causes no loss. When the fight ends, each rig gets back its integrity, cycles, files, and place in the city. During a fight, the save keeps the rig from before the fight.

| Fight | Start | Win | Time |
| --- | --- | --- | --- |
| Duel | `duel <handle> [credits]`, and the other handle answers in 60 seconds. | Crash the other rig. A jack out or a disconnect loses. | After 10 minutes, a draw. |
| Crew match | `match <crew> [credits]` by a leader, and the other leader answers in 60 seconds. | Carry the flag of the other crew to your base 3 times. | After 15 minutes, the crew with more captures wins. A tie is a draw. |

In a crew match, the crewmates who are online and in the city join. A crash sends the handle to its base with full integrity and 0 cycles, and the flag that it carries goes back. A crewmate who jacks out or disconnects leaves the match. If a leader leaves, the crew loses. The loser pays the wager to the winner. In a crew match, the two leaders pay and get the wager.
