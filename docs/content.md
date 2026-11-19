# Content files

A content file describes a city: its hosts, the routes between the hosts, and the files on the hosts. This document gives the format. Port Ostra, the default city, uses this format. A later version of the server will also read content files from its data directory.

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

### Host

A host section makes one host.

| Key | Count | Meaning |
| --- | --- | --- |
| `name` | 1 | The name of the host that players see. |
| `desc` | 1 | The description of the host. |
| `route` | 0 or more | The name of a host that this host connects to. |

A route connects two hosts in both directions. As a result, you write each route one time only.

### File

A file section puts one file on a host. The section name has the form `<host>/<file>`, for example `[file relay/motd.txt]`.

| Key | Count | Meaning |
| --- | --- | --- |
| `line` | 1 or more | One line of the file. The lines stay in their order. |

## Example

    [city port-ostra]
    name = Port Ostra
    start = relay

    [host relay]
    name = The Relay
    desc = The public node at the heart of the city net.
    route = night-market

    [host night-market]
    name = The Night Market
    desc = Stalls of cracked firmware and second-hand RAM.

    [file relay/motd.txt]
    line = Welcome to Port Ostra.

## Checks

The server checks a content file before it uses the file. It finds these errors and gives the line number of each error:

- Lines that it cannot read, and keys before the first section.
- Unknown section types and unknown keys.
- Keys that are missing or that occur too many times.
- Names that contain other characters.
- Two hosts or two files with the same name.
- Routes and files on unknown hosts, and routes from a host to itself.
- A start host that does not exist.
- Hosts that players cannot reach from the start host.

The server shows all errors at the same time. As a result, you can correct all of them in one step.
