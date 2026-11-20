use ttymud_text::{Message, Style};

use super::builds::Build;
use super::rig::Rig;
use super::save::{Text, allow, checked, number, problem};
use super::{HostId, Output, Player, PlayerId, Step, World, lines};
use crate::commands::{self, TOP_VERSION, Verb};
use crate::content::{ContentError, File};
use crate::ini::{self, Pair, Section};
use crate::kits::{CYCLES, RAM};

pub(super) const AUCTION: &str = "auction.ini";
const HOUR: u64 = 3_600;
const DAY: u64 = 24 * HOUR;
const LENGTH: u64 = 3 * DAY;
const OPENING: u32 = 100;
const USAGE: &str = "Usage: bid <lot> <credits>. Type lots to see the lots.";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Seized {
    File(File),
    Ram(u32),
    Cpu(u32),
    Build(Verb, Build),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Bid {
    pub(super) key: String,
    pub(super) credits: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Lot {
    pub(super) number: u32,
    pub(super) item: Seized,
    pub(super) ends: u64,
    pub(super) bid: Option<Bid>,
}

#[derive(Debug, Default)]
pub(super) struct Auction {
    pub(super) host: Option<HostId>,
    pub(super) lots: Vec<Lot>,
    pub(super) next: u32,
    pub(super) dirty: bool,
}

impl Auction {
    pub(super) fn save(&mut self) -> Option<(String, String)> {
        std::mem::take(&mut self.dirty).then(|| (AUCTION.to_owned(), self.text()))
    }

    fn text(&self) -> String {
        let mut text = Text::new("# The police auction. The server writes this file.");
        text.section("auction", "police");
        text.pair("next", &self.next.to_string());
        for lot in &self.lots {
            text.section("lot", &lot.number.to_string());
            text.pair("ends", &lot.ends.to_string());
            match &lot.item {
                Seized::File(file) => {
                    text.pair("file", &file.name);
                    if file.value > 0 {
                        text.pair("value", &file.value.to_string());
                    }
                    for line in &file.lines {
                        text.pair("line", line);
                    }
                }
                Seized::Ram(size) => text.pair("ram", &size.to_string()),
                Seized::Cpu(cycles) => text.pair("cpu", &cycles.to_string()),
                Seized::Build(verb, build) => {
                    let name = commands::name_of(*verb);
                    text.pair("build", &format!("{name} {}", build.version));
                    if build.quiet {
                        text.pair("quiet", "yes");
                    }
                }
            }
            if let Some(bid) = &lot.bid {
                text.pair("bid", &format!("{} {}", bid.key, bid.credits));
            }
        }
        text.finish()
    }

    pub(super) fn read(&mut self, text: &str) -> Result<(), Vec<ContentError>> {
        let (sections, mut errors) = ini::read(text);
        let (mut next, mut lots) = (0, Vec::new());
        for section in &sections {
            match section.kind {
                "auction" => {
                    allow(section, &["next"], &mut errors);
                    let pair = section.pairs.iter().find(|pair| pair.key == "next");
                    next = pair.and_then(|pair| number(pair, &mut errors)).unwrap_or(0);
                }
                "lot" => lots.extend(read_lot(section, &mut errors)),
                kind => errors.push(problem(
                    section.line,
                    format!("unknown section type: {kind}"),
                )),
            }
        }
        let (next, lots): (u32, Vec<Lot>) = checked(errors, (next, lots))?;
        self.next = lots.iter().map(|lot| lot.number).fold(next, u32::max);
        self.lots = lots;
        Ok(())
    }
}

fn read_lot(section: &Section<'_>, errors: &mut Vec<ContentError>) -> Option<Lot> {
    let keys = [
        "ends", "file", "value", "line", "ram", "cpu", "build", "quiet", "bid",
    ];
    allow(section, &keys, errors);
    let Ok(lot) = section.id.parse::<u32>() else {
        let id = section.id;
        errors.push(problem(section.line, format!("bad lot number: {id}")));
        return None;
    };
    let value = |key: &str| section.pairs.iter().find(|pair| pair.key == key);
    let Some(ends) = value("ends") else {
        errors.push(problem(section.line, format!("lot {lot} has no ends")));
        return None;
    };
    let ends = number(ends, errors)?;
    let item = if let Some(pair) = value("file") {
        let lines = section.pairs.iter().filter(|pair| pair.key == "line");
        let mut file = File::new(
            pair.value,
            lines.map(|pair| pair.value.to_owned()).collect(),
        );
        file.value = value("value")
            .and_then(|pair| number(pair, errors))
            .unwrap_or(0);
        Some(Seized::File(file))
    } else if let Some(pair) = value("ram") {
        number(pair, errors).map(Seized::Ram)
    } else if let Some(pair) = value("cpu") {
        number(pair, errors).map(Seized::Cpu)
    } else if let Some(pair) = value("build") {
        build(pair, value("quiet").is_some(), errors)
    } else {
        errors.push(problem(section.line, format!("lot {lot} has no item")));
        None
    }?;
    let bid = value("bid").and_then(|pair| {
        let parsed = pair.value.split_once(' ').and_then(|(key, credits)| {
            let credits = credits.trim().parse().ok()?;
            Some(Bid {
                key: key.to_owned(),
                credits,
            })
        });
        if parsed.is_none() {
            let value = pair.value;
            let message = format!("bad bid: {value} (use an account and credits)");
            errors.push(problem(pair.line, message));
        }
        parsed
    });
    Some(Lot {
        number: lot,
        item,
        ends,
        bid,
    })
}

fn build(pair: &Pair<'_>, quiet: bool, errors: &mut Vec<ContentError>) -> Option<Seized> {
    let parsed = pair.value.split_once(' ').and_then(|(name, version)| {
        let verb = commands::find(name)
            .map(|command| command.verb)
            .filter(|verb| verb.program().is_some())?;
        let version = version.trim().parse().ok()?;
        let fits = (1..=TOP_VERSION).contains(&version) && (version == 1 || verb.power().is_some());
        fits.then_some(Seized::Build(verb, Build { version, quiet }))
    });
    if parsed.is_none() {
        let value = pair.value;
        let message = format!("bad build: {value} (use a program and a version)");
        errors.push(problem(pair.line, message));
    }
    parsed
}

impl Seized {
    fn shown(&self, message: Message) -> Message {
        match self {
            Self::File(file) => {
                let rest = match file.teaches() {
                    Some((verb, version)) => {
                        let name = commands::name_of(verb);
                        format!(", the blueprint for {name} v{version}")
                    }
                    None => format!(", data worth {} cr", file.value),
                };
                message.span(Style::File, &file.name).text(rest)
            }
            Self::Ram(size) => message.text(format!("{size} GB of RAM")),
            Self::Cpu(cycles) => message.text(format!("a CPU with {cycles} cycles")),
            Self::Build(verb, build) => {
                let version = match build.version {
                    1 => String::new(),
                    version => format!(" v{version}"),
                };
                let quiet = if build.quiet {
                    " with the quiet mod"
                } else {
                    ""
                };
                message
                    .span(Style::Command, commands::name_of(*verb))
                    .text(format!("{version}{quiet}"))
            }
        }
    }
}

fn seized(rig: Rig) -> Vec<Seized> {
    let files = rig
        .files
        .into_iter()
        .filter(|file| file.value > 0 || file.teaches().is_some())
        .map(Seized::File);
    let ram = (rig.ram > RAM).then_some(Seized::Ram(rig.ram));
    let cpu = (rig.cpu > CYCLES).then_some(Seized::Cpu(rig.cpu));
    let builds = rig
        .builds
        .into_iter()
        .map(|(verb, build)| Seized::Build(verb, build));
    files.chain(ram).chain(cpu).chain(builds).collect()
}

fn least(bid: Option<&Bid>) -> u32 {
    bid.map_or(OPENING, |bid| bid.credits + bid.credits.div_ceil(10))
}

fn left(seconds: u64) -> String {
    let (count, unit) = if seconds > DAY {
        (seconds.div_ceil(DAY), "day")
    } else {
        (seconds.div_ceil(HOUR).max(1), "hour")
    };
    if count == 1 {
        format!("1 {unit}")
    } else {
        format!("{count} {unit}s")
    }
}

impl World {
    pub(super) fn auction_rig(&mut self, name: &str, rig: Rig) {
        if self.auction.host.is_none() {
            return;
        }
        let items = seized(rig);
        let count = items.len();
        if count == 0 {
            return;
        }
        let ends = self.now + LENGTH;
        for item in items {
            self.auction.next += 1;
            let number = self.auction.next;
            self.auction.lots.push(Lot {
                number,
                item,
                ends,
                bid: None,
            });
        }
        self.auction.dirty = true;
        let lots = if count == 1 {
            "1 lot".to_owned()
        } else {
            format!("{count} lots")
        };
        self.publish(format!("The police auction {lots} from the rig of {name}."));
    }

    fn auction_here(&self, player: PlayerId) -> Result<(), Message> {
        let Some(host) = self.auction.host else {
            return Err(Message::error("This city has no police auction."));
        };
        if self.here(player) == host {
            return Ok(());
        }
        Err(
            Message::error("There is no auction here. The police auction is at ")
                .span(Style::Host, &self.hosts[host.0].id)
                .text("."),
        )
    }

    pub(super) fn lots(&self, player: PlayerId) -> Result<Step, Message> {
        self.auction_here(player)?;
        if self.auction.lots.is_empty() {
            let none = Message::game("The police have no lots for auction now.");
            return Ok(lines(vec![none]));
        }
        let key = self.players.get(&player).map(Player::key);
        let mut shown = vec![Message::game("The police auction these lots:")];
        for lot in &self.auction.lots {
            let top = match &lot.bid {
                None => "none".to_owned(),
                Some(bid) if Some(&bid.key) == key.as_ref() => format!("{} cr, yours", bid.credits),
                Some(bid) => format!("{} cr", bid.credits),
            };
            let left = left(lot.ends.saturating_sub(self.now));
            let message = lot
                .item
                .shown(Message::game(format!("Lot {}: ", lot.number)));
            shown.push(message.text(format!(". Top bid: {top}. Ends in {left}.")));
        }
        shown.push(
            Message::game("Type ")
                .span(Style::Command, "bid <lot> <credits>")
                .text(format!(
                    ". The first bid must be {OPENING} cr or more, and a new bid must beat the top bid by 10%."
                )),
        );
        Ok(lines(shown))
    }

    pub(super) fn bid(&mut self, player: PlayerId, args: &[String]) -> Result<Step, Message> {
        self.auction_here(player)?;
        let [lot, credits] = args else {
            return Err(Message::error(USAGE));
        };
        let (Ok(number), Ok(credits)) = (lot.parse::<u32>(), credits.parse::<u32>()) else {
            return Err(Message::error(USAGE));
        };
        let Some(state) = self.players.get(&player) else {
            return Ok(Step::default());
        };
        let (key, have) = (state.key(), state.credits);
        let Some(index) = self
            .auction
            .lots
            .iter()
            .position(|lot| lot.number == number)
        else {
            return Err(Message::error(format!(
                "There is no lot {number}. Type lots to see the lots."
            )));
        };
        let top = self.auction.lots[index].bid.as_ref();
        if top.is_some_and(|bid| bid.key == key) {
            return Err(Message::error(format!(
                "You already have the top bid on lot {number}."
            )));
        }
        let least = least(top);
        if credits < least {
            return Err(Message::error(format!(
                "Your bid must be {least} cr or more."
            )));
        }
        if credits > have {
            return Err(Message::error(format!("You have only {have} cr.")));
        }
        if let Some(state) = self.players.get_mut(&player) {
            state.credits -= credits;
        }
        self.touch(player);
        self.auction.dirty = true;
        let old = self.auction.lots[index].bid.replace(Bid { key, credits });
        if let Some(old) = old {
            self.refund(&old, number);
        }
        Ok(Step::show(vec![Message::game(format!(
            "You bid {credits} cr on lot {number}. The police hold your credits until someone bids more or the auction ends."
        ))]))
    }

    fn refund(&mut self, bid: &Bid, number: u32) {
        let credits = bid.credits;
        let online = self
            .players
            .iter()
            .find(|(_, state)| state.key() == bid.key);
        if let Some(player) = online.map(|(&player, _)| player) {
            if let Some(state) = self.players.get_mut(&player) {
                state.credits += credits;
            }
            self.touch(player);
            self.outbox.push(Output {
                player,
                message: Message::game(format!(
                    "You lose the top bid on lot {number}. The police give back your {credits} cr."
                )),
            });
            return;
        }
        let account = self.accounts.get_mut(&bid.key);
        if let Some(handle) = account.and_then(|account| account.handle.as_mut()) {
            handle.credits += credits;
            self.dirty.insert(bid.key.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::commands::Verb;
    use crate::content::File;
    use crate::testing::{hacker, heard, plain, world};
    use crate::world::builds::Build;
    use crate::{Content, PlayerId, World};

    const START: u64 = 1_000_000;

    fn seized() -> (World, PlayerId) {
        let mut world = world();
        world.set_time(START);
        let (player, _) = world.connect("a");
        for line in ["nyx", "ghost"] {
            world.command(player, line);
        }
        if let Some(state) = world.players.get_mut(&player) {
            let mut payroll = File::new("payroll.db", vec!["Pay.".to_owned()]);
            payroll.value = 40;
            let memo = File::new("memo.txt", vec!["Hi.".to_owned()]);
            let blueprint = File::blueprint(Verb::Crack, 2);
            state.rig.files.extend([payroll, memo, blueprint]);
            state.rig.ram = 12;
            state.rig.cpu = 5;
            let build = Build {
                version: 3,
                quiet: true,
            };
            state.rig.builds = vec![(Verb::Crack, build)];
        }
        world.burn(player);
        for line in ["zer0", "breaker", "go bazaar"] {
            world.command(player, line);
        }
        (world, player)
    }

    #[test]
    fn a_burn_puts_the_seized_rig_up_for_auction() {
        let (mut world, player) = seized();
        let ends = "Top bid: none. Ends in 3 days.";
        assert_eq!(
            plain(&world.command(player, "lots")),
            [
                "The police auction these lots:".to_owned(),
                format!("Lot 1: payroll.db, data worth 40 cr. {ends}"),
                format!("Lot 2: crack-v2.blueprint, the blueprint for crack v2. {ends}"),
                format!("Lot 3: 12 GB of RAM. {ends}"),
                format!("Lot 4: a CPU with 5 cycles. {ends}"),
                format!("Lot 5: crack v3 with the quiet mod. {ends}"),
                "Type bid <lot> <credits>. The first bid must be 100 cr or more, and a new bid must beat the top bid by 10%.".to_owned(),
            ]
        );
        let news = plain(&world.command(player, "news | grep auction"));
        assert_eq!(news.len(), 1, "{news:?}");
        assert!(
            news[0].ends_with(" The police auction 5 lots from the rig of nyx."),
            "{news:?}"
        );
    }

    #[test]
    fn the_time_left_counts_down() {
        let (mut world, player) = seized();
        let left = |world: &mut World, at: u64| {
            world.set_time(START + at);
            plain(&world.command(player, "lots | grep \"Lot 3\""))
        };
        assert_eq!(
            left(&mut world, 86_400 + 1),
            ["Lot 3: 12 GB of RAM. Top bid: none. Ends in 2 days."]
        );
        assert_eq!(
            left(&mut world, 3 * 86_400 - 7_000),
            ["Lot 3: 12 GB of RAM. Top bid: none. Ends in 2 hours."]
        );
    }

    #[test]
    fn lots_work_only_at_the_auction_host() {
        let (mut world, player) = seized();
        world.command(player, "back");
        assert_eq!(
            plain(&world.command(player, "lots")),
            ["There is no auction here. The police auction is at bazaar."]
        );
        let city = "[city c]\nname = C\nstart = hub\n[host hub]\nname = Hub\ndesc = A hub.\n";
        let mut world = World::new(Content::parse(city).unwrap(), 0);
        let (player, _) = world.join("nyx");
        assert_eq!(
            plain(&world.command(player, "lots")),
            ["This city has no police auction."]
        );
    }

    fn bidder(world: &mut World, account: &str, handle: &str) -> PlayerId {
        let player = hacker(world, account, handle);
        world.command(player, "go bazaar");
        if let Some(state) = world.players.get_mut(&player) {
            state.credits = 500;
        }
        player
    }

    fn credits(world: &mut World, player: PlayerId) -> Vec<String> {
        plain(&world.command(player, "status | grep credits"))
    }

    #[test]
    fn a_bid_holds_the_credits_until_a_higher_bid_gives_them_back() {
        let (mut world, _) = seized();
        let moth = bidder(&mut world, "b", "moth");
        let wren = bidder(&mut world, "c", "wren");
        let held = "The police hold your credits until someone bids more or the auction ends.";
        assert_eq!(
            plain(&world.command(moth, "bid 3 100")),
            [format!("You bid 100 cr on lot 3. {held}")]
        );
        assert_eq!(credits(&mut world, moth), ["Credits: 400 cr"]);
        assert_eq!(
            plain(&world.command(moth, "lots | grep \"Lot 3\"")),
            ["Lot 3: 12 GB of RAM. Top bid: 100 cr, yours. Ends in 3 days."]
        );
        assert_eq!(
            plain(&world.command(wren, "bid 3 109")),
            ["Your bid must be 110 cr or more."]
        );
        let shown = world.command(wren, "bid 3 110");
        assert_eq!(
            heard(&shown, wren),
            [format!("You bid 110 cr on lot 3. {held}")]
        );
        assert_eq!(
            heard(&shown, moth),
            ["You lose the top bid on lot 3. The police give back your 100 cr."]
        );
        assert_eq!(credits(&mut world, moth), ["Credits: 500 cr"]);
        assert_eq!(
            plain(&world.command(moth, "lots | grep \"Lot 3\"")),
            ["Lot 3: 12 GB of RAM. Top bid: 110 cr. Ends in 3 days."]
        );
        assert_eq!(
            plain(&world.command(wren, "bid 3 200")),
            ["You already have the top bid on lot 3."]
        );
        assert_eq!(
            plain(&world.command(moth, "bid 3 600")),
            ["You have only 500 cr."]
        );
    }

    #[test]
    fn an_offline_bidder_gets_the_credits_back_in_the_save() {
        let (mut world, _) = seized();
        let moth = bidder(&mut world, "b", "moth");
        let wren = bidder(&mut world, "c", "wren");
        world.command(moth, "bid 3 100");
        world.leave(moth);
        world.command(wren, "bid 3 110");
        let (moth, _) = world.connect("b");
        assert_eq!(credits(&mut world, moth), ["Credits: 500 cr"]);
    }

    #[test]
    fn bid_checks_its_words_and_the_lot() {
        let (mut world, _) = seized();
        let moth = bidder(&mut world, "b", "moth");
        let usage = ["Usage: bid <lot> <credits>. Type lots to see the lots."];
        for line in [
            "bid",
            "bid 3",
            "bid three 100",
            "bid 3 lots",
            "bid 3 100 more",
        ] {
            assert_eq!(plain(&world.command(moth, line)), usage, "{line}");
        }
        assert_eq!(
            plain(&world.command(moth, "bid 9 100")),
            ["There is no lot 9. Type lots to see the lots."]
        );
        assert_eq!(
            plain(&world.command(moth, "bid 1 99")),
            ["Your bid must be 100 cr or more."]
        );
        world.command(moth, "back");
        assert_eq!(
            plain(&world.command(moth, "bid 1 100")),
            ["There is no auction here. The police auction is at bazaar."]
        );
    }

    #[test]
    fn lots_and_bids_come_back_from_the_save() {
        let (mut world, _) = seized();
        let moth = bidder(&mut world, "b", "moth");
        world.command(moth, "bid 3 150");
        let before = plain(&world.command(moth, "lots"));
        let saves = world.saves();
        let text = saves
            .iter()
            .find(|(path, _)| path == "auction.ini")
            .map_or_default(|(_, text)| text.clone());
        let ends = START + 3 * 86_400;
        for part in [
            "\n[auction police]\nnext = 5\n".to_owned(),
            format!("\n[lot 1]\nends = {ends}\nfile = payroll.db\nvalue = 40\nline = Pay.\n"),
            format!("\n[lot 3]\nends = {ends}\nram = 12\nbid = b 150\n"),
            format!("\n[lot 5]\nends = {ends}\nbuild = crack 3\nquiet = yes\n"),
        ] {
            assert!(text.contains(&part), "{part}\n{text}");
        }
        let mut fresh = crate::testing::world();
        fresh.set_time(START);
        for (path, text) in &saves {
            fresh.restore(path, text).unwrap();
        }
        let (moth, _) = fresh.connect("b");
        for line in ["go relay", "go bazaar"] {
            fresh.command(moth, line);
        }
        assert_eq!(plain(&fresh.command(moth, "lots")), before);
        assert_eq!(
            fresh
                .saves()
                .iter()
                .filter(|(path, _)| path == "auction.ini")
                .count(),
            0
        );
    }

    #[test]
    fn a_bad_auction_file_fails() {
        let mut world = world();
        let text = "[auction police]\nnext = 2\n[lot x]\nends = 5\nram = 12\n[lot 2]\nram = 12\n\
                    [lot 3]\nends = 9\n[thing y]\n";
        let errors: Vec<String> = world
            .restore("auction.ini", text)
            .unwrap_err()
            .iter()
            .map(ToString::to_string)
            .collect();
        assert_eq!(
            errors,
            [
                "line 3: bad lot number: x",
                "line 6: lot 2 has no ends",
                "line 8: lot 3 has no item",
                "line 10: unknown section type: thing",
            ]
        );
    }

    #[test]
    fn a_rig_with_nothing_of_value_makes_no_lots() {
        let mut world = world();
        let (player, _) = world.connect("a");
        for line in ["nyx", "ghost"] {
            world.command(player, line);
        }
        world.burn(player);
        for line in ["zer0", "breaker", "go bazaar"] {
            world.command(player, line);
        }
        assert_eq!(
            plain(&world.command(player, "lots")),
            ["The police have no lots for auction now."]
        );
    }
}
