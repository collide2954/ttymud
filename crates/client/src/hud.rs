use std::collections::VecDeque;
use std::io::{self, Write};

use ttymud_text::{Kind, Message, Status, Style};

use crate::Shown;
use crate::canvas::{Canvas, Ink};
use crate::editor::{Action, Completed, Editor, Key};
use crate::session::{Typed, View};
use crate::settings::{Depth, Settings};
use crate::wrap::{Row, columns, wrap};

const KEEP: usize = 1_000;
const MIN_WIDTH: usize = 40;
const MIN_HEIGHT: usize = 10;
const BORDERS: usize = 60;
const CHAT: usize = 20;
const BAR: usize = 10;
const SYMBOLS: u32 = 12;
const ENTER: &str = "\x1b[?1049h\x1b[?7l\x1b[2J";
const LEAVE: &str = "\x1b[0m\x1b[?7h\x1b[?1049l\x1b[?25h";
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
        };
        hud.output.write_all(ENTER.as_bytes())?;
        hud.draw()?;
        Ok(hud)
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
        let mut more = Vec::new();
        if self.scroll > 0 {
            let down = self.glyphs.down;
            more.push((format!(" {down} more below (PgDn) "), Ink::Strong));
        }
        bar(canvas, layout, layout.output + 1, &label, &more);
        self.input(canvas, layout)
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
        let mut left = vec![(format!(" {}", status.host), Ink::Strong)];
        let mut right = Vec::new();
        if let Some(run) = &status.run {
            left.push(dot.clone());
            left.push((run.network.clone(), Ink::Plain));
            left.push(dot.clone());
            left.push((run.access.clone(), Ink::Plain));
            if let Some(level) = alert_level(&run.alert) {
                left.push(dot);
                left.push((format!("{} alert", run.alert), Ink::Level(level)));
            }
            let percent = run.trace.min(100);
            let filled = (usize::try_from(percent).unwrap_or(100) + 5) / 10;
            let mut meter = glyphs.full.to_string().repeat(filled);
            meter.push_str(&glyphs.empty.to_string().repeat(BAR - filled));
            right.push((" trace ".to_owned(), Ink::Plain));
            right.push((meter, Ink::Level(percent)));
            right.push((format!(" {percent}% "), Ink::Plain));
        }
        left.push((" ".to_owned(), Ink::Plain));
        bar(canvas, layout, 0, &left, &right);
    }

    fn bottom_bar(&self, canvas: &mut Canvas, layout: Layout) {
        let Some(status) = &self.status else {
            return;
        };
        let glyphs = self.glyphs;
        let dot = || (format!(" {} ", glyphs.dot), Ink::Dim);
        let heat = (status.heat.saturating_mul(100))
            .checked_div(status.heat_max)
            .unwrap_or(0);
        let parts = vec![
            (
                format!(" integ {}", status.integrity),
                Ink::Level(100_u32.saturating_sub(status.integrity)),
            ),
            dot(),
            ("heat ".to_owned(), Ink::Plain),
            (
                symbols(glyphs.hot, glyphs.cool, status.heat, status.heat_max),
                Ink::Level(heat),
            ),
            dot(),
            (
                format!("ram {}/{} GB", status.ram_used, status.ram_total),
                Ink::Plain,
            ),
            dot(),
            ("cycles ".to_owned(), Ink::Plain),
            (
                symbols(glyphs.cycle, glyphs.spent, status.cycles, status.cycles_max),
                Ink::Strong,
            ),
            dot(),
            (format!("{} cr ", status.credits), Ink::Plain),
        ];
        bar(canvas, layout, layout.height - 1, &parts, &[]);
        if !status.headline.is_empty() {
            let news = [
                (" news: ".to_owned(), Ink::Dim),
                (format!("{} ", status.headline), Ink::Plain),
            ];
            bar(canvas, layout, layout.height - 3, &news, &[]);
        }
    }

    fn input(&mut self, canvas: &mut Canvas, layout: Layout) -> (usize, usize) {
        let y = layout.height - 2;
        let (x, room) = (layout.left(), layout.inner());
        let used = canvas.put(x, y, &self.prompt, Ink::Plain, room);
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
            self.output.write_all(LEAVE.as_bytes())?;
        }
        Ok(())
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
            Shown::Line(message) => self.add(message),
            Shown::Prompt(prompt) => self.prompt = prompt,
            Shown::Status(status) => self.status = Some(status),
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
            _ => {}
        }
        let typed = match self.editor.press(key) {
            Action::Nothing if matches!(key, Key::PageUp | Key::PageDown) => Typed::Nothing,
            Action::Nothing => return Ok(Typed::Nothing),
            Action::Complete(text) => {
                self.asked = Some(text.clone());
                return Ok(Typed::Complete(text));
            }
            Action::Quit => return Ok(Typed::Quit),
            Action::Redraw => Typed::Nothing,
            Action::Submit(line) => {
                let echo = Message::game(self.prompt.clone()).span(Style::Command, line.clone());
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

    fn finish(&mut self) -> io::Result<()> {
        self.leave()?;
        if let Some(reason) = self.closed.take() {
            write!(self.output, "{reason}\r\n")?;
        }
        self.output.flush()
    }
}

fn bar(
    canvas: &mut Canvas,
    layout: Layout,
    y: usize,
    left: &[(String, Ink)],
    right: &[(String, Ink)],
) {
    let start = usize::from(layout.borders);
    let end = layout.width - start;
    let size: usize = right
        .iter()
        .map(|(text, _)| text.chars().map(columns).sum::<usize>())
        .sum();
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
    use ttymud_text::{Kind, Message, RunStatus, Status, Style};

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
        }
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

    #[test]
    fn a_close_leaves_the_screen_and_then_shows_the_reason() {
        let mut hud = hud(70, 20);
        assert!(written(&mut hud).starts_with("\x1b[?1049h"));
        hud.show(Shown::Closed("Bye.".to_owned())).unwrap();
        hud.finish().unwrap();
        assert!(written(&mut hud).ends_with("\x1b[?1049l\x1b[?25hBye.\r\n"));
    }
}
