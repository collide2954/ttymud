use std::collections::VecDeque;
use std::io::{self, Write};
use std::time::Instant;

use ttymud_text::{Kind, Message, Status, Style};

use crate::Shown;
use crate::canvas::{Canvas, Ink};
use crate::editor::{Action, Completed, Editor, Key};
use crate::session::{Typed, View};
use crate::settings::{Depth, Settings};
use crate::threats::{Threat, Threats};
use crate::wrap::{Row, columns, wrap};

const KEEP: usize = 1_000;
const MIN_WIDTH: usize = 40;
const MIN_HEIGHT: usize = 10;
const BORDERS: usize = 60;
const CHAT: usize = 20;
const BAR: usize = 10;
const SYMBOLS: u32 = 12;
const WHEEL: usize = 3;
const ENTER: &str = "\x1b[?1049h\x1b[?7l\x1b[2J";
const LEAVE: &str = "\x1b[0m\x1b[?7h\x1b[?1049l\x1b[?25h";
const MOUSE_ON: &str = "\x1b[?1000h\x1b[?1006h";
const MOUSE_OFF: &str = "\x1b[?1006l\x1b[?1000l";
const SMALL: &str = "Make the window larger.";

struct Glyphs {
    line: char,
    side: char,
    top: (char, char),
    middle: (char, char),
    bottom: (char, char),
    full: char,
    empty: char,
    hot: char,
    cool: char,
    cycle: char,
    spent: char,
    dot: char,
    down: char,
}

static UNICODE: Glyphs = Glyphs {
    line: '─',
    side: '│',
    top: ('┌', '┐'),
    middle: ('├', '┤'),
    bottom: ('└', '┘'),
    full: '▓',
    empty: '░',
    hot: '▲',
    cool: '△',
    cycle: '●',
    spent: '○',
    dot: '·',
    down: '▼',
};

static ASCII: Glyphs = Glyphs {
    line: '-',
    side: '|',
    top: ('+', '+'),
    middle: ('+', '+'),
    bottom: ('+', '+'),
    full: '#',
    empty: '.',
    hot: '^',
    cool: '.',
    cycle: '*',
    spent: '.',
    dot: '|',
    down: 'v',
};

#[derive(Debug, Clone, Copy)]
struct Layout {
    width: usize,
    height: usize,
    borders: bool,
    output: usize,
    chat: usize,
}

impl Layout {
    fn new(width: usize, height: usize) -> Option<Self> {
        if width < MIN_WIDTH || height < MIN_HEIGHT {
            return None;
        }
        let rest = height - 4;
        let chat = if height >= CHAT {
            (rest / 4).clamp(3, 8)
        } else {
            0
        };
        Some(Self {
            width,
            height,
            borders: width >= BORDERS,
            output: rest - chat - usize::from(chat > 0),
            chat,
        })
    }

    fn left(self) -> usize {
        if self.borders { 2 } else { 0 }
    }

    fn room(self) -> usize {
        self.width - 2 * usize::from(self.borders)
    }

    fn inner(self) -> usize {
        self.width - 2 * self.left()
    }

    fn rules(self) -> [usize; 4] {
        [0, self.output + 1, self.height - 3, self.height - 1]
    }
}

pub(crate) struct Hud<W: Write> {
    output: W,
    depth: Depth,
    glyphs: &'static Glyphs,
    width: usize,
    height: usize,
    editor: Editor,
    prompt: String,
    start: usize,
    asked: Option<String>,
    lines: VecDeque<Message>,
    scroll: usize,
    status: Option<Status>,
    shown: Option<Canvas>,
    closed: Option<String>,
    open: bool,
    mouse: bool,
    threats: Threats,
    fixed: Option<Instant>,
}

impl<W: Write> Hud<W> {
    pub(crate) fn new(output: W, settings: Settings, width: u16, height: u16) -> io::Result<Self> {
        let mut hud = Self {
            output,
            depth: settings.color,
            glyphs: if settings.ascii { &ASCII } else { &UNICODE },
            width: usize::from(width),
            height: usize::from(height),
            editor: Editor::default(),
            prompt: String::new(),
            start: 0,
            asked: None,
            lines: VecDeque::new(),
            scroll: 0,
            status: None,
            shown: None,
            closed: None,
            open: true,
            mouse: false,
            threats: Threats::default(),
            fixed: None,
        };
        hud.output.write_all(ENTER.as_bytes())?;
        hud.draw()?;
        Ok(hud)
    }

    pub(crate) fn capture_mouse(&mut self) -> io::Result<()> {
        self.mouse = true;
        self.output.write_all(MOUSE_ON.as_bytes())?;
        self.output.flush()
    }

