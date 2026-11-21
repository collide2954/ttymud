# Player guide

This guide tells you how to connect to a TTYmud server and how to play.

## Build the client

Install Rust with [rustup](https://rustup.rs). Then use this command to build the client:

    cargo build --release -p ttymud

The binary is `target/release/ttymud`. On Windows, the binary is `target\release\ttymud.exe`.

## Connect

The client has a public list of servers. Use this command to see the list:

    ttymud servers

To connect to a server on the list, use its name:

    ttymud connect ostra

To connect to a different server, use its address:

    ttymud connect ostra.example.net

The default port is 7331. To use a different port, add it after a colon, for example `ostra.example.net:7400`. Put an IPv6 address in brackets, for example `[2001:db8::7]:7400`.

If a server on the list has the same name as a host, the client connects to the server on the list. To connect to the host, add the port, for example `ostra:7331`.

The [website](https://collide2954.github.io/ttymud/servers.html) shows the same list. A client knows only the servers that were on the list when someone built it. For a newer server, use its address and its key.

## The first connection

At the first start, the client makes your identity key and shows its fingerprint. The key is your account. There is no password.

For a server on the list, the client compares the key of the server with the key on the list. If the two keys are the same, the client keeps the key of the server. It does not ask you.

When you connect to a new server that is not on the list, the client shows the fingerprint of the server key:

    ostra.example.net:7331 is a new server for you. Its fingerprint is 9b5b:f04a:f430:799f.
    Type yes to trust it:

Compare the fingerprint with the fingerprint that the operator of the server publishes. If the two fingerprints are the same, type `yes`. The client then keeps the key of the server, and it does not ask again.

The operator can also publish the full key of the server: 64 hexadecimal digits. Give the full key with the `--key` option. Then the client compares the keys, and it does not ask you:

    ttymud connect ostra.example.net --key 9b5bf04af430799f0e2a6c4d8b1f3e5a7c9d2b4f6e8a0c1d3e5f7a9b2c4d6e8f

The `--accept-new` option trusts a new server without a question. Use it only for tests, or for a server that you know.

## A changed server key

If the key of a known server changes, the client stops and shows a warning. A changed key can mean that a different computer is between you and the server. The client also stops if the key of the server is not the key on the list, or not the key that you gave.

Do not continue if you are not sure. Ask the operator of the server. If the operator changed the key, connect again with `--key` and the new key. The client then replaces the old key in the file `known_servers`.

## Play over SSH

If the server has an SSH port, you can play with a normal SSH client. You do not need the TTYmud client. Ask the operator for the port, for example 7332:

    ssh -p 7332 ostra.example.net

You need an SSH key. To make one, use the command `ssh-keygen -t ed25519`. The server accepts only keys, and your SSH key is the key of your account. At the first connection, the SSH client shows the fingerprint of the server. Compare it with the SSH fingerprint that the operator publishes.

The server selects the HUD for your terminal. To select a different display, add words after the address, and add the `-t` option:

    ssh -t -p 7332 ostra.example.net edit ascii

| Word | Effect |
| --- | --- |
| `line`, `edit`, `color`, `hud` | The display layer. Without a terminal, for example with `ssh -T`, you get line mode. |
| `ascii` | The display uses only ASCII characters. |
| `utf8` | The display uses UTF-8 characters. |
| `verbose` | The display writes the changes of state as lines, for screen readers. |

One account can use the client and SSH. On the device that has the account, type `link`. Then connect with SSH, and type `link <code>` at the handle prompt. To quit, type `quit`, or push Ctrl+D on an empty line.

## Your files

The client keeps your files in a configuration directory.

| System | Directory |
| --- | --- |
| Windows | `%APPDATA%\ttymud` |
| macOS, Linux, and other Unix systems | `$XDG_CONFIG_HOME/ttymud`, or `~/.config/ttymud` if `XDG_CONFIG_HOME` is not set |

To use a different directory, set the `TTYMUD_CONFIG` environment variable, or use the `--config` option. The directory contains these files:

- `identity.key`: your identity key. On Unix, the file has the permissions 0600.
- `known_servers`: the keys of the servers that you trust.
- `settings.ini`: your display settings. This file is optional.

> CAUTION: Keep the file `identity.key` secret. Keep a copy of it in a safe place. A person with this file can use your account. If you lose the file, you need your recovery code to get your account back.

## The HUD

In a terminal, the client shows the HUD. The HUD fills the window:

- The top bar shows the host. During a run, it also shows the network, your access, the alert level of the company, and the trace.
- The output pane shows the game text.
- The chat pane shows `say` messages, channel messages, and private messages. During a group run, the line above it shows your crewmates in the run and their traces.
- The line above the input line shows the latest headline of the news.
- The input line shows the prompt and the text that you type.
- The bottom bar shows the integrity of your rig, your heat, your RAM, your cycles, and your credits.

Push PgUp and PgDn to scroll the output pane. When you scroll up, the line below the pane shows "more below". When you send a command, the pane shows the newest text again.

If the window is small, the HUD has no side borders and no chat pane. If the window is very small, the HUD asks for a larger window.

## Edit mode

In the HUD and in edit mode, the client edits your input line itself. Text from the server shows above the input line. As a result, it does not mix with the text that you type.

| Key | Action |
| --- | --- |
| Left, Right, Home, End | Move the cursor. Ctrl+B, Ctrl+F, Ctrl+A, and Ctrl+E do the same. |
| Backspace, Delete | Delete one character. |
| Ctrl+W, Alt+Backspace | Delete the word before the cursor. |
| Ctrl+U | Delete the text before the cursor. |
| Ctrl+K | Delete the text after the cursor. |
| Up, Down | Show an older or a newer line from the history. Ctrl+P and Ctrl+N do the same. |
| Tab | Complete the word before the cursor. |
| Enter | Send the line. |
| Ctrl+C | Clear the line. On an empty line, quit. |
| Ctrl+D | Delete the character at the cursor. On an empty line, quit. |

Tab completes the names of commands, routes, files, help topics, channels, the handles that are online, and the ICE that you know. If more than one name is possible, Tab adds the part that the names have in common. If there is no part to add, the client shows all the possible names.

## Line mode

The client uses line mode when the input or the output is not a terminal. It also uses line mode when the `TERM` environment variable is `dumb`. On Windows, it uses line mode if the console cannot use virtual terminal sequences.

In line mode, the client reads full lines and writes full lines. Line mode works on all terminals and with screen readers. To use line mode in a terminal, use the `--line` option:

    ttymud connect ostra.example.net --line

## Display settings

The client selects the best display layer that your terminal can show:

| Layer | What you see |
| --- | --- |
| `line` | Lines of text and a prompt. |
| `edit` | The line editor, without color. |
| `color` | The line editor, with color. |
| `hud` | The HUD, with color. |

Use these options with `ttymud connect`:

- `--layer <layer>` selects a lower layer. `--line` is the same as `--layer line`.
- `--color <depth>` sets the colors: `none`, `16`, `256`, or `true`.
- `--ascii` draws lines and symbols with ASCII characters only.
- `--verbose` writes changes of state as text, for screen readers.

To keep your choices, write them in the file `settings.ini` in your configuration directory:

    layer = color
    color = 256
    ascii = no
    verbose = yes

An option on the command line has priority over the file. The client never uses a layer that your terminal cannot show.

The `verbose` setting writes a line when a run starts or ends, and when your trace, access, alert level, integrity, or heat changes. An example is `Trace is now 50 percent.` It also writes each new headline of the news, and each crewmate who comes into your run or leaves it. Each telegraph also starts with `Alert`.

## Play

Type `help` to see the commands. Type `?` to see the commands that you can use on the current host.

Type `quit` to leave the game. The client waits for the answers to your last commands. Then it closes the connection.

If you connect again with the same account, the new connection replaces the old connection.

## Chat

You can talk in three ways:

- `say <text>` talks to the players on the same host. A line that starts with `'` is also a `say` message.
- A line that starts with `#` and the name of a channel talks in that channel, for example `#ostra hello`. A channel reaches its members on all hosts.
- `tell <handle> <text>` sends a private message to a handle that is online. `reply <text>` answers the last handle that sent you a private message.

Port Ostra has the channels `#ostra`, `#help`, `#market`, and `#longline`. A new handle starts in `#ostra` and `#help`. Type `channels` to see the channels. Type `join market` or `leave market` to change your channels. The server keeps your channels with your handle.

Type `who` to see the handles that are online. The list marks the moderators.

The server can read all chat, also private messages. Before your first private message, the server tells you this. A new account must wait 10 minutes before it sends private messages. The server also blocks a message that is the same as your last message, for 30 seconds.

## Ignore, block, and report

If a player harasses you, use these commands:

- `ignore <handle>` hides all messages of that handle from you.
- `block <handle>` also refuses the private messages of that handle.
- `unignore <handle>` and `unblock <handle>` remove a handle from your lists.
- `ignore` without a handle shows whom you ignore and block.

The server keeps your lists with your account. You can also ignore or block a handle that is offline.

To tell the moderators about a player, type `report`, the handle, and a reason, for example `report zer0 spam`. The report contains the chat of that handle from the last 15 minutes. It contains the public messages of that handle, and its private messages to you. The reported player does not see the report.

## Moderators

The operator of the server selects the moderators. A moderator can do these actions:

- Mute a handle for some minutes. A muted handle cannot send chat messages.
- Kick a handle. The server disconnects the handle, but the player can connect again.
- Ban an account. The server disconnects the account and refuses it after that. A ban can also refuse an IP address.

The moderators also read the reports. The server writes each action of a moderator to an audit log. If you are a moderator, type `help` to see your commands.

## Linked cities

Each server is a different city. Operators can make links between their servers. Type `cities` to see the linked cities, and if each link is up.

The `#longline` channel reaches the players of all linked cities. To talk there, type `join longline` first. The name of a handle from a different city ends with the name of its city, for example `nyx@night-owl`.

To read the headlines of a linked city, type `news` and the name of the city, for example `news night-owl`.

The `ignore`, `block`, and `report` commands also work on a handle from a different city, for example `ignore nyx@night-owl`. Private messages stay in your city.

## Your account on more devices

Your identity key is your account. When you make your first handle, the server shows a recovery code one time:

    Your recovery code: 3f9a-2c71-0b8e-55d4-91c6-7a2f-e310-4d8b

> CAUTION: Write down your recovery code and keep it in a safe place. A person with the code can use your account.

If you lose the file `identity.key`, the client makes a new key at its next start. Connect to the server with the new key. At the handle prompt, type `recover` and your code. You get your account back, and the server shows a new recovery code. After that, the old code does not work. To make a new code at a different time, type `recovery`.

To use your account on a second device, do these steps:

1. On the device that you use now, type `link`. The server shows a link code.
2. On the new device, connect to the server. At the handle prompt, type `link` and the code.

A link code works one time, for 10 minutes. Type `keys` to see the keys of your account. To remove a key, for example the key of a lost device, type `revoke` and the number of the key. A revoked key cannot connect.

Your account can be online on one device at a time. When you connect on a second device, the first device disconnects.

To delete your account and its data on a server, ask the operator of the server.

## Your handle

At your first connection, the game asks for a handle. The handle is the name that other players see. A handle has 3 to 16 letters, digits, hyphens, or underscores. The game compares names without case, so `Nyx` and `nyx` are the same name.

Then the game asks for a starter kit. Each kit fills the RAM of your rig with five programs.

| Kit | Strength | Programs |
| --- | --- | --- |
| `breaker` | Breaks ICE and takes the hits. | `scan`, `probe`, `crack`, `patch`, `firewall` |
| `ghost` | Stays hidden from Sentries. | `scan`, `probe`, `crack`, `scrub`, `mask` |
| `phreak` | Slows the trace. | `scan`, `probe`, `crack`, `scrub`, `proxy` |
| `face` | Talks past Wardens. | `scan`, `probe`, `crack`, `patch`, `pretext` |

The server keeps your handle, your rig, and your contract. When you connect again, you start at your safehouse. Until you have a handle, the client shows the name `anon-` and the first eight characters of your key.

## Your first run

When you have a handle, the fixer Pell offers you a job. A dentist keeps patient files on an old box, and a client wants them. Type `contract` to see the job and the next step. If you make no progress for 30 seconds, the game gives you a hint.

Do these steps:

1. Go to your safehouse, where your rig is: `go safehouse`.
2. Connect to the target: `go smile-gw`. The prompt now shows your trace and your integrity.
3. Find the ICE on this host: `scan`. The Sentry saw you come in, and it compiles an ALERT.
4. Find the weak point of the Sentry, and crack it in the same pipeline: `probe sentry | crack`. The pipeline needs 4 cycles, so wait 2 ticks after the scan.
5. A Warden guards the route to `records`. Crack it: `probe warden | crack`, and then `crack warden`.
6. Go to `records`, and download the file: `download patients.db`. The ICE on `records` wakes and burns your rig, so leave at once: `back`.
7. If your kit has `scrub`, lower your trace: `scrub`.
8. Go back the way that you came in: `back`. Then the contract is complete, and Pell pays you 250 cr.

If the Sentry sends its ALERT, your trace goes up, but you can go on. If the target traces you in this first job, you lose the run but get no heat. Then the steps start again.

## The job board

After the first job, go to the Back Room: `go back-room`. Type `jobs` to see the jobs of the fixers Pell and Wren. All players see the same jobs, and the jobs change each day. To take a job, type `accept` and the number of the job.

When you take a job, your safehouse gets routes into the network of the target. Each target has an edge, a LAN, and a core. The goal file is on a core host. Each clean job adds 1 reputation with its fixer. Jobs against bigger companies pay more, but they need more reputation.

A job has one of three goals. The board and the `contract` command show it:

- **Get** a file: download it, and then leave with a clean exit.
- **Wipe** a file: delete it with `rm <file>` on its host, and then leave with a clean exit.
- **Plant** a file: the fixer gives you the file. Copy it to the named host with `upload <file>`, and then leave with a clean exit.

A wipe or a plant counts only if you leave clean in the same run. If you jack out, the next run has the old files again. You still have the file to plant, so you can upload it again.

## News and rumors

Type `news` on any host to see the 10 newest headlines of the city. The news reports breaches, traces, burns, patches, and changes of alert levels. It never gives the name of a handle, except for a burn. The Newsstand has the same headlines in the file `news.txt`.

In the Back Room, type `rumors` to hear what the fixers know. Each fixer tells you 3 rumors each day, about alert levels, patches, and analysts. Some rumors are true, some are old, and some are false. With more reputation, a fixer tells you more true rumors. Rumors are free.

## Crews

A crew has up to 4 handles. Type `crew create <name>` to make one. Then type `crew invite <handle>` for each handle that you want. That handle then types `crew join <crew>`. Type `crew` to see your crew, and `#crew <text>` to talk to your crewmates on all hosts.

When crewmates go into the network of the same job, they share one copy of it. You see your crewmates and hear their `say` messages. When one of you breaks an ICE, it is broken for all of you. Each of you keeps your own trace, and the ICE attacks one of you at a time. These programs help your crew:

- `scrub <handle>` cleans the logs of a crewmate on your host and lowers the trace of that crewmate.
- `decoy` makes the ICE on your host attack you for 10 ticks. Breakers know it, and the Night Market sells it.

## Heists

The board in the Back Room also lists heists for crews of 2 to 4. The leader of your crew types `accept` and the number of the heist. Then each safehouse of the crew has a way into the target. Type `contract` to see the stages of the heist.

The goal is in the vault, and the vault has no route. To get in, use a bridge:

- Wait on the maintenance laptop. Every 20 ticks, it syncs with the vault for 3 ticks.
- If the target has an IT admin, a Face can trick the admin with `pretext`.
- If the target has an old modem line, a Phreak can run `proxy` on its host.

Type `scan` on a host to find the laptop, the admin, or the modem line. If the sync ends while you are in the vault, wait for the next sync or jack out. When a crewmate leaves clean with the goal file, the pay goes in equal parts to each crewmate who entered the target.

## Access

In a target, you start with guest access on each host. Some files need more access. The `look` and `ls` commands mark them, for example `payroll.db (needs user)`. Find the credentials of the company and read them:

1. Find a credentials file on an edge host, for example `helpdesk.txt`. Read it to get user access in the whole network.
2. Find a second credentials file on a LAN host. Read it to get admin access.

Your access goes back to guest when the run ends. If you download a credentials file, you can read it from your rig in the next run. The `exploit` program raises your access on one host by one level, but it makes much noise. You can buy it at the Night Market.

## Patches

Each host of a company runs software, and `scan` shows it, for example `Software: Deskline OS 11`. Each time a handle uses `exploit` against a company, the company counts the use for that software. All players add to the same count. After too many uses, the company patches the software. Then `exploit` does not work on that software at that company, and `scan` shows `(patched)`.

A company with a bigger budget patches after fewer uses, and its patch stays for a longer time. After some days, somebody finds a new hole in the software, and `exploit` works again. The news reports each patch and each new hole.

## Money

Type `status` to see your credits. A clean exit with the goal file gets the pay of your contract. Use these commands at the Night Market:

- `shop` shows the programs, the blueprints, and the hardware that you can buy.
- `buy <item>` buys a program, a blueprint (for example `buy crack-v2`), more RAM (`buy ram`), or a faster CPU (`buy cpu`).
- `sell <file>` sells stolen data to the fence. Files from bigger companies give more credits. Type `sell` without a file to see the prices now.

The fence pays less for a file name after each sale. Each sale of a name lowers its price by 10%, to a minimum of 30% of the value. The price goes up again by 10% every 2 hours. All players share these prices, so a file that many players sell is cheap for some hours.

At your safehouse, `repair` restores your rig to 50 integrity for free. Then it shows the price of a full repair. Above 50, each point costs 2 cr, and `repair all` pays for it. A burn takes your credits and your hardware.

## Cycles and noise

Programs cost cycles. Your rig has a maximum of 4 cycles, and it gets 1 cycle on each tick of two seconds. If a command needs more cycles than you have, it does not run. Type `status` to see your cycles.

Programs also make noise, and noise raises your trace. A Sentry on the host doubles the trace from noise. A pipeline makes less noise than the same commands on different lines.

A working Sentry also sees you when you enter its host, and it starts to compile an ALERT. This ALERT takes 5 ticks. Crack the Sentry before the ALERT comes, or use `mask`. While `mask` is up, a Sentry does not see you. When you start `mask`, a Sentry on your host that compiles an ALERT against you loses you.

## Programs and RAM

A new rig has 8 GB of RAM. A program must be in RAM before you can run it. Type `status` to see the programs in RAM. At your safehouse, use `unload <program>` and `load <program>` to change them.

You can load only the programs that your account knows. A new account knows the programs of its starter kit. A shop sells the other programs. After a burn, your account also knows the programs of the next kit.

## Program versions

Six programs have versions: `crack`, `scrub`, `patch`, `firewall`, `mask`, and `decoy`. Version 2 and version 3 make the main effect of the program stronger. For example, `crack` does 15 damage at version 1, 20 damage at version 2, and 25 damage at version 3. The `status` command shows the version after the name, for example `crack v2`.

To make a version, you need its blueprint. At your safehouse, type `compile` to see your blueprints and the versions that you can compile. Then type `compile <program>` to compile the next version. Version 2 costs 1000 cr, and version 3 costs 3000 cr.

## Blueprints

A blueprint is a file with a name like `crack-v2.blueprint`. Read the file to learn the blueprint. About one job target in three has a blueprint for version 2 on a core host. Each heist vault has a blueprint for version 3. The Night Market sells some blueprints for version 2. Your account keeps its blueprints after a burn.

## The quiet mod

The quiet mod makes a program make 1 less noise, but the program needs 1 more cycle. At your safehouse, type `compile <program> quiet` to add it for 400 cr. Type `compile <program> plain` to remove it. A program can have one mod, and a program without noise cannot have the quiet mod. The `status` command shows the mod, for example `crack v2 (quiet)`.

The versions and the mods are part of your rig. A burn takes them.

## Telegraphs

ICE acts on its own time. Before an ICE acts, it shows its next action in a telegraph:

    hound starts to compile TRACE BURST (2 ticks).

In this example, you have two ticks to answer. You can crack the ICE, leave the host, or scrub after the attack. Type `help ice` to see what each type of ICE does.

Bigger companies use two more types of ICE. The TAR of a Tarpit takes all your cycles. The BITE of a Lockjaw freezes the program that you used last, for 6 ticks. Both stop when you leave their host.

## Honeypots

Each target has a honeypot: a bait file with a name such as `passwords.txt`. If you read or download it, your trace goes up by 30%. Before you touch a file, probe it:

    probe passwords.txt

The `ls | probe` pipeline checks all files on the host.

## The SOC

Each company has a security operations center (SOC). Its analysts read the logs of your run. The day shift, from 08:00 to 20:00 UTC, has two analysts. The night shift has one analyst, but the ICE is one version stronger. Type `scan` to see the analysts on shift and their habits.

Before an analyst reads the logs of a host, a telegraph shows it:

    [SOC] Analyst h.reyes turns to the logs of vpn.

The telegraph comes only when the host has logs to find. If you leave logs on a host that an analyst watches, the telegraph comes at once. Two ticks later, the analyst reads the logs. If your noise is in the logs of that host, your trace goes up.

A careful analyst reads the host with the most noise. A strict analyst reads the host where you are. A lazy analyst reads a random host, and reads less often.

Stolen credentials make no noise, but each login leaves a line in the logs of the host where you read them. If you have access from stolen credentials, a careful or a strict analyst who finds your logs locks the account. Then your access in the network is guest again. The analyst also resets the password, so those credentials do not work until the run ends.

A strict analyst who finds your noise can also cut one route that leads deeper into the network. A telegraph comes two ticks before the cut. The cut lasts until the run ends, but the way back out stays open.

These actions help you against the SOC:

- `scrub` deletes your logs on the host where you are.
- While `mask` is up, your noise and your logins do not go into the logs.
- `pretext <analyst>` tricks an analyst. Then the analyst does not watch your run. A strict analyst does not believe you.

## Alert levels

Each company has an alert level: green, yellow, orange, or red. The noise, traces, and breaches of all players raise it. The alert level goes down slowly. The change from red to green takes about 6 hours. The job board, `status`, and the HUD show the alert level.

A higher alert level makes each new run more difficult. Each level above green adds 25% to the trace from noise, one analyst, and one version to each ICE. For example, if a different player made noise at Arcadia Bank one hour ago, the bank can still be on alert when you arrive. Type `help soc` for a summary.

## The end of a run

A run ends in one of four ways:

- **Clean exit.** You leave through the host where you came in. You keep the loot. If you have the goal file of your contract, the fixer takes it and pays you.
- **Jack out.** You type `jackout`. You keep the loot, but the shock costs your rig 20 integrity. If the trace is 75% or more, you also get 1 heat, but not in your first job.
- **Traced.** The trace gets to 100%. You lose the loot of the run, and you get heat. In your first job, you get no heat.
- **Crash.** The integrity of your rig gets to 0. You lose the loot of the run.

If you disconnect from the server during a run, the game jacks you out. After a crash, your rig cannot connect to a target. Go to your safehouse and type `repair`.

## Heat

Each time a target traces you, your heat increases. The security level of the host where the trace gets to 100% sets the quantity: 1, 2, 3, or 5 heat. Type `status` to see your heat.

At heat 3 or more, Sentries alert faster. At heat 4 or more, fixers keep 25% of your pay, and Wren gives you no jobs. Your heat decreases by 1 for each 24 hours of real time without new heat. It also decreases while you are offline. At heat 5, your handle burns.

## A burn

When your handle burns, the police seize your rig and its files. You also lose your credits and your reputation. The handle goes on the burn list, the file `burn-list.txt` on the Newsstand. Nobody can use the name again.

Your account keeps its know-how and its stash. The know-how is the programs and the blueprints that your handles knew. After the burn, select a new handle and a starter kit.

## The police auction

The police auction the valuable items of each burned rig at the Evidence Locker, next to the Night Market. Each item is a lot:

- Each data file and each blueprint file.
- RAM above 8 GB, and a CPU above 4 cycles.
- Each program version and each mod.

Use these commands at the Evidence Locker:

- `lots` shows the lots, the top bid of each lot, and the time that is left.
- `bid <lot> <credits>` bids on a lot. The first bid is 100 cr or more. A new bid must be 10% more than the top bid.

The police hold your credits until somebody bids more. Then you get your credits back. Each lot is open for 3 days. Then the top bid wins, and the lot goes onto the rig of the winner. A won version also teaches its program. The police keep the credits.

A lot without a bid goes away when its time ends. The news reports each sale, but not the name of the winner. Type `help economy` for a summary.

## The stash

The stash keeps files safe from a burn. It holds 3 files. Use these commands at your safehouse:

- `stash <file>` moves a file from your rig into the stash.
- `unstash <file>` moves a file from the stash back to your rig.

The `stash` command without a file shows the files in the stash. It works on all hosts.

## Fights between handles

Other handles can attack you only in places that you select: a darknet, a duel, or a crew match. In targets and in the city, no handle can attack you. Type `help pvp` for a summary.

## Darknets

A darknet is a network that all handles share. In Port Ostra, the Undercity is a darknet under the Night Market. When you go to a darknet, the game tells you the risks. To agree, type the same `go` command again in the next 60 seconds. The game asks again each time that you come in from the city.

In a darknet, noise gives no trace and no heat. But other handles on your host can attack your rig:

- `probe <handle>` shows the integrity, the firewall, and the programs of the rig. The handle sees the probe.
- `crack <handle>` hits the integrity of the rig. `probe <handle> | crack` hits the weak point for double damage.
- `firewall` stops the next attacks, and `patch` repairs your rig. A higher version makes each program stronger. For example, `crack` v3 does 25 damage.

Crewmates cannot attack each other. If your rig crashes in a darknet, your data files and your blueprint files stay on that host. Any handle can download them. You wake up at your safehouse with 0 integrity. To leave at once, type `jackout`. It costs 20 integrity, but you keep your files.

`tip <handle>` tells the police about a different handle in the same darknet. The tip adds 1 heat to that handle, but never the fifth heat. A handle gets only one tip in 24 hours. The handle does not learn who sent the tip.

The fence of the Black Market in the Undercity pays 50% more than the fences in the city, and it has its own prices. The Black Market also sells blueprints for version 3.

## Duels

A duel is a fight between two handles in an arena. In Port Ostra, the arena is the Grid. Use these commands in the city, outside the networks:

- `duel <handle>` challenges a handle. For a wager, add the credits, for example `duel zer0 100`.
- `duel <handle>` also accepts the challenge of that handle. Type it in the next 60 seconds.

Each handle starts on a base of the arena. Crash the rig of the other handle to win. A handle that jacks out or disconnects loses. After 10 minutes, the duel is a draw. The winner takes the wager from the loser.

A fight gives no heat, and no rig loses anything. When the fight ends, you get back your integrity, your cycles, your files, and your place in the city.

## Crew matches

A crew match is a game of capture the flag between two crews in the arena. Only a crew leader can start a match:

- `match <crew>` challenges a crew. For a wager between the two leaders, add the credits, for example `match owls 200`.
- `match <crew>` also accepts the challenge of that crew. Type it in the next 60 seconds.

All crewmates who are online and in the city join the match. Each crew starts on its base, and each base has the flag of its crew, for example `owls.flag`. Download the flag of the other crew, and carry it to your own base. Each time that you do this, your crew scores one capture. You cannot take the flag of your own crew.

If your rig crashes in a match, you wake up at your base with full integrity and 0 cycles. A flag that you carry goes back to its base. The first crew with 3 captures wins. After 15 minutes, the crew with more captures wins, and a tie is a draw.

A crewmate who jacks out or disconnects leaves the match. If the leader leaves, the crew loses the match. The leader of the winning crew takes the wager from the other leader.

## Scripts

A script is a file on your rig with one command line on each line. Your rig runs the commands for you, one after the other. The name of a script ends in `.sh`, for example `sweep.sh`. Type `help scripts` for a summary.

Use `echo` to write a script:

    echo 'scan' > sweep.sh
    echo 'probe $1 | crack' >> sweep.sh
    echo 'scan | grep $1 && crack $1' >> sweep.sh

The `>` arrow makes the script, or replaces all its lines with one line. The `>>` arrow adds a line at the end. Put the text in single quotes when it has `|`, `&&`, `||`, or double quotes. Type `cat sweep.sh` to see the script, and type `rm sweep.sh` to delete it. A script has a maximum of 20 lines, and your rig holds a maximum of 10 scripts.

To run the script, type `sh sweep.sh sentry`. The words after the name of the script go into `$1` to `$9`. In this example, `$1` is `sentry`. Before the script starts, the game checks all its lines. If a line has an error, the script does not start, and the game shows the line.

A script runs in the background, so you can type other commands at the same time. It runs one command on each tick. If a command needs more cycles than your rig has, the script waits for the cycles. A command from a script costs the same cycles and makes the same noise as the command that you type.

A running script uses 1 GB of RAM. A starter kit fills all the RAM of a new rig. Before you run a script, unload a program at your safehouse, or buy more RAM. Only one script runs at a time. Type `ps` to see the script and its line, and type `kill` to stop it.

These parts of a line control the script:

- `&&` runs the next command only if the last command worked.
- `||` runs the next command only if the last command failed.
- `wait 3` makes the script wait for 3 ticks. A `wait` line has no other commands.
- A line that starts with `#` is a note. The script does not run it.

If the last command of a line fails, the script stops. The `grep` command fails when nothing matches. For example, `scan | grep sentry && crack sentry` cracks the Sentry only if the Sentry is on the host. A crash, a trace, a burn, or a disconnect also stops the script.

A script cannot chat, and it cannot use `report`, `crew`, `duel`, or `match`. It also cannot use the moderator commands or the commands for keys and recovery codes. A script cannot start a different script. At the prompt, `&&` and `||` do not work. Use them only in scripts.

The Night Market of Port Ostra has a sample script, `break.sh`. Download it, and read it. Then run it in a target, for example `sh break.sh sentry`.
