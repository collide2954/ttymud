# Content files

A content file describes a city: its hosts and routes, the files on the hosts, the target networks, their ICE, and the contracts. It also gives the fixers, the shops, the chat channels, and the corporations and host templates for generated targets. This document gives the format. Port Ostra, the default city, uses this format.

A server reads its city from the file `city.ini` in its data directory. Without this file, it runs Port Ostra. The [operator guide](operator.md#make-your-own-city) tells you how to start a city and how to check it.

## Format

A content file is a text file in UTF-8. The server reads it line by line.

- A line that starts with `#` is a comment. The server ignores comments and empty lines.
- A line in square brackets starts a section, for example `[host relay]`. The first word is the section type. The second word is the name of the section.
- A line with `=` sets a key in the current section, for example `name = The Relay`. Spaces around the `=` are optional.
- Some keys can occur more than one time in a section, for example `route`.
- The server ignores a byte order mark (BOM) at the start of the file.

## Names

Section names contain only the characters `a-z`, `0-9`, and `-`. File names can also contain `.`. Players type these names, so keep them short.

## Sections

### City

Each content file has one city section.

| Key | Count | Meaning |
| --- | --- | --- |
| `name` | 1 | The name of the city that players see. |
| `start` | 1 | The host where new players start. |
| `home` | 0 or 1 | The host where players repair their rigs, change their RAM, and use their stash. It must be a hub host. Other players do not see a player there. |
| `burns` | 0 or 1 | The host with the burn list, the file `burn-list.txt`. It must be a hub host. |
| `news` | 0 or 1 | The host with the news feed, the file `news.txt`. It must be a hub host. The `news` command works without this key. |
| `auction` | 0 or 1 | The host of the police auction. When a handle burns, the police auction the valuable items of its rig there. It must be a hub host. Without this key, the city has no auction. |
| `contract` | 0 or 1 | The contract that each new handle gets. |

### Network

A network section makes one network by hand. A run starts when a player goes from a hub host to a host in a network.

| Key | Count | Meaning |
| --- | --- | --- |
| `name` | 1 | The name of the network that players see, for example the name of the company. |
| `security` | 0 or 1 | `low`, `medium`, `high`, or `top`. The default is `low`. A trace in this network gives 1, 2, 3, or 5 heat. |
| `zone` | 0 or 1 | `target`, `darknet`, or `arena`. The default is `target`. |
| `base` | 0 or 2 | A host of an arena where one side of a fight starts. Each arena must have two different bases. |

The zone sets the rules of the network:

- **Target.** Each run gets its own copy of the network, or one copy for each crew. Noise raises the trace.
- **Darknet.** All handles share one copy, and noise gives no trace. Handles can attack each other there. The players must agree before they enter.
- **Arena.** Duels and crew matches occur in an arena. Each fight gets its own copy, and the fight puts the handles on the two bases.

An arena has no route from the city. As a result, its hosts do not connect to hub hosts, and the check for hosts that players cannot reach skips them. The server uses the first arena of the content file.

### Host

A host section makes one host.

| Key | Count | Meaning |
| --- | --- | --- |
| `name` | 1 | The name of the host that players see. |
| `desc` | 1 | The description of the host. |
| `network` | 0 or 1 | The network of the host. A host without a network is a hub host. |
| `route` | 0 or more | The name of a host that this host connects to. |

A route connects two hosts in both directions. As a result, you write each route one time only. A route can connect a hub host and a host in a network. A route cannot connect two different networks.

### File

A file section puts one file on a host or on a template. The section name has the form `<host>/<file>`, for example `[file relay/motd.txt]`.

| Key | Count | Meaning |
| --- | --- | --- |
| `line` | 1 or more | One line of the file. The lines stay in their order. |
| `bait` | 0 or 1 | `yes` or `no`. The default is `no`. With `yes`, the file is a honeypot. If a player reads or downloads it in a target, the trace increases by 30%. |

A file with a name like `crack-v2.blueprint` is a blueprint. A player who reads it learns the blueprint for that version. The program must have versions, and the version must be 2 or 3.

A file with a name that ends in `.sh` is a script. A player who downloads it can run it with `sh`. Each line is one command line of the script, and a line that starts with `#` is a note. Players can also write scripts. As a result, a script cannot be the data of a corp or the goal of a contract.

### ICE

An ICE section puts one ICE on a host. The section name has the form `<host>/<name>`, for example `[ice smile-gw/sentry]`. The host must be in a network. Players type the name to probe or crack the ICE.

| Key | Count | Meaning |
| --- | --- | --- |
| `type` | 1 | `sentry`, `warden`, `hound`, `scorch`, `tarpit`, or `lockjaw`. |
| `version` | 0 or 1 | A number from 1 to 9. The default is 1. |
| `guards` | 0 or 1 | The other end of the route that a warden blocks. Each warden must have this key. Other types cannot have it. |

The [gameplay](design/gameplay.md#ice-types) document tells what each type does.

### Contract

A contract section makes one contract by hand. The `contract` key of the city section gives a contract to each new handle.

| Key | Count | Meaning |
| --- | --- | --- |
| `fixer` | 1 | The fixer section of the fixer who gives the contract. |
| `network` | 1 | The target network. |
| `goal` | 1 | The name of a file in the target network. A clean exit with this file completes the contract. |
| `pay` | 0 or 1 | The credits that the fixer pays for the contract. The default is 0. |
| `offer` | 1 or more | One line of the offer. A new handle sees the offer after it selects a starter kit. |
| `step` | 0 or more | One step of the contract: an event, a colon, and a hint. |

A step teaches one command. The `contract` command shows the hint of the next step. If the player makes no progress for 15 ticks, the game shows the hint one time. An event also completes the steps before it. The game skips a step for a program that the handle does not know.

| Event | When it occurs |
| --- | --- |
| `home` | The player goes to the home host. |
| `enter` | The player connects to the network of the contract. |
| `scan`, `probe`, `crack`, `scrub`, `patch` | The player runs the program. |
| `download` | The player downloads the goal file. |
| `exit` | The player leaves the network of the contract with a clean exit. |

### Fixer

A fixer section makes one fixer. Each day, a fixer posts 3 jobs against its corps. Players see the jobs on the host of the fixer.

| Key | Count | Meaning |
| --- | --- | --- |
| `name` | 1 | The name of the fixer that players see. |
| `host` | 1 | The hub host where the fixer is. |
| `corp` | 0 or more | A corp section. The fixer posts jobs against this corporation. |
| `max-heat` | 0 or 1 | A number from 0 to 4. The fixer refuses handles with more heat. |

A fixer with corps must have a corp with a low budget. As a result, a new handle can always find work. A fixer without corps gives only the contracts of the content file.

### Corp

A corp section makes one corporation. The server makes a new target network for each job against the corporation.

| Key | Count | Meaning |
| --- | --- | --- |
| `name` | 1 | The name of the corporation that players see. |
| `budget` | 1 | `low`, `medium`, `high`, or `top`. The budget sets the security levels, the ICE, the pay, and the necessary reputation. |
| `data` | 1 or more | One data file: a file name, a colon, and a line, for example `data = claims.db: Ten years of claims.` |

The server puts the data files on the LAN hosts and the core hosts of a target. One of the files is the goal of the job. The [world](design/world.md#targets) document tells how the server makes a target.

### Template

A template section describes one type of host for the targets. The server makes the hosts of a target from the templates.

| Key | Count | Meaning |
| --- | --- | --- |
| `layer` | 1 | `edge`, `lan`, `core`, or `vault`. |
| `name` | 1 | The name of the host type, for example `VPN Gateway`. The server puts the name of the corporation before it. |
| `desc` | 1 | The description of the host. |
| `software` | 0 or 1 | The software family of the host, for example `Gatehouse VPN 4`. The `scan` command shows it. |
| `laptop` | 0 or 1 | `yes` or `no`. The default is `no`. With `yes`, a `lan` template makes the maintenance laptop of a heist target. |

A file section can put a file on a template, for example `[file vpn/banner.txt]`. Each host from the template gets the file. If the content file has corps, each layer must have at least one template. A template cannot have the name of a host.

A `vault` template makes the vault of a heist target, and a laptop template makes its maintenance laptop. The fixers post heists only if the content file has both. Job targets do not use these templates, so each layer also needs a template that is not a laptop.

Each corporation counts the uses of `exploit` against each software family, and it patches a family after too many uses. Two templates with the same `software` value share one count. On a host without software, `exploit` always works. The [gameplay](design/gameplay.md#exploit-decay) document gives the limits.

### Shop

A shop section opens a shop on a hub host or on a host in a darknet. The section name is the name of the host, for example `[shop night-market]`. Each shop also has a fence that buys stolen data. The fence of a shop in a darknet pays 50% more, and it has its own prices.

| Key | Count | Meaning |
| --- | --- | --- |
| `program` | 0 or more | A program and its price in credits, for example `program = mask 400`. |
| `blueprint` | 0 or more | A program, a version, and the price of its blueprint, for example `blueprint = crack 3 2500`. The program must have versions. The version is 2 or 3. Without a version, the blueprint is for version 2. |
| `ram` | 0 or more | A RAM size in GB and its price, for example `ram = 12 900`. Each size must be more than the size before it. |
| `cpu` | 0 or more | A number of cycles and its price, for example `cpu = 5 1200`. Each number must be more than the number before it. |

The shop sells a program only to a handle whose account does not know it. It sells a blueprint only to an account that does not have it. Players buy a blueprint by its name, for example `buy crack-v2`. The shop sells only the next RAM size and the next CPU.

### Channel

A channel section makes one chat channel. Players talk in a channel with `#<channel> <text>`. A channel message reaches the members of the channel on all hosts.

| Key | Count | Meaning |
| --- | --- | --- |
| `desc` | 1 | The description of the channel. The `channels` command shows it. |
| `auto` | 0 or 1 | `yes` or `no`. The default is `no`. With `yes`, each new handle starts in the channel. |
| `linked` | 0 or 1 | `yes` or `no`. The default is `no`. With `yes`, the channel also reaches the players of the linked cities. |

The channel name `crew` is for crews. The `#crew` command always sends to the crew of the player. Only one channel of a city can be linked. In Port Ostra, the linked channel is `#longline`.

## Example

    [city port-ostra]
    name = Port Ostra
    start = relay
    home = safehouse
    burns = relay
    news = relay
    auction = relay
    contract = first-job

    [host relay]
    name = The Relay
    desc = The public node at the heart of the city net.
    route = safehouse

    [host safehouse]
    name = Your Safehouse
    desc = A quiet node that the city forgot to index.
    route = smile-gw

    [shop relay]
    program = mask 400
    blueprint = crack 600
    ram = 12 900

    [channel ostra]
    desc = All of Port Ostra.
    auto = yes

    [network smile-dental]
    name = Bright Smile Dental
    security = low

    [host smile-gw]
    name = Bright Smile Gateway
    desc = A cheap VPN box behind a dental office.
    network = smile-dental

    [ice smile-gw/sentry]
    type = sentry

    [file smile-gw/patients.db]
    line = 2,114 patient records.

    [fixer pell]
    name = Pell
    host = relay
    corp = mosaic

    [contract first-job]
    fixer = pell
    network = smile-dental
    goal = patients.db
    pay = 250
    offer = A client wants the patient files of a dentist.
    step = enter: Type go smile-gw to connect.
    step = download: Type download patients.db.
    step = exit: Type back to leave the network.

    [corp mosaic]
    name = Mosaic Media
    budget = low
    data = drafts.db: Next week of headlines.
    data = payroll.db: What each reporter earns.

    [template vpn]
    layer = edge
    name = VPN Gateway
    desc = The remote door for staff who work from home.
    software = Gatehouse VPN 4

    [template share]
    layer = lan
    name = File Share
    desc = Shared folders for every team.
    software = Ironside Server 8

    [template db]
    layer = core
    name = Database Server
    desc = The main database of the company.
    software = Ledgerbase 9

    [template maint]
    layer = lan
    name = Maintenance Laptop
    desc = The IT staff update the vault from it.
    laptop = yes

    [template vault]
    layer = vault
    name = Cold Vault
    desc = An air-gapped server in a locked room.

    [file vpn/banner.txt]
    line = Authorized users only.

## Checks

The server checks a content file before it uses the file. It finds these errors and gives the line number of each error:

- Lines that it cannot read, and keys before the first section.
- Unknown section types and unknown keys.
- Keys that are missing or that occur too many times.
- Names that contain other characters.
- Two sections of the same type with the same name, and two files or two ICE with the same name on one host.
- Routes and files on unknown hosts, and routes from a host to itself.
- A start host that does not exist.
- Hosts that players cannot reach from the start host. Hosts in an arena are not errors.
- A home, burns, news, auction, or fixer host that does not exist, or that is in a network.
- A shop host that does not exist, or that is in a target network or in an arena.
- Networks without hosts, hosts in unknown networks, and routes between two networks.
- Unknown zones, bases of networks that are not arenas, bases outside their arena, arenas without two bases, and routes into an arena.
- Unknown security levels, budgets, and layers.
- ICE on unknown hosts or on hub hosts, unknown ICE types, and bad versions.
- Wardens without a `guards` key, other ICE with a `guards` key, and wardens that guard no route.
- Contracts with an unknown fixer or for an unknown network, goals that are not files in the network, and goals that are scripts.
- Unknown events and steps without a hint.
- A `contract` key in the city section that names an unknown contract.
- Fixers against unknown corps, and fixers without a corp with a low budget.
- Corps without data, data lines without a colon, data that is a script, and corps without a template for each layer.
- Templates with the name of a host, laptop templates on a layer that is not `lan`, and vault templates without a laptop template.
- A channel with the name `crew`, and a second linked channel.
- Unknown programs in a shop, blueprints for programs without versions, bad blueprint versions, bad prices, and RAM or CPU steps that do not go up.
- Bad values for `pay`, `max-heat`, `auto`, `linked`, and `laptop`.

The server shows all errors at the same time. As a result, you can correct all of them in one step.