    fn add(&mut self, message: Message) {
        if self.scroll > 0
            && let Some(layout) = Layout::new(self.width, self.height)
            && (layout.chat == 0 || message.kind != Kind::Chat)
        {
            self.scroll += wrap(&message, layout.inner()).len();
        }
        self.lines.push_back(message);
        if self.lines.len() > KEEP {
            self.lines.pop_front();
        }
    }

    fn page(&self) -> usize {
        Layout::new(self.width, self.height)
            .map_or(1, |layout| layout.output.saturating_sub(1).max(1))
    }

    fn draw(&mut self) -> io::Result<()> {
        let mut canvas = Canvas::new(self.width, self.height);
        let layout = Layout::new(self.width, self.height);
        let cursor = layout.map(|layout| self.paint(&mut canvas, layout));
        if cursor.is_none() {
            canvas.put(0, 0, SMALL, Ink::Plain, self.width);
        }
        let changes = canvas.changes(self.shown.as_ref(), self.depth);
        write!(self.output, "\x1b[?25l{changes}")?;
        if let Some((x, y)) = cursor {
            write!(self.output, "\x1b[{};{}H\x1b[?25h", y + 1, x + 1)?;
        }
        self.shown = Some(canvas);
        self.output.flush()
    }

    fn paint(&mut self, canvas: &mut Canvas, layout: Layout) -> (usize, usize) {
        self.frame(canvas, layout);
        self.top_bar(canvas, layout);
        self.bottom_bar(canvas, layout);
        self.notice(canvas, layout);
        let chat = layout.chat > 0;
        let game = self
            .lines
            .iter()
            .filter(|message| !chat || message.kind != Kind::Chat);
        self.scroll = pane(canvas, layout, game, 1, layout.output, self.scroll);
        let mut label = Vec::new();
        if chat {
            label.push((" chat ".to_owned(), Ink::Dim));
            let talk = self
                .lines
                .iter()
                .filter(|message| message.kind == Kind::Chat);
            pane(canvas, layout, talk, layout.output + 2, layout.chat, 0);
        }
        let mut more = self.crew_marks();
        if self.scroll > 0 {
            let down = self.glyphs.down;
            more.push((format!(" {down} more below (PgDn) "), Ink::Strong));
        }
        bar(canvas, layout, layout.output + 1, &label, &more);
        self.input(canvas, layout)
    }

    fn crew_marks(&self) -> Vec<(String, Ink)> {
        let crew = self.status.iter().flat_map(|status| &status.crew);
        let mut marks = Vec::new();
        for (index, mate) in crew.enumerate() {
            let gap = if index == 0 {
                " ".to_owned()
            } else {
                format!(" {} ", self.glyphs.dot)
            };
            let trace = mate.trace.min(100);
            marks.push((gap, Ink::Dim));
            marks.push((format!("{} ", mate.name), Ink::Plain));
            marks.push((format!("{trace}%"), Ink::Level(trace)));
        }
        if !marks.is_empty() {
            marks.push((" ".to_owned(), Ink::Plain));
        }
        marks
    }

    fn frame(&self, canvas: &mut Canvas, layout: Layout) {
        let glyphs = self.glyphs;
        let width = layout.width;
        let line = glyphs.line.to_string().repeat(width);
        let ends = [glyphs.top, glyphs.middle, glyphs.middle, glyphs.bottom];
        for (y, (left, right)) in layout.rules().into_iter().zip(ends) {
            canvas.put(0, y, &line, Ink::Dim, width);
            if layout.borders {
                canvas.put(0, y, &left.to_string(), Ink::Dim, 1);
                canvas.put(width - 1, y, &right.to_string(), Ink::Dim, 1);
            }
        }
        if layout.borders {
            let side = glyphs.side.to_string();
            for y in (1..layout.height - 1).filter(|y| !layout.rules().contains(y)) {
                canvas.put(0, y, &side, Ink::Dim, 1);
                canvas.put(width - 1, y, &side, Ink::Dim, 1);
            }
        }
    }

    fn top_bar(&self, canvas: &mut Canvas, layout: Layout) {
        let glyphs = self.glyphs;
        let Some(status) = &self.status else {
            let title = vec![(" TTYmud ".to_owned(), Ink::Strong)];
            bar(canvas, layout, 0, &title, &[]);
            return;
        };
        let dot = (format!(" {} ", glyphs.dot), Ink::Dim);
        let mut groups = vec![Group::always(format!(" {}", status.host), Ink::Strong)];
        let mut right = Vec::new();
        if let Some(run) = &status.run {
            groups.push(Group::new(1, vec![(run.network.clone(), Ink::Plain)]));
            groups.push(Group::new(2, vec![(run.access.clone(), Ink::Plain)]));
            if let Some(level) = alert_level(&run.alert) {
                let alert = (format!("{} alert", run.alert), Ink::Level(level));
                groups.push(Group::new(0, vec![alert]));
            }
            let percent = run.trace.min(100);
            let filled = (usize::try_from(percent).unwrap_or(100) + 5) / 10;
            let mut meter = glyphs.full.to_string().repeat(filled);
            meter.push_str(&glyphs.empty.to_string().repeat(BAR - filled));
            right.push((" trace ".to_owned(), Ink::Plain));
            right.push((meter, Ink::Level(percent)));
            right.push((format!(" {percent}% "), Ink::Plain));
        }
        let gap = usize::from(!right.is_empty());
        let room = layout.room().saturating_sub(width(&right) + gap);
        bar(canvas, layout, 0, &fit(groups, &dot, room), &right);
    }

