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
| blueprint | Know-how that lets a handle compile a program. |
| budget | How much a corporation spends on security: low, medium, high, or top. |
| burn list | The public list of the handles that burned on a server. |
| contract | A job from a fixer. |
| corp | A corporation in a content file. |
| crate | A Rust package. |
| credentials | Stolen login data that raises the access of a handle in a network. |
| credit, cr | The money of the city. |
| crew | A group of players. |
| cycle | One unit of CPU time on a rig. |
| fence | An NPC who buys stolen data. |
| fixer | An NPC who gives contracts. |
| handle | A character under an account. A handle can burn. |
| heat | How much the police know about a handle, from 0 to 5. |
| heist | A large contract for a crew, with more than one stage. |
| hint | A help text for the next step of a contract. |
| home host | The private hub host where a handle repairs the rig, changes the RAM, and uses the stash. |
| host | A computer in the game world. It is the TTYmud equivalent of a room. |
| hub | A safe zone that an author makes by hand. |
| ICE | Hostile security software in a target. |
| integrity | The health of a rig, from 100 to 0. |
| job board, board | The list of the jobs that the fixers post each day. |
| know-how | The programs and blueprints that an account keeps after a burn. |
| layer | One level of a target: the edge, the LAN, or the core. Later, also the vault. |
| loot | Data and items that a handle takes from a target. |
| noise | The evidence that an action leaves in a target. Noise increases the trace. |
| NPC | A character that the server controls. |
| offshore stash, stash | A store of credits and items that stays after a burn. |
| pipeline | Two or more commands that the pipe character (`\|`) joins. |
| program | Software that a handle runs to do an action. |
| reputation | The trust of a fixer in a handle. More reputation opens bigger jobs. |
| rig | The computer of a handle. |
| route | A network path from one host to a different host. |
| run | One visit of a handle to a target, from entry to exit. |
| security level | The strength of the security of a host: low, medium, high, or top. |
| shop | A place on a hub host that sells programs and hardware. |
| SOC | The security operations center of a corporation. |
| starter kit, kit | The first programs of a new handle. A kit sets the first role of the handle. |
| step | One part of a contract. A step teaches one command. |
| target | A network that a handle attacks. |
| telegraph | A message that shows the next action of an ICE before it acts. |
| template | A type of host for the targets that the server makes. |
| tick | One step of game time, about two seconds. |
| trace | How near the SOC is to the location of a handle, from 0% to 100%. |
| weak point | A flaw of an ICE that `probe` finds. |
| workspace | A set of Rust crates that build together. |
| world seed | The number that sets the job board and the targets of a server. |

## Technical verbs

| Verb | Meaning |
| --- | --- |
| burn | To lose a handle permanently at maximum heat. |
| compile | To make a program from a blueprint and parts. |
| crack | To break ICE or encryption. |
| decrypt | To make encrypted data readable. |
| encrypt | To make data unreadable without a key. |
| exploit | To use a weakness in software to get more access. |
| hack | To get access to a computer without permission. |
| jack out | To drop the link to a target at once. |
| load | To put a program into the RAM of a rig. |
| patch | To repair a weakness or damage in software. |
| probe | To find the weaknesses of a host or of ICE. |
| sanitize | To remove unsafe characters from text. |
| scan | To find hosts and services. |
| scrub | To delete log lines. |
| unload | To remove a program from the RAM of a rig. |
