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
| `inventory` | `inv`, `i` | Show the files on your rig. |
| `grep <pattern>` | | Keep only the items that contain the pattern. |
| `help [command]` | `man` | Show help for all commands or for one command. |
| `?` | | Show the commands that you can use here. |
| `status` | `st` | Show trace, access, heat, credits, integrity, cycles, RAM, and the programs in RAM. |
| `say <text>` | `'` | Talk to all players on the same host. |
| `jackout` | | Disconnect from the target immediately. |
| `repair` | | Restore integrity at your home host. A repair up to 50 integrity is free. |
| `load <program>` | | Put a program into RAM. This command works only at your home host. |
| `unload <program>` | | Remove a program from RAM. This command works only at your home host. |
| `stash [file]` | | Show your stash. At your home host, move a file from your rig into the stash. |
| `unstash <file>` | | Move a file from your stash to your rig. This command works only at your home host. |
| `contract [drop]` | `job` | Show your contract and its next step, or stop the contract. |
| `jobs` | | Show the jobs of the fixers on this host. |
| `accept <number>` | | Take a job from a fixer on this host. |
| `shop` | | Show what the shop on this host sells. |
| `buy <item>` | | Buy a program, more RAM, or a faster CPU at a shop. |
| `sell <file>` | | Sell stolen data to the fence of a shop. |