    fn bottom_bar(&self, canvas: &mut Canvas, layout: Layout) {
        let Some(status) = &self.status else {
            return;
        };
        let glyphs = self.glyphs;
        let dot = (format!(" {} ", glyphs.dot), Ink::Dim);
        let heat = (status.heat.saturating_mul(100))
            .checked_div(status.heat_max)
            .unwrap_or(0);
        let integrity = Ink::Level(100_u32.saturating_sub(status.integrity));
        let hot = symbols(glyphs.hot, glyphs.cool, status.heat, status.heat_max);
        let cycles = symbols(glyphs.cycle, glyphs.spent, status.cycles, status.cycles_max);
        let groups = vec![
            Group::always(format!(" integ {}", status.integrity), integrity),
            Group::new(
                2,
                vec![("heat ".to_owned(), Ink::Plain), (hot, Ink::Level(heat))],
            ),
            Group::new(
                0,
                vec![(
                    format!("ram {}/{} GB", status.ram_used, status.ram_total),
                    Ink::Plain,
                )],
            ),
            Group::new(
                3,
                vec![("cycles ".to_owned(), Ink::Plain), (cycles, Ink::Strong)],
            ),
            Group::new(1, vec![(format!("{} cr", status.credits), Ink::Plain)]),
        ];
        let parts = fit(groups, &dot, layout.room());
        bar(canvas, layout, layout.height - 1, &parts, &[]);
    }

    fn notice(&mut self, canvas: &mut Canvas, layout: Layout) {
        let now = self.now();
        let dot = (format!(" {} ", self.glyphs.dot), Ink::Dim);
        let threats = self.threats.live(now);
        let pieces = if !threats.is_empty() {
            countdowns(threats, now, &dot, layout.room())
        } else if let Some(status) = self
            .status
            .as_ref()
            .filter(|status| !status.headline.is_empty())
        {
            vec![
                (" news: ".to_owned(), Ink::Dim),
                (format!("{} ", status.headline), Ink::Plain),
            ]
        } else {
            return;
        };
        bar(canvas, layout, layout.height - 3, &pieces, &[]);
    }

    fn short_prompt(&self) -> &str {
        let state = self.status.is_some().then(|| self.prompt.strip_prefix('['));
        match state.flatten().and_then(|rest| rest.split_once("] ")) {
            Some((_, prompt)) => prompt,
            None => &self.prompt,
        }
    }

    fn input(&mut self, canvas: &mut Canvas, layout: Layout) -> (usize, usize) {
        let y = layout.height - 2;
        let (x, room) = (layout.left(), layout.inner());
        let prompt = self.short_prompt().to_owned();
        let used = canvas.put(x, y, &prompt, Ink::Plain, room);
        let line: Vec<char> = self.editor.line().chars().collect();
        let cursor = self.editor.cursor();
        let space = room.saturating_sub(used + 1).max(1);
        self.start = self
            .start
            .min(cursor)
            .max((cursor + 1).saturating_sub(space));
        let end = line.len().min(self.start + space);
        let before: String = line[self.start..cursor].iter().collect();
        let after: String = line[cursor..end].iter().collect();
        let width = canvas.put(x + used, y, &before, Ink::Plain, room - used);
        canvas.put(x + used + width, y, &after, Ink::Plain, room - used - width);
        (x + used + width, y)
    }

    fn leave(&mut self) -> io::Result<()> {
        if std::mem::take(&mut self.open) {
            if self.mouse {
                self.output.write_all(MOUSE_OFF.as_bytes())?;
            }
            self.output.write_all(LEAVE.as_bytes())?;
        }
        Ok(())
    }

    fn now(&self) -> Instant {
        self.fixed.unwrap_or_else(Instant::now)
    }

    #[cfg(test)]
    fn at(&mut self, now: Instant) {
        self.fixed = Some(now);
    }

    #[cfg(test)]
    fn rows(&self) -> Vec<String> {
        self.shown.as_ref().map_or_default(Canvas::rows)
    }
}

impl<W: Write> Drop for Hud<W> {
    fn drop(&mut self) {
        let _ = self.leave();
        let _ = self.output.flush();
    }
}

