# Interface

This document tells how the TTYmud client shows the game in a terminal. It also tells how a new player learns the game.

## Layers

The client has display layers. Each layer adds features to the layer below it. The client uses the highest layer that the terminal supports. The player can select a lower layer with the `--layer` option or in the settings file.

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
- Tab to complete commands, routes, files, programs, ICE, shop items, help topics, channels, and handles.
- Ctrl+C to clear the line, and Ctrl+D to delete a character. On an empty line, these keys quit.

A long input line moves to the side, so that the cursor stays on the screen. The line holds a maximum of 400 characters, as many as the server reads, and the history keeps the last 100 lines. The `--line` option selects line mode in a terminal. The [player guide](../player.md) gives all the keys.

## Color mode

The client finds the color depth of the terminal: no color, 16 colors, 256 colors, or true color. Text uses the 16 colors of the terminal theme. As a result, text looks correct on light themes and on dark themes. In the HUD, 256 colors and true color give finer shades to the bars and the borders.

The client obeys the `NO_COLOR` environment variable. Color never gives information alone. A word or a symbol always gives the same information.

## HUD mode

The HUD uses the full terminal window:

    ┌ arcadia-db · Arcadia Bank · user · red alert ── trace ▓▓▓░░░░░░░ 31% ┐
    │ The ledger server hums behind two firewalls.                         │
    │ Sentry v2 watches port 8443.                                         │
    │ $ probe sentry | crack                                               │
    │ crack: overflow lands. Sentry integrity 40 -> 12                     │
    │ Sentry v2 starts to compile TRACE BURST (2 ticks)                    │
    ├ chat ──────────────────────────────────────────── nyx 34% · zer0 12% ┤
    │ [ostra] <nyx> arcadia patched ledgerbase. anyone got a new hole?     │
    ├ news: Arcadia Bank raises its alert to red. ─────────────────────────┤
    │ ghost@arcadia-db $ _                                                 │
    └ integ 82 · heat ▲▲△△△ · ram 6/8 GB · cycles ●●●○ · 1250 cr ──────────┘

The HUD has these parts:

- The top bar shows the host. During a run, it also shows the network, the access, the alert level of the corporation, and the trace.
- The output pane shows the game text. It keeps the last 1,000 lines.
- The chat pane shows `say` messages, channel messages, and private messages. During a group run, the line above it shows each crewmate in the run and the trace of that crewmate.
- The line above the input line shows the latest headline of the news.
- The input line shows the prompt and the line editor.
- The bottom bar shows the integrity, the heat, the RAM, the cycles, and the credits.

The color of the alert level goes from green to red. In a narrow window, the top bar cuts the end of its left part. As a result, the alert level is the first part to go.

PgUp and PgDn scroll the output pane. When the pane shows older text, the line below the pane shows "more below", and new text does not move the pane. A new command moves the pane back to the newest text.

The HUD changes its layout when the window size changes. A window with fewer than 60 columns gets no side borders. A window with fewer than 20 rows gets no chat pane, and the chat goes into the output pane. If the window is smaller than 40 columns or 10 rows, the HUD asks for a larger window.

The HUD uses the alternate screen of the terminal. When the client stops, the terminal shows its earlier text again.

## Terminal detection

The client examines these items:

- The `TERM` environment variable. A `dumb` terminal gets line mode.
- The `COLORTERM`, `NO_COLOR`, and `WT_SESSION` environment variables, for the color depth.
- The locale in `LC_ALL`, `LC_CTYPE`, or `LANG`, to find if the terminal can show UTF-8.
- On Windows, the console mode, to find if the console supports virtual terminal sequences.

If the terminal cannot show UTF-8, the client uses ASCII for all symbols and lines. The player can set each item with a command-line option or in the settings file:

    ttymud connect ostra.example.net --layer color --color 256 --ascii

The settings file is `settings.ini` in the configuration directory. An option on the command line has priority over the file. The client never uses a layer that the terminal cannot show.

## Over SSH

With the `ssh` feature, the server also lets a player use a normal SSH client. Then the server runs the display of the client for the SSH terminal: line mode, edit mode, color mode, or the HUD. The same display code works for both ways in.

- The SSH client sends the terminal type, the window size, and some environment variables. The server uses them as the client uses its own environment. Without a locale, the server uses UTF-8.
- The words after the address select the layer, ASCII or UTF-8, and verbose output, for example `ssh -t -p 7332 ostra.example.net edit ascii`.
- Without a terminal, for example with `ssh -T`, the terminal of the player edits the lines, so the server uses line mode. With a terminal, the server edits the line, so the lowest layer is edit mode.

## Text from the server

The server does not send terminal escape codes. It sends text with style marks, for example `danger`, `chat`, or `host`. The client changes the style marks into the correct codes for the terminal. As a result, the same game data works on all terminals. Also, another player cannot send escape codes to your terminal.

The server does not wrap text. The client wraps text to the width of the terminal.

Each message has a kind, for example game, chat, telegraph, or system. The HUD uses the kind to put each message in the correct pane.

## Accessibility

TTYmud supports screen readers.

- Line mode does not redraw the screen and does not show animations.
- The prompt and the status use words, not only symbols.
- The `verbose` setting tells the client to write important changes of state as text lines, for example "Trace is now 50 percent." The client writes a line when a run starts or ends. It writes a line when the trace, the access, the alert level, the integrity, or the heat changes. It also writes each new headline of the news, and each crewmate who comes into the run or leaves it.
- When the `verbose` setting is on, each telegraph starts with the word "Alert". As a result, a screen reader user can recognize a telegraph immediately.

## First run

The first run must teach the game without a long tutorial.

1. The client makes an identity key and shows its fingerprint.
2. The client connects to the server. For a server on the public list, it checks the key from the list. For a different new server, it shows the fingerprint of the server.
3. The player selects a handle name and a starter kit.
4. A fixer gives a tutorial contract against a weak target: the network of a small dental office.
5. Each step of the tutorial contract teaches one command. The game gives a hint only when the player makes no progress for some time.
6. The player can stop the tutorial contract at any time.

In version 0.1, the server asks for the handle name first, and then for the starter kit. The fixer Pell offers the tutorial contract to each new handle. The `contract` command shows the next step. After 15 ticks without progress, the game shows the hint one time. The game skips a step for a program that the handle does not know. The `contract drop` command stops the contract.

The offer of the tutorial contract tells the player about the job board. After the tutorial, the player types `jobs` in the Back Room to see more work.

## Help

- `help` shows the most important commands.
- `help <topic>` shows the help for a command or a topic. The topics are `heat`, `ice`, `pipes`, and `runs`.
- `?` shows the commands that you can use on the current host.
- An unknown command gives the most likely correct command.
- Each error message tells what went wrong and what to do.