The programs are also commands. The [programs](#programs) section gives them.

If a command has an error, the game tells you the cause. It also gives the most likely correct command:

    > crak sentry
    Unknown command: crak. Did you mean crack?

In edit mode, the Tab key completes commands, routes, files, programs, ICE, analysts, handles, crewmates, channels, and help topics.

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

The trace does not increase while you wait. The pressure comes from ICE and from the analysts of the SOC.

**Integrity** is the health of your rig, from 100 to 0. Black ICE attacks decrease integrity. The `patch` program repairs some integrity during a run. When integrity gets to 0, your rig crashes.

At your home host, the `repair` command restores integrity. A repair up to 50 integrity is free. Each point above 50 costs 2 credits. If you do not have enough credits, the repair stops at the integrity that you can pay for.

**Heat** shows how much the police know about your handle, from 0 to 5. Each trace adds heat. The security level of the host where the trace gets to 100% sets the quantity of heat. In a network that an author makes by hand, all hosts have the security level of the network. The [world](world.md) document gives the security levels.

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

The noise of your actions goes into the logs of the host. Each analyst reads the logs of one host at a time. Two ticks before the read, a telegraph gives the name of the host. The habit of the analyst sets the host and the time between two reads:

| Habit | Host | Time between reads |
| --- | --- | --- |
| Careful | The host with the most noise in the logs. | 8 ticks |
| Strict | The host where you are. | 6 ticks |
| Lazy | A random host of the network. | 12 ticks |

If the analyst finds your noise, the trace increases by 5% and by the noise in the logs. Then the logs of that host are empty. If your access comes from stolen credentials, a careful or a strict analyst also locks the account. Your access in the network goes back to guest, but a shell from `exploit` stays open.

Three programs work against the SOC:

- `scrub` deletes your logs on the host where you are.
- While `mask` is up, noise does not go into the logs, and an analyst finds nothing.
- `pretext <analyst>` tricks an analyst who is not strict. Then the analyst does not watch your run.

## Programs

A program is software that you run to do an action. Each program has these properties:

- A name and a version, for example `crack v2.1`. A higher version is stronger.
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
| `probe` | 1 GB | 1 | 1 | It shows the weak point of an ICE. |
| `crack` | 2 GB | 2 | 2 | It does 15 damage to an ICE. After a probe in the same pipeline, it does 30 damage. |
| `scrub` | 2 GB | 2 | 0 | It decreases the trace by 15% and deletes your logs on this host. With a handle, it does this for a crewmate on your host. |
| `patch` | 2 GB | 2 | 0 | It repairs 20 integrity. |
| `firewall` | 2 GB | 2 | 0 | It stops the next ICE attack on your integrity. Only one firewall can be up. |
| `decoy` | 2 GB | 2 | 1 | For 10 ticks, the ICE on this host attacks you before your crewmates. |
| `mask` | 2 GB | 2 | 0 | For 5 ticks, ICE and analysts do not see your noise, and noise adds only half the trace. |
| `proxy` | 2 GB | 3 | 1 | Until the run ends, ICE attacks on your trace do half damage. On the host of a modem line, it also opens a route to the vault. |
| `pretext` | 2 GB | 2 | 2 | It tricks a Warden, an analyst who is not strict, or the IT admin of a vault. Until the run ends, the Warden lets you through, or the analyst does not watch you. |
| `exploit` | 2 GB | 3 | 3 | It raises your access on this host by one level. It does not work on software that the corporation patched. |

A new rig holds a maximum of 4 cycles. It gets 1 cycle on each tick. The programs work only in a target network. In a target network, a download also makes 2 noise.

A command line that runs programs costs 1 more cycle for the setup. If the programs make noise, the connection adds 1 more noise. As a result, `probe sentry | crack` costs 4 cycles and makes 4 noise. The same two commands on two lines cost 5 cycles and make 5 noise.

If you do not have enough cycles, the command does not run. It costs nothing.

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

TTYmud has no classes. The programs in your RAM set your role. A new handle selects one starter kit, and the kit fills the RAM with the programs for one role. A handle can load only the programs that its account knows. A shop sells the other programs. Later, handles will also compile programs.

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

- A Sentry sends its ALERT about the player whose noise it saw. The Hounds hunt that player.
- A Scorch attacks the player whose noise woke it. If that player leaves its host, it turns to a different crewmate on its host.

The noise of each player goes into the logs under the name of that player. An analyst who finds noise traces the player who made it. A strict analyst reads the host of the crewmate with the highest trace.

Two programs help a crew. With a handle, `scrub` lowers the trace of a crewmate on your host and deletes the logs of that crewmate there. The `decoy` program makes the ICE on your host attack you for 10 ticks. The copy stays while a crewmate is in it.

## Money

The currency is the credit (cr). Each handle has its own credits, and a new handle starts with 0 cr. Type `status` to see your credits.

- **Contracts pay.** The fixer pays when you make a clean exit with the goal file. The fixer keeps the goal file.
- **The fence buys stolen data.** A data file from a LAN host gives 40 cr. A data file from a core host gives 100 cr. The budget of the company multiplies the price by 1, 2, 4, or 8. The fence buys each file one time in each job.
- **A shop sells programs and hardware.** A program goes into the know-how of your account. A burn takes the hardware, but not the know-how.
- **Repairs cost credits** above 50 integrity.

The content file sets the prices of a shop. These are the prices in Port Ostra:

| Item | Price |
| --- | --- |
| `scrub`, `patch` | 300 cr |
| `firewall`, `mask`, `pretext` | 400 cr |
| `proxy`, `decoy` | 500 cr |
| `exploit` | 800 cr |
| RAM: 12 GB, then 16 GB | 900 cr, then 1800 cr |
| CPU: 5 cycles, then 6 cycles | 1200 cr, then 2400 cr |

## Jobs

Fixers post jobs on a job board. Each day of UTC time, each fixer posts 3 jobs against the companies that the fixer works with. All players see the same jobs. Each player or crew gets a separate copy of the network, so many players can do the same job.

Type `jobs` on the host of a fixer to see the board. Type `accept <number>` to take a job. You can have one contract at a time. When you take a job, the routes into its network start at your home host. A job has no time limit. You cannot take a job again on the day that you finish it.

| Budget of the company | Pay | Necessary reputation |
| --- | --- | --- |
| Low | About 300 cr | 0 |
| Medium | About 700 cr | 2 |
| High | About 1500 cr | 5 |
| Top | About 3000 cr | 9 |

The pay of a job can be up to 20% more or less. Each clean contract adds 1 reputation with its fixer. The first job of each fixer is always against its company with the lowest budget. As a result, a new handle can always find work.

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

1. **Rig.** Better hardware. The CPU gives more cycles, and RAM holds more programs. The NIC makes downloads faster. Storage holds more loot. In version 0.1, a shop sells RAM and CPUs.
2. **Software.** Higher program versions and better mods. You find, buy, or compile programs. A patch increases the minor version, for example from v2.1 to v2.2. A blueprint and parts let you compile a new major version, for example v3.0.
3. **Reputation.** Each faction keeps a reputation value for your handle. Higher reputation opens harder targets, better contracts, and better vendors. In version 0.1, each fixer keeps a reputation value for your handle.

## Burn and legacy

When the heat gets to 5, the handle burns. Then these events occur:

- The handle goes on the burn list of the server. Nobody can use the name again on that server.
- The police seize the rig and its files. Later, the items will go to the evidence auction.
- The handle loses its credits, its reputation with each fixer, and its contract. Later, it will also lose its contacts.

The burn list is the file `burn-list.txt` on a hub host. In Port Ostra, the file is on the Newsstand. It gives each burned handle and the date of the burn.

The account keeps these things:

- **Know-how.** The account keeps all the programs that its handles knew. A new handle can load these programs. Later, know-how will also keep blueprints, so that the rebuild is faster.
- **The offshore stash.** At your home host, the `stash` command moves a file from your rig into the stash. The stash holds 3 files. After a burn, the next handle gets the stash. Later, the stash will also hold credits, and each deposit will have a fee.

Then the player makes a new handle and selects a starter kit. The new handle starts with heat 0 and a new rig.