impl<W: Write> View for Hud<W> {
    fn show(&mut self, shown: Shown) -> io::Result<()> {
        match shown {
            Shown::Line(message) => {
                let now = self.now();
                self.threats.hear(&message, now);
                self.add(message);
            }
            Shown::Prompt(prompt) => self.prompt = prompt,
            Shown::Status(status) => {
                if status.run.is_none() {
                    self.threats.clear();
                }
                self.status = Some(status);
            }
            Shown::Closed(reason) => {
                self.closed = Some(reason);
                return Ok(());
            }
            Shown::Candidates { word, names } => {
                let Some(asked) = self.asked.take() else {
                    return Ok(());
                };
                match self.editor.complete(&asked, &word, &names) {
                    Completed::Nothing => return Ok(()),
                    Completed::Changed => {}
                    Completed::Show(list) => self.add(Message::game(list)),
                }
            }
        }
        self.draw()
    }

    fn press(&mut self, key: Key) -> io::Result<Typed> {
        match key {
            Key::PageUp => self.scroll += self.page(),
            Key::PageDown => self.scroll = self.scroll.saturating_sub(self.page()),
            Key::ScrollUp => self.scroll += WHEEL,
            Key::ScrollDown => self.scroll = self.scroll.saturating_sub(WHEEL),
            _ => {}
        }
        let scrolled = matches!(
            key,
            Key::PageUp | Key::PageDown | Key::ScrollUp | Key::ScrollDown
        );
        let typed = match self.editor.press(key) {
            Action::Nothing if scrolled => Typed::Nothing,
            Action::Nothing => return Ok(Typed::Nothing),
            Action::Complete(text) => {
                self.asked = Some(text.clone());
                return Ok(Typed::Complete(text));
            }
            Action::Quit => return Ok(Typed::Quit),
            Action::Redraw => Typed::Nothing,
            Action::Submit(line) => {
                let prompt = self.short_prompt().to_owned();
                let echo = Message::game(prompt).span(Style::Command, line.clone());
                self.add(echo);
                (self.start, self.scroll) = (0, 0);
                Typed::Line(line)
            }
        };
        self.draw()?;
        Ok(typed)
    }

    fn resize(&mut self, width: u16, height: u16) -> io::Result<()> {
        self.width = usize::from(width);
        self.height = usize::from(height);
        self.shown = None;
        self.output.write_all(b"\x1b[2J")?;
        self.draw()
    }

    fn redraw_at(&self) -> Option<Instant> {
        self.threats.wake(self.now())
    }

    fn redraw(&mut self) -> io::Result<()> {
        self.draw()
    }

    fn finish(&mut self) -> io::Result<()> {
        self.leave()?;
        if let Some(reason) = self.closed.take() {
            write!(self.output, "{reason}\r\n")?;
        }
        self.output.flush()
    }
}

type Piece = (String, Ink);

struct Group {
    pieces: Vec<Piece>,
    keep: u8,
}

impl Group {
    fn new(keep: u8, pieces: Vec<Piece>) -> Self {
        Self { pieces, keep }
    }

    fn always(text: String, ink: Ink) -> Self {
        Self::new(u8::MAX, vec![(text, ink)])
    }
}

fn joined<'a>(parts: impl IntoIterator<Item = &'a [Piece]>, dot: &Piece) -> Vec<Piece> {
    let mut pieces = Vec::new();
    for (index, part) in parts.into_iter().enumerate() {
        if index > 0 {
            pieces.push(dot.clone());
        }
        pieces.extend_from_slice(part);
    }
    pieces
}

fn countdowns(threats: &[Threat], now: Instant, dot: &Piece, room: usize) -> Vec<Piece> {
    let parts: Vec<Vec<Piece>> = threats
        .iter()
        .map(|threat| {
            let kind = threat.label.kind;
            let spans = threat.label.spans.iter();
            let mut part: Vec<Piece> = spans
                .map(|span| (span.text.clone(), Ink::Text(span.style, kind)))
                .collect();
            let left = threat
                .left(now)
                .map_or_else(|| "now".to_owned(), |seconds| format!("{seconds}s"));
            part.push((format!(" {left}"), Ink::Strong));
            part
        })
        .collect();
    let mut shown = parts.len();
    loop {
        let mut pieces = vec![(" threats: ".to_owned(), Ink::Dim)];
        pieces.extend(joined(parts[..shown].iter().map(Vec::as_slice), dot));
        if shown < parts.len() {
            pieces.push((format!(" +{}", parts.len() - shown), Ink::Strong));
        }
        pieces.push((" ".to_owned(), Ink::Plain));
        if shown == 0 || width(&pieces) <= room {
            return pieces;
        }
        shown -= 1;
    }
}

