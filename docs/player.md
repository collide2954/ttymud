# Player guide

This guide tells you how to connect to a TTYmud server and how to play.

## Build the client

Install Rust with [rustup](https://rustup.rs). Then use this command to build the client:

    cargo build --release -p ttymud

The binary is `target/release/ttymud`. On Windows, the binary is `target\release\ttymud.exe`.

## Connect

Use this command to connect to a server:

    ttymud connect ostra.example.net

The default port is 7331. To use a different port, add it after a colon, for example `ostra.example.net:7400`. Put an IPv6 address in brackets, for example `[2001:db8::7]:7400`.

## The first connection

At the first start, the client makes your identity key and shows its fingerprint. The key is your account. There is no password.

When you connect to a new server, the client shows the fingerprint of the server key:

    ostra.example.net:7331 is a new server for you. Its fingerprint is 9b5b:f04a:f430:799f.
    Type yes to trust it:

Compare the fingerprint with the fingerprint that the operator of the server publishes. If the two fingerprints are the same, type `yes`. The client then keeps the key of the server, and it does not ask again.

The `--accept-new` option trusts a new server without a question. Use it only for tests, or for a server that you know.

## A changed server key

If the key of a known server changes, the client stops and shows a warning. A changed key can mean that a different computer is between you and the server.

Do not continue if you are not sure. Ask the operator of the server. If the operator changed the key, remove the line for that server from the file `known_servers`. Then connect again.

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

- The top bar shows the host. During a run, it also shows the network, your access, and the trace.
- The output pane shows the game text.
- The chat pane shows `say` messages, channel messages, and private messages.
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

The `verbose` setting writes a line when a run starts or ends, and when your trace, access, integrity, or heat changes. An example is `Trace is now 50 percent.` Each telegraph also starts with `Alert`.

## Play

Type `help` to see the commands. Type `?` to see the commands that you can use on the current host.

Type `quit` to leave the game. The client waits for the answers to your last commands. Then it closes the connection.

If you connect again with the same account, the new connection replaces the old connection.

## Chat

You can talk in three ways:

- `say <text>` talks to the players on the same host. A line that starts with `'` is also a `say` message.
- A line that starts with `#` and the name of a channel talks in that channel, for example `#ostra hello`. A channel reaches its members on all hosts.
- `tell <handle> <text>` sends a private message to a handle that is online. `reply <text>` answers the last handle that sent you a private message.

Port Ostra has the channels `#ostra`, `#help`, and `#market`. A new handle starts in `#ostra` and `#help`. Type `channels` to see the channels. Type `join market` or `leave market` to change your channels. The server keeps your channels with your handle.

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
3. Find the ICE on this host: `scan`.
4. Find the weak point of the Sentry, and crack it in the same pipeline: `probe sentry | crack`.
5. Find the host with the file `patients.db`. Then download the file: `download patients.db`.
6. If your kit has `scrub`, lower your trace: `scrub`.
7. Go back the way that you came in: `back`. Then the contract is complete, and Pell pays you 250 cr.

## The job board

After the first job, go to the Back Room: `go back-room`. Type `jobs` to see the jobs of the fixers Pell and Wren. All players see the same jobs, and the jobs change each day. To take a job, type `accept` and the number of the job.

When you take a job, your safehouse gets routes into the network of the target. Each target has an edge, a LAN, and a core. The goal file is on a core host. Each clean job adds 1 reputation with its fixer. Jobs against bigger companies pay more, but they need more reputation.

## Access

In a target, you start with guest access on each host. Some files need more access. The `look` and `ls` commands mark them, for example `payroll.db (needs user)`. Find the credentials of the company and read them:

1. Find a credentials file on an edge host, for example `helpdesk.txt`. Read it to get user access in the whole network.
2. Find a second credentials file on a LAN host. Read it to get admin access.

Your access goes back to guest when the run ends. If you download a credentials file, you can read it from your rig in the next run. The `exploit` program raises your access on one host by one level, but it makes much noise. You can buy it at the Night Market.

## Money

Type `status` to see your credits. A clean exit with the goal file gets the pay of your contract. Use these commands at the Night Market:

- `shop` shows the programs and the hardware that you can buy.
- `buy <item>` buys a program, more RAM (`buy ram`), or a faster CPU (`buy cpu`).
- `sell <file>` sells stolen data to the fence. Files from bigger companies give more credits.

A repair up to 50 integrity is free. Each point above 50 costs 2 cr. A burn takes your credits and your hardware.

## Cycles and noise

Programs cost cycles. Your rig has a maximum of 4 cycles, and it gets 1 cycle on each tick of two seconds. If a command needs more cycles than you have, it does not run. Type `status` to see your cycles.

Programs also make noise, and noise raises your trace. A Sentry on the host doubles the trace from noise. A pipeline makes less noise than the same commands on different lines.

## Programs and RAM

A new rig has 8 GB of RAM. A program must be in RAM before you can run it. Type `status` to see the programs in RAM. At your safehouse, use `unload <program>` and `load <program>` to change them.

You can load only the programs that your account knows. A new account knows the programs of its starter kit. A shop sells the other programs. After a burn, your account also knows the programs of the next kit.

## Telegraphs

ICE acts on its own time. Before an ICE acts, it shows its next action in a telegraph:

    hound starts to compile TRACE BURST (2 ticks).

In this example, you have two ticks to answer. You can crack the ICE, leave the host, or scrub after the attack. Type `help ice` to see what each type of ICE does.

## The end of a run

A run ends in one of four ways:

- **Clean exit.** You leave through the host where you came in. You keep the loot. If you have the goal file of your contract, the fixer takes it and pays you.
- **Jack out.** You type `jackout`. You keep the loot, but the shock costs your rig 20 integrity. If the trace is 75% or more, you also get 1 heat.
- **Traced.** The trace gets to 100%. You lose the loot of the run, and you get heat.
- **Crash.** The integrity of your rig gets to 0. You lose the loot of the run.

If you disconnect from the server during a run, the game jacks you out. After a crash, your rig cannot connect to a target. Go to your safehouse and type `repair`.

## Heat

Each time a target traces you, your heat increases. The security level of the host where the trace gets to 100% sets the quantity: 1, 2, 3, or 5 heat. Type `status` to see your heat.

At heat 3 or more, Sentries alert faster. At heat 4 or more, fixers keep 25% of your pay, and Wren gives you no jobs. Your heat decreases by 1 for each 24 hours of real time without new heat. It also decreases while you are offline. At heat 5, your handle burns.

## A burn

When your handle burns, the police seize your rig and its files. You also lose your credits and your reputation. The handle goes on the burn list, the file `burn-list.txt` on the Newsstand. Nobody can use the name again.

Your account keeps its know-how and its stash. The know-how is the programs that your handles knew. After the burn, select a new handle and a starter kit.

## The stash

The stash keeps files safe from a burn. It holds 3 files. Use these commands at your safehouse:

- `stash <file>` moves a file from your rig into the stash.
- `unstash <file>` moves a file from the stash back to your rig.

The `stash` command without a file shows the files in the stash. It works on all hosts.
