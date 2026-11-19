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
| 1 | Welcome | Text: the city. Text: the name of the player. | The first message after the handshake. |
| 2 | Text | Byte: the kind. List: the spans. | One line of game text. |
| 3 | Prompt | Text: the prompt. | The prompt for the next input line. |
| 4 | Close | Text: the reason. | The last message before the server closes the connection. |
| 5 | Candidates | Text: the word. List: the names, as texts. | The answer to a Complete message. |

A span has two fields: a byte for the style, and a text.

| Kind | Byte |
| --- | --- |
| Game | 0 |
| Error | 1 |
| Chat | 2 |

| Style | Byte |
| --- | --- |
| Plain | 0 |
| Title | 1 |
| Host | 2 |
| File | 3 |
| Command | 4 |
| Player | 5 |

A kind byte or a style byte with a different value is not valid.

## Rules

- After the Welcome message, the server sends the scene of the start host and a Prompt message.
- After the answer to each Command message, the server sends a Prompt message. The client can count the prompts to know when all answers are in.
- The server does not send a Prompt message after a Candidates message.
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

If a command goes above a limit, the server does not run it. The server sends a Text message with the kind Error. If a Complete message goes above the limit, the server does not answer it.

## Close

The server sends a Close message when it ends a connection. For example, it ends the old connection when the same key connects again. After the Close message, the server stops all writes. Then it waits a maximum of 5 seconds for the client to close the connection. As a result, the connection does not reset before the client reads the Close message.