fn fit(mut groups: Vec<Group>, dot: &Piece, room: usize) -> Vec<Piece> {
    loop {
        let mut pieces = joined(groups.iter().map(|group| group.pieces.as_slice()), dot);
        pieces.push((" ".to_owned(), Ink::Plain));
        let lowest = groups
            .iter()
            .enumerate()
            .filter(|(_, group)| group.keep < u8::MAX)
            .min_by_key(|(_, group)| group.keep)
            .map(|(index, _)| index);
        match lowest {
            Some(index) if width(&pieces) > room => {
                groups.remove(index);
            }
            _ => return pieces,
        }
    }
}

fn width(pieces: &[Piece]) -> usize {
    pieces
        .iter()
        .map(|(text, _)| text.chars().map(columns).sum::<usize>())
        .sum()
}

fn bar(canvas: &mut Canvas, layout: Layout, y: usize, left: &[Piece], right: &[Piece]) {
    let start = usize::from(layout.borders);
    let end = layout.width - start;
    let size = width(right);
    let mut limit = end;
    if size > 0 && start + size < end {
        let mut x = end - size;
        limit = x;
        for (text, ink) in right {
            x += canvas.put(x, y, text, *ink, end - x);
        }
    }
    let mut x = start;
    for (text, ink) in left {
        x += canvas.put(x, y, text, *ink, limit.saturating_sub(x));
    }
}

fn pane<'a>(
    canvas: &mut Canvas,
    layout: Layout,
    messages: impl DoubleEndedIterator<Item = &'a Message>,
    top: usize,
    rows: usize,
    scroll: usize,
) -> usize {
    let width = layout.inner();
    let mut kept: Vec<Row> = Vec::new();
    for message in messages.rev() {
        kept.extend(wrap(message, width).into_iter().rev());
        if kept.len() >= rows + scroll {
            break;
        }
    }
    let scroll = scroll.min(kept.len().saturating_sub(rows));
    let shown = kept.iter().skip(scroll).take(rows);
    for (index, row) in shown.rev().enumerate() {
        let mut used = 0;
        for group in row.chunk_by(|a, b| a.1 == b.1) {
            let text: String = group.iter().map(|&(c, _)| c).collect();
            let room = width.saturating_sub(used);
            used += canvas.put(layout.left() + used, top + index, &text, group[0].1, room);
        }
    }
    scroll
}

fn alert_level(alert: &str) -> Option<u32> {
    match alert {
        "green" => Some(0),
        "yellow" => Some(50),
        "orange" => Some(75),
        "red" => Some(100),
        _ => None,
    }
}

