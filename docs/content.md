# Content files

A content file describes a city: its hosts and routes, the files on the hosts, the target networks, their ICE, and the contracts. This document gives the format. Port Ostra, the default city, uses this format. A later version of the server will also read content files from its data directory.

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
| `contract` | 0 or 1 | The contract that each new handle gets. |

### Network

A network section makes one target network. A run starts when a player goes from a hub host to a host in a network.

| Key | Count | Meaning |
| --- | --- | --- |
| `name` | 1 | The name of the network that players see, for example the name of the company. |
| `security` | 0 or 1 | `low`, `medium`, `high`, or `top`. The default is `low`. A trace in this network gives 1, 2, 3, or 5 heat. |

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

A file section puts one file on a host. The section name has the form `<host>/<file>`, for example `[file relay/motd.txt]`.

| Key | Count | Meaning |
| --- | --- | --- |
| `line` | 1 or more | One line of the file. The lines stay in their order. |

### ICE

An ICE section puts one ICE on a host. The section name has the form `<host>/<name>`, for example `[ice smile-gw/sentry]`. The host must be in a network. Players type the name to probe or crack the ICE.

| Key | Count | Meaning |
| --- | --- | --- |
| `type` | 1 | `sentry`, `warden`, `hound`, or `scorch`. |
| `version` | 0 or 1 | A number from 1 to 9. The default is 1. |
| `guards` | 0 or 1 | The other end of the route that a warden blocks. Each warden must have this key. Other types cannot have it. |

The [gameplay](design/gameplay.md#ice-in-version-01) document tells what each type does.

### Contract

A contract section makes one contract. The `contract` key of the city section gives a contract to each new player.

| Key | Count | Meaning |
| --- | --- | --- |
| `fixer` | 1 | The name of the fixer who gives the contract. |
| `network` | 1 | The target network. |
| `goal` | 1 | The name of a file in the target network. A clean exit with this file completes the contract. |
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

## Example

    [city port-ostra]
    name = Port Ostra
    start = relay
    home = safehouse
    burns = relay
    contract = first-job

    [host relay]
    name = The Relay
    desc = The public node at the heart of the city net.
    route = safehouse

    [host safehouse]
    name = Your Safehouse
    desc = A quiet node that the city forgot to index.
    route = smile-gw

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

    [contract first-job]
    fixer = Pell
    network = smile-dental
    goal = patients.db
    offer = A client wants the patient files of a dentist.
    step = enter: Type go smile-gw to connect.
    step = download: Type download patients.db.
    step = exit: Type back to leave the network.

## Checks

The server checks a content file before it uses the file. It finds these errors and gives the line number of each error:

- Lines that it cannot read, and keys before the first section.
- Unknown section types and unknown keys.
- Keys that are missing or that occur too many times.
- Names that contain other characters.
- Two hosts, two files, two ICE on one host, two networks, or two contracts with the same name.
- Routes and files on unknown hosts, and routes from a host to itself.
- A start host that does not exist.
- Hosts that players cannot reach from the start host.
- A home host or a burns host that does not exist, or that is in a network.
- Networks without hosts, hosts in unknown networks, and routes between two networks.
- Unknown security levels.
- ICE on unknown hosts or on hub hosts, unknown ICE types, and bad versions.
- Wardens without a `guards` key, other ICE with a `guards` key, and wardens that guard no route.
- Contracts for unknown networks, and goals that are not files in the network.
- Unknown events and steps without a hint.
- A `contract` key in the city section that names an unknown contract.

The server shows all errors at the same time. As a result, you can correct all of them in one step.
