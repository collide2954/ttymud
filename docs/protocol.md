# Protocol

This document gives the protocol between the TTYmud client and the TTYmud server. It is protocol version 1.

## Transport

The client connects to the server with TCP. The default port is 7331.

Then the two sides do a Noise handshake with the pattern `Noise_XX_25519_ChaChaPoly_BLAKE2s`. The prologue is the ASCII text `ttymud/1`. If the two sides use different prologues, the handshake fails. The server closes each handshake that takes more than 10 seconds.

After the handshake, each side knows the static public key of the other side. The client compares the server key with its list of known servers. The server uses the client key as the identity of the player.

## Frames

Each Noise message goes in one frame. A frame has these parts:

1. The length of the encrypted message, in two bytes. The high byte is first.
2. The encrypted message.

The maximum length of an encrypted message is 65535 bytes. The encryption adds 16 bytes. As a result, the maximum length of a protocol message is 65519 bytes.

## Encoding

Each protocol message starts with a type byte. The fields of the message follow the type byte, in the order that this document gives.

| Field | Encoding |
| --- | --- |
| Byte | One byte. |
| Number | An unsigned integer in the LEB128 format, with a maximum of 10 bytes. |
| Text | A number that gives the length in bytes, then the UTF-8 bytes. |
| List | A number that gives the count of items, then the items. |

A count or a length cannot be larger than the number of bytes that remain in the message. As a result, a bad message cannot make the receiver use much memory. A message with bytes after its last field is not valid.

## Client messages

| Type | Name | Fields | Use |
| --- | --- | --- | --- |
| 1 | Command | Text: the input line. | Run one input line. |
| 2 | Complete | Text: the input before the cursor. | Ask for tab completion. |

## Server messages

| Type | Name | Fields | Use |
| --- | --- | --- | --- |
| 1 | Welcome | Text: the city. Text: the handle, or a temporary name. | The first message after the handshake. The server sends it again when a link code or a recovery code changes the account. |
| 2 | Text | Byte: the kind. List: the spans. | One line of game text. |
| 3 | Prompt | Text: the prompt. | The prompt for the next input line. |
| 4 | Close | Text: the reason. | The last message before the server closes the connection. |
| 5 | Candidates | Text: the word. List: the names, as texts. | The answer to a Complete message. |
| 6 | Refresh | Text: the prompt. | A new prompt after text that is not an answer to a command. |
| 7 | Status | The fields of the status. | The state of the handle, for a status bar. |

A Status message has these fields, in this order:

1. Text: the host.
2. Byte: 1 during a run, or 0 at other times.
3. Only during a run: Text: the network. Text: the access. Number: the trace. Text: the alert level of the corporation.
4. Numbers: the integrity, the heat, the maximum heat, the RAM in use, the RAM in total, the cycles, the maximum cycles, and the credits.
5. Text: the latest headline of the news.
6. List: the crewmates in the same copy of the network. Each item is a text, the handle, and then a number, the trace.

The alert level is green, yellow, orange, or red. If no corporation owns the network, the alert level is an empty text. If there is no news, the headline is an empty text. Outside a run, the list of crewmates is empty.

A different run byte is not valid. Each number in a Status message must be smaller than 4294967296.

A span has two fields: a byte for the style, and a text.

| Kind | Byte |
| --- | --- |
| Game | 0 |
| Error | 1 |
| Chat | 2 |
| Telegraph | 3 |

The Telegraph kind is for ICE. A telegraph tells the next action of an ICE before the ICE does it. An attack of an ICE also has this kind.

| Style | Byte |
| --- | --- |
| Plain | 0 |
| Title | 1 |
| Host | 2 |
| File | 3 |
| Command | 4 |
| Player | 5 |
| Ice | 6 |
| Danger | 7 |

A kind byte or a style byte with a different value is not valid.

## Rules

- After the Welcome message, the server sends the first text and a Prompt message. For an account without a handle, the text asks for a handle. For an account with a handle, the text is the scene of the home host.
- An account without a handle has a temporary name: `anon-` and the first eight hexadecimal characters of its key.
- After the answer to each Command message, the server sends a Prompt message. The client can count the prompts to know when all answers are in.
- Some text comes without a command, for example the actions of ICE on a tick or the chat of other players. After this text, the server sends a Refresh message. A client shows a Refresh message as a prompt, but it does not count it as an answer.
- The server does not send a Prompt message after a Candidates message.
- If the state of a handle changes, the server sends a Status message before the next Prompt or Refresh message. After a tick, it also sends a Status message to each handle whose state changed. An account without a handle gets no Status message.
- The word in a Candidates message is the end of the input that a name can replace. Each name starts with the word. This comparison ignores case.
- If a message is not valid, the receiver closes the connection.
- Text from the server can contain unsafe characters. The client must sanitize all text before it shows the text.

## Limits

The server applies these limits to each connection.

| Limit | Value |
| --- | --- |
| Commands | 20 commands quickly, then a maximum of 10 each second. |
| Chat | 5 messages quickly, then a maximum of 1 every 2 seconds. |
| Complete messages | The same limit as commands. |
| Time to write a frame | 10 seconds. |
| Messages on a link | 40 messages quickly, then a maximum of 4 each second. The server ignores the other messages. |
| Time to write a frame on a link | 30 seconds. |

If a command goes above a limit, the server does not run it. The server sends a Text message with the kind Error. If a Complete message goes above the limit, the server does not answer it.

## Close

The server sends a Close message when it ends a connection. For example, it ends the old connection when the same account connects again. It also ends a connection after a kick or a ban. A banned or revoked key gets only a Close message.

After the Close message, the server stops all writes. Then it waits a maximum of 5 seconds for the client to close the connection. As a result, the connection does not reset before the client reads the Close message.

## Links between servers

Two servers can make a link between their cities. Each operator adds the other server to the file `links.ini`, with its address and its full key. The [operator guide](operator.md#linked-cities) tells how.

A link uses the same port and the same Noise handshake as a client. The server with the lower key connects to the other server. It compares the key of the other server with the key in its `links.ini`. The other server knows a linked server by its key. A key that is not in its `links.ini` gets a Welcome message, as a client does.

After the handshake, the server that accepts the connection sends a Hello message. Then the server that connects sends a Hello message. After that, each side can send these messages at any time.

| Type | Name | Fields | Use |
| --- | --- | --- | --- |
| 32 | Hello | Text: the name of the city. | The first message of each side. |
| 33 | Chat | Text: the handle. Text: the message. | One message in the linked channel. |
| 34 | News | Number: the time, in seconds since 1970. Text: the headline. | One headline of the city. |
| 35 | Ping | None. | It shows that the link still works. |

These rules apply to a link:

- The types of link messages are different from the types of server messages. If the other server does not list this server, the first message is a Welcome message, and the link stops at once.
- When a link starts, each side sends its 10 latest headlines. After that, it sends each new headline.
- If a side has nothing to send for 60 seconds, it sends a Ping message. If a side gets no message for 150 seconds, it closes the link.
- A side ignores a message type that it does not know. A second Hello message, or a message that is not valid, closes the link.
- The receiving side sanitizes each text. It ignores a Chat message from a handle with a name that is not valid.
- If a link stops, the server that connects tries again after 5 seconds. After each failure, it waits two times longer, to a maximum of 5 minutes.