fn symbols(on: char, off: char, count: u32, total: u32) -> String {
    let total = total.min(SYMBOLS);
    let count = count.min(total);
    let mut text = on.to_string().repeat(usize::try_from(count).unwrap_or(0));
    text.push_str(
        &off.to_string()
            .repeat(usize::try_from(total - count).unwrap_or(0)),
    );
    text
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use ttymud_text::{Kind, Mate, Message, RunStatus, Status, Style};

    use super::Hud;
    use crate::Shown;
    use crate::editor::Key;
    use crate::session::{Typed, View};
    use crate::settings::{Depth, Layer, Settings};

    fn settings(ascii: bool, color: Depth) -> Settings {
        Settings {
            layer: Layer::Hud,
            color,
            ascii,
            verbose: false,
            mouse: false,
        }
    }

    fn hud(width: u16, height: u16) -> Hud<Vec<u8>> {
        Hud::new(Vec::new(), settings(false, Depth::None), width, height).unwrap()
    }

    fn framed(text: &str) -> String {
        format!("│ {text:<66} │")
    }

    fn written(hud: &mut Hud<Vec<u8>>) -> String {
        String::from_utf8_lossy(&std::mem::take(&mut hud.output)).into_owned()
    }

    fn run_status() -> Status {
        Status {
            host: "arcadia-db".to_owned(),
            run: Some(RunStatus {
                network: "Arcadia Health".to_owned(),
                access: "user".to_owned(),
                trace: 31,
                alert: String::new(),
            }),
            integrity: 82,
            heat: 2,
            heat_max: 5,
            ram_used: 6,
            ram_total: 8,
            cycles: 3,
            cycles_max: 4,
            credits: 1250,
            headline: String::new(),
            crew: Vec::new(),
        }
    }

    #[test]
    fn the_chat_rule_shows_the_crewmates_in_the_run_and_their_traces() {
        let mut hud = hud(70, 20);
        let mut status = run_status();
        status.crew = vec![
            Mate {
                name: "nyx".to_owned(),
                trace: 34,
            },
            Mate {
                name: "zer0".to_owned(),
                trace: 0,
            },
        ];
        hud.show(Shown::Status(status)).unwrap();
        let rows = hud.rows();
        let right = " nyx 34% · zer0 0% ┤";
        assert!(rows[12].starts_with("├ chat ─"), "{}", rows[12]);
        assert!(rows[12].ends_with(right), "{}", rows[12]);
    }

    #[test]
    fn the_top_bar_shows_the_alert_and_a_rule_shows_the_latest_headline() {
        let mut hud = Hud::new(Vec::new(), settings(false, Depth::Basic), 80, 20).unwrap();
        let mut status = run_status();
        status.headline = "Acme Corp raises its alert to red.".to_owned();
        if let Some(run) = status.run.as_mut() {
            run.alert = "red".to_owned();
        }
        hud.show(Shown::Status(status)).unwrap();
        let text = written(&mut hud);
        assert!(text.contains("\x1b[91mred alert"), "{text:?}");
        let rows = hud.rows();
        let dashes = |count| "─".repeat(count);
        assert_eq!(
            rows[0],
            format!(
                "┌ arcadia-db · Arcadia Health · user · red alert {} trace ▓▓▓░░░░░░░ 31% ┐",
                dashes(8)
            )
        );
        assert_eq!(
            rows[17],
            format!("├ news: Acme Corp raises its alert to red. {}┤", dashes(36))
        );
    }

    #[test]
    fn the_hud_has_two_bars_two_panes_and_an_input_line() {
        let mut hud = hud(70, 20);
        hud.show(Shown::Status(run_status())).unwrap();
        hud.show(Shown::Line(Message::game("The ledger server hums.")))
            .unwrap();
        let chat = Message::new(Kind::Chat).text("[ostra] <nyx> hi");
        hud.show(Shown::Line(chat)).unwrap();
        hud.show(Shown::Prompt("ghost@arcadia-db $ ".to_owned()))
            .unwrap();
        hud.press(Key::Char('l')).unwrap();
        let rows = hud.rows();
        assert_eq!(rows.len(), 20);
        let dashes = |count| "─".repeat(count);
        assert_eq!(
            rows[0],
            format!(
                "┌ arcadia-db · Arcadia Health · user {} trace ▓▓▓░░░░░░░ 31% ┐",
                dashes(10)
            )
        );
        assert_eq!(rows[1], framed("The ledger server hums."));
        assert_eq!(rows[2], framed(""));
        assert_eq!(rows[12], format!("├ chat {}┤", dashes(62)));
        assert_eq!(rows[13], framed("[ostra] <nyx> hi"));
        assert_eq!(rows[17], format!("├{}┤", dashes(68)));
        assert_eq!(rows[18], framed("ghost@arcadia-db $ l"));
        assert_eq!(
            rows[19],
            format!(
                "└ integ 82 · heat ▲▲△△△ · ram 6/8 GB · cycles ●●●○ · 1250 cr {}┘",
                dashes(8)
            )
        );
        assert!(written(&mut hud).ends_with("\x1b[19;23H\x1b[?25h"));
    }

    #[test]
    fn a_small_window_drops_the_chat_pane_and_then_the_borders() {
        let mut hud = hud(50, 12);
        hud.show(Shown::Line(Message::game("The Relay"))).unwrap();
        let chat = Message::new(Kind::Chat).text("<nyx> hi");
        hud.show(Shown::Line(chat)).unwrap();
        let rows = hud.rows();
        assert_eq!(rows.len(), 12);
        assert_eq!(rows[0], format!(" TTYmud {}", "─".repeat(42)));
        assert_eq!(rows[1], format!("{:<50}", "The Relay"));
        assert_eq!(rows[2], format!("{:<50}", "<nyx> hi"));
        assert_eq!(rows[9], "─".repeat(50));
        assert_eq!(rows[11], "─".repeat(50));
    }

    #[test]
    fn a_tiny_window_asks_for_more_room_until_it_grows() {
        let mut hud = hud(39, 20);
        assert_eq!(hud.rows()[0], format!("{:<39}", "Make the window larger."));
        assert!(!written(&mut hud).ends_with("\x1b[?25h"));
        hud.resize(80, 24).unwrap();
        let rows = hud.rows();
        assert_eq!((rows.len(), rows[0].chars().count()), (24, 80));
        assert!(rows[0].starts_with('┌'));
        assert!(written(&mut hud).starts_with("\x1b[2J"));
    }

    #[test]
    fn ascii_draws_the_frame_and_the_bars_with_ascii_characters() {
        let mut hud = Hud::new(Vec::new(), settings(true, Depth::None), 70, 20).unwrap();
        hud.show(Shown::Status(run_status())).unwrap();
        let rows = hud.rows();
        assert_eq!(
            rows[0],
            format!(
                "+ arcadia-db | Arcadia Health | user {} trace ###....... 31% +",
                "-".repeat(10)
            )
        );
        assert_eq!(rows[1], format!("| {:<66} |", ""));
        assert_eq!(
            rows[19],
            format!(
                "+ integ 82 | heat ^^... | ram 6/8 GB | cycles ***. | 1250 cr {}+",
                "-".repeat(8)
            )
        );
    }

    #[test]
    fn styles_and_levels_have_colors() {
        let mut hud = Hud::new(Vec::new(), settings(false, Depth::Basic), 70, 20).unwrap();
        hud.show(Shown::Status(run_status())).unwrap();
        written(&mut hud);
        let scene = Message::game("Routes: ").span(Style::Host, "relay");
        hud.show(Shown::Line(scene)).unwrap();
        let text = written(&mut hud);
        assert!(text.contains("\x1b[36mrelay"), "{text:?}");
        assert!(!text.contains("\x1b[1;1H"), "{text:?}");
    }

    #[test]
    fn enter_shows_the_line_in_the_output_and_tab_completes_it() {
        let mut hud = hud(70, 20);
        hud.show(Shown::Prompt("$ ".to_owned())).unwrap();
        for c in "go ma".chars() {
            hud.press(Key::Char(c)).unwrap();
        }
        assert_eq!(
            hud.press(Key::Tab).unwrap(),
            Typed::Complete("go ma".to_owned())
        );
        let candidates = Shown::Candidates {
            word: "ma".to_owned(),
            names: vec!["market".to_owned()],
        };
        hud.show(candidates).unwrap();
        assert_eq!(
            hud.press(Key::Enter).unwrap(),
            Typed::Line("go market ".to_owned())
        );
        let rows = hud.rows();
        assert_eq!(rows[1], framed("$ go market"));
        assert_eq!(rows[18], framed("$"));
    }

    #[test]
    fn page_up_and_page_down_scroll_the_output_and_new_text_waits_below() {
        let mut hud = hud(70, 20);
        for number in 1..=30 {
            let line = Message::game(format!("line {number}"));
            hud.show(Shown::Line(line)).unwrap();
        }
        assert_eq!(hud.rows()[11], framed("line 30"));
        hud.press(Key::PageUp).unwrap();
        assert_eq!(hud.rows()[11], framed("line 20"));
        assert!(hud.rows()[12].ends_with(" ▼ more below (PgDn) ┤"));
        hud.show(Shown::Line(Message::game("line 31"))).unwrap();
        assert_eq!(hud.rows()[11], framed("line 20"));
        let chat = Message::new(Kind::Chat).text("<nyx> hi");
        hud.show(Shown::Line(chat)).unwrap();
        assert_eq!(hud.rows()[11], framed("line 20"));
        hud.press(Key::PageDown).unwrap();
        hud.press(Key::PageDown).unwrap();
        assert_eq!(hud.rows()[11], framed("line 31"));
        assert!(!hud.rows()[12].contains("more below"));
        for _ in 0..10 {
            hud.press(Key::PageUp).unwrap();
        }
        assert_eq!(hud.rows()[1], framed("line 1"));
        hud.press(Key::Char('x')).unwrap();
        hud.press(Key::Enter).unwrap();
        assert_eq!(hud.rows()[11], framed("x"));
    }

    fn mosaic() -> Status {
        let mut status = run_status();
        status.host = "ws".to_owned();
        if let Some(run) = status.run.as_mut() {
            run.network = "Mosaic Media".to_owned();
            run.alert = "yellow".to_owned();
        }
        status
    }

    #[test]
    fn a_narrow_top_bar_drops_whole_parts_and_keeps_the_trace() {
        let mut wide = hud(60, 20);
        wide.show(Shown::Status(mosaic())).unwrap();
        assert_eq!(
            wide.rows()[0],
            format!(
                "┌ ws · Mosaic Media · user {} trace ▓▓▓░░░░░░░ 31% ┐",
                "─".repeat(10)
            )
        );
        let mut narrow = hud(42, 20);
        narrow.show(Shown::Status(mosaic())).unwrap();
        assert_eq!(
            narrow.rows()[0],
            format!(" ws · user {} trace ▓▓▓░░░░░░░ 31% ", "─".repeat(9))
        );
    }

    #[test]
    fn a_narrow_bottom_bar_drops_the_ram_and_then_the_credits() {
        let mut wide = hud(60, 20);
        wide.show(Shown::Status(mosaic())).unwrap();
        assert_eq!(
            wide.rows()[19],
            format!(
                "└ integ 82 · heat ▲▲△△△ · cycles ●●●○ · 1250 cr {}┘",
                "─".repeat(11)
            )
        );
        let mut narrow = hud(45, 20);
        narrow.show(Shown::Status(mosaic())).unwrap();
        assert_eq!(
            narrow.rows()[19],
            format!(" integ 82 · heat ▲▲△△△ · cycles ●●●○ {}", "─".repeat(8))
        );
    }

    #[test]
    fn the_prompt_drops_the_state_that_the_bars_show() {
        let mut hud = hud(70, 20);
        let full = "[trace 31% integ 82] ghost@arcadia-db $ ";
        hud.show(Shown::Prompt(full.to_owned())).unwrap();
        assert_eq!(hud.rows()[18], framed(full.trim_end()));
        hud.show(Shown::Status(run_status())).unwrap();
        assert_eq!(hud.rows()[18], framed("ghost@arcadia-db $"));
        hud.press(Key::Char('l')).unwrap();
        hud.press(Key::Enter).unwrap();
        assert_eq!(hud.rows()[1], framed("ghost@arcadia-db $ l"));
    }

    #[test]
    fn the_mouse_wheel_scrolls_the_output_three_rows_at_a_time() {
        let mut hud = hud(70, 20);
        for number in 1..=30 {
            hud.show(Shown::Line(Message::game(format!("line {number}"))))
                .unwrap();
        }
        hud.press(Key::ScrollUp).unwrap();
        assert_eq!(hud.rows()[11], framed("line 27"));
        hud.press(Key::ScrollDown).unwrap();
        assert_eq!(hud.rows()[11], framed("line 30"));
    }

    #[test]
    fn a_captured_mouse_is_set_free_when_the_hud_closes() {
        let mut hud = hud(70, 20);
        written(&mut hud);
        hud.capture_mouse().unwrap();
        assert_eq!(written(&mut hud), "\x1b[?1000h\x1b[?1006h");
        hud.finish().unwrap();
        assert!(
            written(&mut hud).starts_with("\x1b[?1006l\x1b[?1000l\x1b[0m"),
            "the mouse goes free before the screen"
        );
    }

    fn compile(name: &str, action: &str, ticks: u32) -> Message {
        Message::new(Kind::Telegraph)
            .span(Style::Ice, name)
            .text(" starts to compile ")
            .span(Style::Danger, action)
            .text(format!(" ({ticks} ticks)."))
    }

    #[test]
    fn the_threat_line_counts_down_above_the_input_and_then_gives_way_to_the_news() {
        let start = Instant::now();
        let mut hud = hud(70, 20);
        hud.at(start);
        let mut status = run_status();
        status.headline = "Acme Corp raises its alert to red.".to_owned();
        hud.show(Shown::Status(status)).unwrap();
        hud.show(Shown::Line(compile("sentry", "ALERT", 3)))
            .unwrap();
        let dashes = |count| "─".repeat(count);
        let threat = |left| format!("├ threats: sentry ALERT {left} {}┤", dashes(42));
        assert_eq!(hud.rows()[17], threat("6s"));
        assert_eq!(hud.redraw_at(), Some(start + Duration::from_secs(1)));
        hud.at(start + Duration::from_millis(2_500));
        hud.redraw().unwrap();
        assert_eq!(hud.rows()[17], threat("4s"));
        let sends = Message::new(Kind::Telegraph)
            .span(Style::Ice, "sentry")
            .text(" sends an ")
            .span(Style::Danger, "ALERT")
            .text(".");
        hud.show(Shown::Line(sends)).unwrap();
        assert_eq!(
            hud.rows()[17],
            format!("├ news: Acme Corp raises its alert to red. {}┤", dashes(26))
        );
        assert_eq!(hud.redraw_at(), None);
    }

    #[test]
    fn a_narrow_threat_line_keeps_the_soonest_threats_and_counts_the_rest() {
        let mut hud = hud(55, 20);
        hud.at(Instant::now());
        hud.show(Shown::Status(run_status())).unwrap();
        let threats = [
            ("sentry", "ALERT", 5),
            ("scorch", "BURN", 2),
            ("hound", "TRACE BURST", 4),
        ];
        for (name, action, ticks) in threats {
            hud.show(Shown::Line(compile(name, action, ticks))).unwrap();
        }
        assert_eq!(
            hud.rows()[17],
            format!(
                " threats: scorch BURN 4s · hound TRACE BURST 8s +1 {}",
                "─".repeat(4)
            )
        );
        let mut home = run_status();
        home.run = None;
        hud.show(Shown::Status(home)).unwrap();
        assert_eq!(
            hud.rows()[17],
            "─".repeat(55),
            "a run that ends takes its threats"
        );
    }

    #[test]
    fn a_close_leaves_the_screen_and_then_shows_the_reason() {
        let mut hud = hud(70, 20);
        assert!(written(&mut hud).starts_with("\x1b[?1049h"));
        hud.show(Shown::Closed("Bye.".to_owned())).unwrap();
        hud.finish().unwrap();
        assert!(written(&mut hud).ends_with("\x1b[?1049l\x1b[?25hBye.\r\n"));
    }
}
