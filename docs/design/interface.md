# Interface

This document tells how the TTYmud client shows the game in a terminal. It also tells how a new player learns the game.

## Layers

The client has layers. Each layer adds features to the layer below it. The client uses the highest layer that the terminal supports. The player can select a lower layer at any time.

| Layer | Features | Necessary terminal features |
| --- | --- | --- |
| Line | Lines of text and a prompt | None |
| Edit | A local line editor, a history, and tab completion | Raw mode |
| Color | Colors and text styles | ANSI colors |
| HUD | A status bar, a chat pane, and an output pane | Cursor movement and a known window size |

## Line mode

Line mode is the base of TTYmud. It works on all terminals: in tmux and screen, on dumb terminals, over serial lines, and with screen readers. In line mode, the client only writes lines and reads lines. It does not move the cursor.

The prompt shows the most important state:

    [trace 31% integ 82] ghost@arcadia-db $

Outside a target network, the prompt shows only the handle and the host. When a tick changes the state, the server sends a new prompt.

## Edit mode

When the input and the output are terminals, the client edits the input line locally. As a result, keys show immediately, also on a slow connection. Text from the server shows above the input line. The editor has these keys:

- Left, Right, Home, End, Backspace, and Delete.
- Ctrl+A, Ctrl+E, Ctrl+B, Ctrl+F, Ctrl+W, Ctrl+U, and Ctrl+K, as in a shell.
- Up and Down, or Ctrl+P and Ctrl+N, for the command history.
- Tab to complete commands, routes, files, and help topics. Later, Tab will also complete handles.
- Ctrl+C to clear the line, and Ctrl+D to delete a character. On an empty line, these keys quit.

A long input line moves to the side, so that the cursor stays on the screen. The `--line` option selects line mode in a terminal. The [player guide](../player.md) gives all the keys.

## Color mode

The client finds the color depth of the terminal: no color, 16 colors, 256 colors, or true color. It uses the best palette for that depth. It obeys the `NO_COLOR` environment variable. Color never gives information alone. A word or a symbol always gives the same information.

## HUD mode

The HUD uses the full terminal window:

    ┌ arcadia-db · LAN · user ──────── trace ▓▓▓░░░░ 31% ┐
    │ The ledger server hums behind two firewalls.       │
    │ Sentry v2 watches port 8443.                       │
    │ > probe sentry | crack                             │
    │   crack: overflow lands. Sentry integrity 40 -> 12 │
    │ Sentry v2 starts to compile TRACE BURST (2 ticks)  │
    ├ #relay ────────────────────────────────────────────┤
    │ <nyx> arcadia patched crack v2. anyone got v3?     │
    ├────────────────────────────────────────────────────┤
    │ ghost@arcadia-db $ _                               │
    └ integ 82 · heat ▲▲△△△ · ram 6/8 GB · cycles ●●●○ ──┘

The HUD changes its layout when the window size changes. If the window is too small, the client goes back to line mode.

## Terminal detection

The client examines these items:

- The `TERM`, `COLORTERM`, and `NO_COLOR` environment variables.
- The locale, to find if the terminal can show UTF-8.
- On Windows, the console mode, to find if the console supports virtual terminal sequences.
- The window size.

If the terminal cannot show UTF-8, the client uses ASCII for all symbols and lines. The player can set each item with a command-line option or in the configuration file:

    ttymud --layer line --color none --ascii

## Text from the server

The server does not send terminal escape codes. It sends text with style marks, for example `danger`, `chat`, or `host`. The client changes the style marks into the correct codes for the terminal. As a result, the same game data works on all terminals. Also, another player cannot send escape codes to your terminal.

The server does not wrap text. The client wraps text to the width of the terminal.

Each message has a kind, for example game, chat, telegraph, or system. The HUD uses the kind to put each message in the correct pane.

## Accessibility

TTYmud supports screen readers.

- Line mode does not redraw the screen and does not show animations.
- The prompt and the status use words, not only symbols.
- The `verbose` setting tells the client to write important changes of state as text lines, for example "Trace is now 50 percent."
- When the `verbose` setting is on, each telegraph starts with the word "Alert". As a result, a screen reader user can recognize a telegraph immediately.

## First run

The first run must teach the game without a long tutorial.

1. The client makes an identity key and shows its fingerprint.
2. The client connects to the server and shows the fingerprint of the server.
3. The player selects a handle name and a starter kit.
4. A fixer gives a tutorial contract against a weak target: the network of a small dental office.
5. Each step of the tutorial contract teaches one command. The game gives a hint only when the player makes no progress for some time.
6. The player can stop the tutorial contract at any time.

In version 0.1, handles and starter kits do not exist yet. They come in M4. The fixer Pell offers the tutorial contract when the player connects. The `contract` command shows the next step. After 15 ticks without progress, the game shows the hint one time. The `contract drop` command stops the contract.

## Help

- `help` shows the most important commands.
- `help <topic>` shows the help for a command or a topic. The topics are `ice`, `pipes`, and `runs`.
- `?` shows the commands that you can use on the current host.
- An unknown command gives the most likely correct command.
- Each error message tells what went wrong and what to do.
