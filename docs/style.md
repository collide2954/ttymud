# Style guide

All TTYmud documents use ASD-STE100 Simplified Technical English (STE), Issue 9. STE makes technical text clear for all readers. This is also true for readers who do not speak English as a first language.

This page gives the STE rules that TTYmud documents use most. It also gives the technical names and technical verbs of TTYmud. For all other rules, read the official STE specification at [asd-ste100.org](https://www.asd-ste100.org).

## Words

- Use words from the STE dictionary, with their approved meaning and part of speech.
- Use the technical names and technical verbs on this page.
- Use the same word for the same thing each time.
- Do not use the "-ing" form of a verb, except in a technical name.

TTYmud documents do not use the words in the left column. Use the words in the right column. Check all other words in the official STE dictionary.

| Do not use | Use |
| --- | --- |
| `additional` | more |
| `approximately` | about |
| `assist` | help |
| `attempt` | try |
| `commence`, `initiate` | start |
| `demonstrate`, `indicate` | show |
| `ensure` | make sure |
| `may`, `might` | can |
| `numerous` | many |
| `perform` | do |
| `prior to` | before |
| `provide` | give, supply |
| `require` | must, necessary |
| `should` | must, or an instruction |
| `subsequent` | after, next |
| `sufficient` | enough |
| `terminate` | stop |
| `therefore`, `thus` | as a result |
| `utilize` | use |

## Sentences

- Write a maximum of 20 words in a procedural sentence.
- Write a maximum of 25 words in a descriptive sentence.
- Do not use contractions. Write `do not`, not `don't`.
- Do not use semicolons. Write two sentences.
- Use the active voice.
- Do not omit words to make a sentence shorter. Keep the articles "a", "an", and "the".
- Do not use more than three nouns in a noun cluster.
- Use a vertical list for complex text.

## Procedures

- Write instructions in the imperative form. Example: "Start the server."
- Write one instruction in each sentence. Two actions can be in one sentence only if you do them at the same time.
- If an instruction starts with a condition, put a comma after the condition. Example: "If the server does not start, read the log."
- Use notes to give information, not instructions.

## Descriptive text

- Give information gradually.
- Write about one topic in each paragraph.
- Write a maximum of six sentences in each paragraph.

## Cautions

Use a caution to tell the reader about a risk of data loss or a security risk. Start the caution with a clear command.

> CAUTION: Keep your recovery code secret. A person with the code can use your account.

## The STE check

The `stelint` test checks all Markdown files in the repository. It finds these problems:

- Sentences with too many words.
- Paragraphs with more than six sentences.
- Semicolons.
- Contractions.
- The words in the left column of the word table.

The check ignores code spans and code blocks. When you write about a word itself, put the word in a code span, for example `ensure`.

To run the check, use this command:

    cargo test -p stelint

The check cannot find all STE problems. A person must also read each document.

## Technical names

| Name | Meaning |
| --- | --- |
| access level, access | The rights of a handle on a host: guest, user, or admin. |
| account | The identity of a person on a server. A key pair identifies the account. |
| alert level | The state of alarm of a corporation: green, yellow, orange, or red. |
| analyst | An NPC who works in a SOC. |
| arena | A network for fights between handles: duels and crew matches. Each fight gets its own copy. |
| audit log | The file where the server writes each action of the moderators. |
| base | A host in an arena where one side of a fight starts. |
| blueprint | Know-how that lets a handle compile one version of a program. |
| bridge | A way into a vault: the sync of a maintenance laptop, a USB drive, or an old modem line. |
| budget | How much a corporation spends on security: low, medium, high, or top. |
| burn list | The public list of the handles that burned on a server. |
| channel | A chat room for the players of a server, on all hosts. |
| city file | The content file of the city of a server: `city.ini` in the data directory, or Port Ostra in the binary. |
| contract | A job from a fixer. |
| copy | The state of a target network for one run, or for the runs of one crew: its ICE, analysts, logs, and bridges. |
| corp | A corporation in a content file. |
| crate | A Rust package. |
| credentials | Stolen login data that raises the access of a handle in a network. |
| credit, cr | The money of the city. |
| crew | A group of up to 4 handles, with a leader, a channel, and a reputation. |
| crewmate | A handle in the same crew. |
| cycle | One unit of CPU time on a rig. |
| darknet | A network without law that all handles share. Other handles can attack you there, and noise gives no trace. |
| display layer | One level of the client display: line, edit, color, or HUD. |
| duel | A fight between two handles in an arena. |
| fence | An NPC who buys stolen data. |
| fixer | An NPC who gives contracts. |
| flag | A file on a base in a crew match. A crew scores when it carries the flag of the other crew to its own base. |
| habit | How an analyst works: careful, lazy, or strict. |
| handle | A character under an account. A handle can burn. |
| headline | One item of the news feed. |
| heat | How much the police know about a handle, from 0 to 5. |
| heist | A large contract for a crew, with more than one stage. |
| hint | A help text for the next step of a contract. |
| hole | A weakness in a software family. The `exploit` program uses it until the corporation patches the family. |
| home host | The private hub host where a handle repairs the rig, changes the RAM, and uses the stash. |
| honeypot | A bait file in a target. A read or a download of it raises the trace. |
| host key | The SSH key of a server. An SSH client shows its fingerprint at the first connection. |
| host | A computer in the game world. It is the TTYmud equivalent of a room. |
| hub | A safe zone that an author makes by hand. |
| HUD | The client display that fills the window, with two status bars, an output pane, a chat pane, and the input line. |
| ICE | Hostile security software in a target. |
| integrity | The health of a rig, from 100 to 0. |
| job board, board | The list of the jobs that the fixers post each day. |
| know-how | The programs and blueprints that an account keeps after a burn. |
| layer | One level of a target: the edge, the LAN, the core, or the vault. |
| link | A connection between two servers that list each other in `links.ini`. It carries the linked channel and the headlines. |
| link code | A code for one use that adds a new device to an account. |
| linked channel | The one channel that reaches the players of all linked cities. In Port Ostra, it is `#longline`. |
| linked city | A city on a different server that has a link to this server. |
| loot | Data and items that a handle takes from a target. |
| lot | One item of the police auction. |
| maintenance laptop, laptop | A LAN host of a heist target that syncs with the vault at times. |
| match | A game of capture the flag between two crews in an arena. |
| mod | A change to a program, for example the quiet mod: 1 less noise for 1 more cycle. |
| moderator | A player who can mute, kick, and ban other players. The operator selects the moderators. |
| news feed, news | The public list of the latest events in a city. |
| noise | The evidence that an action leaves in a target. Noise increases the trace. |
| NPC | A character that the server controls. |
| offshore stash, stash | A store of credits and items that stays after a burn. |
| pane | A part of the HUD that shows one type of text. |
| pipeline | Two or more commands that the pipe character (`\|`) joins. |
| plant | A job goal: put a false file on a host of the target. |
| police auction, auction | The sale of the valuable items from the rigs of burned handles. |
| program | Software that a handle runs to do an action. |
| recovery code | A secret code that gets an account back on a new device. |
| report | A message to the moderators about a player. It keeps the recent chat of that player. |
| reputation | The trust of a fixer in a handle. More reputation opens bigger jobs. |
| rig | The computer of a handle. |
| route | A network path from one host to a different host. |
| rumor | A hint from a fixer about a corporation. A rumor can be true, old, or false. |
| run | One visit of a handle to a target, from entry to exit. |
| script | A file on a rig with one command line on each line. Its name ends in `.sh`. The `sh` command runs it, one command on each tick. |
| security level | The strength of the security of a host: low, medium, high, or top. |
| server list, list | The public list of servers in the file `servers.ini`. The client and the website show it. |
| shift | The time when a group of analysts works in a SOC: the day shift or the night shift. |
| shop | A place on a hub host or in a darknet that sells programs, blueprints, and hardware. |
| SOC | The security operations center of a corporation. |
| software family, software | The software that a host runs, for example a mail server. An exploit attacks one software family. |
| starter kit, kit | The first programs of a new handle. A kit sets the first role of the handle. |
| step | One part of a contract. A step teaches one command. |
| target | A network that a handle attacks. |
| telegraph | A message that shows the next action of an ICE before it acts. |
| template | A type of host for the targets that the server makes. |
| tick | One step of game time, about two seconds. |
| trace | How near the SOC is to the location of a handle, from 0% to 100%. |
| vault | The last layer of a heist target. It has no route, so a crew needs a bridge. |
| version | The strength of a program, from v1 to v3. A higher version makes the main effect of the program stronger. |
| wager | The credits that the loser of a fight pays to the winner. |
| weak point | A flaw of an ICE that `probe` finds. |
| wipe | A job goal: delete a file on a host of the target. |
| workspace | A set of Rust crates that build together. |
| world seed | The number that sets the job board and the targets of a server. |
| zone | The type of a network: target, darknet, or arena. |

## Technical verbs

| Verb | Meaning |
| --- | --- |
| ban | To refuse an account on a server, and as an option its IP address. |
| bid | To offer credits for a lot of the police auction. |
| burn | To lose a handle permanently at maximum heat. |
| compile | To make the next version of a program from a blueprint, or to add a mod. |
| crack | To break ICE or encryption. |
| decrypt | To make encrypted data readable. |
| download | To copy a file from a host to a rig. |
| encrypt | To make data unreadable without a key. |
| exploit | To use a weakness in software to get more access. |
| hack | To get access to a computer without permission. |
| jack out | To drop the link to a target at once. |
| kick | To disconnect a player from a server. |
| link | To add a device to an account with a link code. |
| load | To put a program into the RAM of a rig. |
| mute | To stop the chat messages of a player for some minutes. |
| patch | To repair a weakness or damage in software. |
| probe | To find the weaknesses of a host or of ICE, or to find a honeypot. |
| recover | To get an account back with a recovery code. |
| revoke | To remove a key from an account. |
| sanitize | To remove unsafe characters from text. |
| scan | To find hosts and services. |
| scroll | To move the text in a pane up or down. |
| scrub | To delete log lines. |
| tip off | To tell the police about a handle. It adds heat to that handle. |
| unload | To remove a program from the RAM of a rig. |
| upload | To copy a file from a rig to a host. |
