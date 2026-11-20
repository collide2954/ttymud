use std::collections::BTreeMap;
use std::str::FromStr;

use super::account::{Account, Saved};
use super::alert::CorpState;
use super::bans::BANS;
use super::builds::Build;
use super::burns::Burn;
use super::contract::Job;
use super::crews::CREWS;
use super::moderation::{AUDIT, MODERATORS};
use super::news::NEWS;
use super::reports::REPORTS;
use super::rig::MAX_INTEGRITY;
use super::{PlayerId, World};
use crate::commands::{self, TOP_VERSION, Verb};
use crate::content::{ContentError, File};
use crate::heat::MAX_HEAT;
use crate::ini::{self, Pair, Section};
use crate::kits::{CYCLES, KITS, RAM};

const ACCOUNTS: &str = "accounts/";

type Corps = Vec<(usize, CorpState)>;
const BURNS: &str = "burns.ini";
const WORLD: &str = "world.ini";

impl World {
    pub fn saves(&mut self) -> Vec<(String, String)> {
        let dirty = std::mem::take(&mut self.dirty);
        let online: Vec<PlayerId> = self
            .players
            .iter()
            .filter(|(_, state)| {
                state
                    .account
                    .as_ref()
                    .is_some_and(|account| dirty.contains(account))
            })
            .map(|(&player, _)| player)
            .collect();
        for player in online {
            self.keep(player);
        }
        let mut saves: Vec<(String, String)> = dirty
            .iter()
            .filter_map(|id| {
                let account = self.accounts.get(id)?;
                Some((
                    format!("{ACCOUNTS}{id}.ini"),
                    self.account_text(id, account),
                ))
            })
            .collect();
        if std::mem::take(&mut self.burns_dirty) {
            saves.push((BURNS.to_owned(), self.burns_text()));
        }
        if std::mem::take(&mut self.state_dirty) {
            saves.push((WORLD.to_owned(), self.world_text()));
        }
        if std::mem::take(&mut self.news.dirty) {
            saves.push((NEWS.to_owned(), self.news_text()));
        }
        saves.extend(self.crews.save());
        saves.extend(self.moderation.save_audit());
        saves.extend(self.moderation.reports.save());
        saves.extend(self.moderation.bans.save());
        saves
    }

    pub fn restore(&mut self, path: &str, text: &str) -> Result<(), Vec<ContentError>> {
        if path == MODERATORS {
            return self.moderation.read_moderators(text);
        }
        if path == REPORTS {
            return self.moderation.reports.read(text);
        }
        if path == BANS {
            return self.moderation.bans.read(text);
        }
        if path == AUDIT {
            self.moderation.read_audit(text);
            return Ok(());
        }
        if path == WORLD {
            let (seed, corps) = self.parse_world(text)?;
            self.seed = seed;
            for (corp, state) in corps {
                self.corp_states[corp] = state;
                self.corp_states[corp].level = self.alert(corp);
            }
            self.state_dirty = false;
            return Ok(());
        }
        if path == NEWS {
            return self.restore_news(text);
        }
        if path == CREWS {
            self.crews.read(text)?;
            self.resume_heists();
            return Ok(());
        }
        if path == BURNS {
            self.burns = parse_burns(text)?;
            self.post_burns();
            return Ok(());
        }
        let Some(id) = path
            .strip_prefix(ACCOUNTS)
            .and_then(|rest| rest.strip_suffix(".ini"))
        else {
            return Err(vec![problem(0, format!("unknown save file: {path}"))]);
        };
        let account = self.parse_account(id, text)?;
        self.accounts.insert(id.to_owned(), account);
        Ok(())
    }

    pub(super) fn touch(&mut self, player: PlayerId) {
        if let Some(account) = self
            .players
            .get(&player)
            .and_then(|state| state.account.clone())
        {
            self.dirty.insert(account);
        }
    }

    fn account_text(&self, id: &str, account: &Account) -> String {
        let mut text = Text::new("# A TTYmud account. The server writes this file.");
        text.section("account", id);
        let social = &account.social;
        text.pair("created", &social.created.to_string());
        if social.warned {
            text.pair("warned", "yes");
        }
        if social.muted > self.now {
            text.pair("muted", &social.muted.to_string());
        }
        for key in &social.ignores {
            text.pair("ignore", key);
        }
        for key in &social.blocks {
            text.pair("block", key);
        }
        for &verb in &account.know {
            text.pair("know", commands::name_of(verb));
        }
        for &(verb, version) in &account.blueprints {
            let name = commands::name_of(verb);
            text.pair("blueprint", &format!("{name} {version}"));
        }
        if let Some(handle) = &account.handle {
            self.handle_text(&mut text, handle);
        }
        for file in &account.stash {
            text.file("stash", file);
        }
        text.finish()
    }

    fn handle_text(&self, text: &mut Text, handle: &Saved) {
        text.section("handle", &handle.name);
        text.pair("kit", handle.kit.id);
        text.pair("heat", &handle.heat.to_string());
        text.pair("cooled", &handle.cooled.to_string());
        text.pair("integrity", &handle.integrity.to_string());
        text.pair("credits", &handle.credits.to_string());
        text.pair("ram", &handle.ram.to_string());
        text.pair("cpu", &handle.cpu.to_string());
        if let Some(channels) = &handle.channels {
            text.pair("channels", &channels.join(", "));
        }
        for name in &handle.sold {
            text.pair("sold", name);
        }
        for (fixer, rep) in &handle.rep {
            text.pair("rep", &format!("{fixer} {rep}"));
        }
        for &verb in &handle.loads {
            text.pair("load", commands::name_of(verb));
        }
        for &(verb, build) in &handle.builds {
            let name = commands::name_of(verb);
            if build.version > 1 {
                text.pair("version", &format!("{name} {}", build.version));
            }
            if build.quiet {
                text.pair("quiet", name);
            }
        }
        let contract = handle
            .contract
            .and_then(|job| Some((job, self.contracts.get(job.contract())?)));
        match (contract, &handle.posting) {
            (Some((job, contract)), _) => {
                if let Some((day, fixer, slot)) = contract.posting {
                    let id = self.fixers.get(fixer).map_or("", |fixer| fixer.id.as_str());
                    text.pair("job", &format!("{id} {day} {slot}"));
                } else {
                    text.pair("contract", &contract.id);
                    text.pair("step", &job.progress().to_string());
                }
            }
            (None, Some((fixer, day, slot))) => text.pair("job", &format!("{fixer} {day} {slot}")),
            (None, None) => {}
        }
        for (day, fixer, slot) in &handle.done {
            text.pair("done", &format!("{fixer} {day} {slot}"));
        }
        for file in &handle.files {
            text.file("file", file);
        }
    }

    fn world_text(&self) -> String {
        let mut text = Text::new("# The TTYmud world. The server writes this file.");
        text.section("world", &self.city_id);
        text.pair("seed", &self.seed.to_string());
        for (corp, state) in self.corp_states.iter().enumerate() {
            self.corp_text(&mut text, corp, state);
        }
        text.finish()
    }

    fn corp_text(&self, text: &mut Text, corp: usize, state: &CorpState) {
        let alert = self.alert_points(corp) > 0;
        let patches: Vec<(&String, &u64)> = state
            .patched
            .iter()
            .filter(|&(_, &until)| until > self.now)
            .collect();
        if !alert && state.used.is_empty() && patches.is_empty() {
            return;
        }
        text.section("corp", &self.corps[corp].id);
        if alert {
            text.pair("alert", &state.alert.to_string());
            text.pair("since", &state.since.to_string());
        }
        for (software, uses) in &state.used {
            text.pair("used", &format!("{software}: {uses}"));
        }
        for (software, until) in patches {
            text.pair("patched", &format!("{software}: {until}"));
        }
    }

    fn parse_world(&self, text: &str) -> Result<(u64, Corps), Vec<ContentError>> {
        let (sections, mut errors) = ini::read(text);
        let (mut seed, mut corps) = (None, Vec::new());
        for section in &sections {
            match section.kind {
                "world" if section.id == self.city_id => {
                    allow(section, &["seed"], &mut errors);
                    let pair = section.pairs.iter().find(|pair| pair.key == "seed");
                    seed = pair.and_then(|pair| number(pair, &mut errors));
                }
                "world" => {
                    let (id, city) = (section.id, &self.city_id);
                    errors.push(problem(
                        section.line,
                        format!("world {id} is not this city: {city}"),
                    ));
                }
                "corp" => {
                    let state = corp_state(section, &mut errors);
                    let corp = self.corps.iter().position(|corp| corp.id == section.id);
                    corps.extend(corp.map(|corp| (corp, state)));
                }
                kind => errors.push(problem(
                    section.line,
                    format!("unknown section type: {kind}"),
                )),
            }
        }
        if !sections.iter().any(|section| section.kind == "world") {
            errors.push(problem(0, format!("{WORLD} has no world section")));
        }
        checked(errors, (seed.unwrap_or_default(), corps))
    }

    fn burns_text(&self) -> String {
        let mut text = Text::new("# The burn list. The server writes this file.");
        for burn in &self.burns {
            text.section("burn", &burn.name);
            text.pair("when", &burn.when.to_string());
        }
        text.finish()
    }

    fn parse_account(&self, id: &str, text: &str) -> Result<Account, Vec<ContentError>> {
        let (sections, mut errors) = ini::read(text);
        let mut account = Account::default();
        let mut found = false;
        let mut files = Vec::new();
        for section in &sections {
            match section.kind {
                "account" if section.id == id => {
                    found = true;
                    read_account(section, &mut account, &mut errors);
                }
                "account" => {
                    let other = section.id;
                    errors.push(problem(
                        section.line,
                        format!("account {other} is not the account of this file"),
                    ));
                }
                "handle" => account.handle = self.parse_handle(section, &mut errors),
                "file" => files.push(parse_file(section, &mut errors)),
                "stash" => account.stash.push(parse_file(section, &mut errors)),
                other => errors.push(problem(
                    section.line,
                    format!("unknown section type: {other}"),
                )),
            }
        }
        if !found {
            errors.push(problem(0, format!("no account section for {id}")));
        }
        if let Some(handle) = &mut account.handle {
            handle.files = files;
        }
        checked(errors, account)
    }

    fn parse_handle(&self, section: &Section<'_>, errors: &mut Vec<ContentError>) -> Option<Saved> {
        let name = section.id;
        let mut kit = None;
        let (mut heat, mut cooled, mut integrity): (Option<u32>, Option<u64>, Option<u32>) =
            (None, None, None);
        let (mut contract, mut step, mut loads) = (None, 0, Vec::new());
        let (mut credits, mut ram, mut cpu) = (0, RAM, CYCLES);
        let (mut sold, mut rep, mut done, mut posting) =
            (Vec::new(), BTreeMap::new(), Vec::new(), None);
        let (mut channels, mut builds) = (None, Vec::new());
        for pair in &section.pairs {
            match pair.key {
                "sold" => sold.push(pair.value.to_owned()),
                "version" => {
                    if let Some((verb, version)) = program_version(pair, errors) {
                        build_of(&mut builds, verb).version = version;
                    }
                }
                "quiet" => {
                    if let Some(verb) = program(pair, errors) {
                        build_of(&mut builds, verb).quiet = true;
                    }
                }
                "channels" => {
                    let names = pair.value.split(',').map(str::trim);
                    channels = Some(
                        names
                            .filter(|name| !name.is_empty())
                            .map(str::to_owned)
                            .collect(),
                    );
                }
                "rep" => rep.extend(reputation(pair, errors)),
                "job" => posting = posting_of(pair, errors),
                "done" => done
                    .extend(posting_of(pair, errors).map(|(fixer, day, slot)| (day, fixer, slot))),
                "credits" => credits = number(pair, errors).unwrap_or(0),
                "ram" => ram = number(pair, errors).unwrap_or(RAM),
                "cpu" => cpu = number(pair, errors).unwrap_or(CYCLES),
                "kit" => {
                    kit = KITS.iter().find(|kit| kit.id == pair.value);
                    if kit.is_none() {
                        errors.push(problem(pair.line, format!("unknown kit: {}", pair.value)));
                    }
                }
                "heat" => heat = number(pair, errors),
                "cooled" => cooled = number(pair, errors),
                "integrity" => integrity = number(pair, errors),
                "load" => loads.extend(program(pair, errors)),
                "contract" => {
                    contract = self
                        .contracts
                        .iter()
                        .position(|known| known.id == pair.value);
                }
                "step" => step = number(pair, errors).unwrap_or(0),
                other => errors.push(problem(
                    pair.line,
                    format!("unknown key in handle {name}: {other}"),
                )),
            }
        }
        for key in ["kit", "heat", "cooled", "integrity"] {
            if !section.pairs.iter().any(|pair| pair.key == key) {
                errors.push(problem(section.line, format!("handle {name} has no {key}")));
            }
        }
        Some(Saved {
            name: name.to_owned(),
            kit: kit?,
            heat: heat?.min(MAX_HEAT),
            cooled: cooled?,
            integrity: integrity?.min(MAX_INTEGRITY),
            credits,
            ram,
            cpu,
            sold,
            rep,
            done,
            posting,
            channels,
            files: Vec::new(),
            loads,
            builds,
            contract: contract.map(|index| Job::resume(index, step)),
        })
    }
}

fn build_of(builds: &mut Vec<(Verb, Build)>, verb: Verb) -> &mut Build {
    let found = builds.iter().position(|&(known, _)| known == verb);
    let index = found.unwrap_or_else(|| {
        builds.push((verb, Build::default()));
        builds.len() - 1
    });
    &mut builds[index].1
}

fn program_version(pair: &Pair<'_>, errors: &mut Vec<ContentError>) -> Option<(Verb, u32)> {
    let parsed = pair.value.split_once(' ').and_then(|(name, version)| {
        let verb = commands::find(name)
            .map(|command| command.verb)
            .filter(|verb| verb.power().is_some())?;
        let version = version.trim().parse().ok()?;
        (2..=TOP_VERSION)
            .contains(&version)
            .then_some((verb, version))
    });
    if parsed.is_none() {
        let (key, value) = (pair.key, pair.value);
        errors.push(problem(
            pair.line,
            format!("bad {key}: {value} (use a program and 2 or 3)"),
        ));
    }
    parsed
}

pub(super) struct Text(Vec<String>);

impl Text {
    pub(super) fn new(comment: &str) -> Self {
        Self(vec![comment.to_owned()])
    }

    pub(super) fn section(&mut self, kind: &str, id: &str) {
        self.0.push(String::new());
        self.0.push(format!("[{kind} {id}]"));
    }

    pub(super) fn pair(&mut self, key: &str, value: &str) {
        self.0.push(format!("{key} = {value}"));
    }

    fn file(&mut self, kind: &str, file: &File) {
        self.section(kind, &file.name);
        for line in &file.lines {
            self.pair("line", line);
        }
        if file.value > 0 {
            self.pair("value", &file.value.to_string());
        }
    }

    pub(super) fn finish(self) -> String {
        let mut text = self.0.join("\n");
        text.push('\n');
        text
    }
}

fn parse_burns(text: &str) -> Result<Vec<Burn>, Vec<ContentError>> {
    let (sections, mut errors) = ini::read(text);
    let mut burns = Vec::new();
    for section in &sections {
        if section.kind != "burn" {
            let kind = section.kind;
            errors.push(problem(
                section.line,
                format!("unknown section type: {kind}"),
            ));
            continue;
        }
        allow(section, &["when"], &mut errors);
        let Some(pair) = section.pairs.iter().find(|pair| pair.key == "when") else {
            let name = section.id;
            errors.push(problem(section.line, format!("burn {name} has no when")));
            continue;
        };
        if let Some(when) = number(pair, &mut errors) {
            burns.push(Burn {
                name: section.id.to_owned(),
                when,
            });
        }
    }
    checked(errors, burns)
}

fn corp_state(section: &Section<'_>, errors: &mut Vec<ContentError>) -> CorpState {
    allow(section, &["alert", "since", "used", "patched"], errors);
    let mut state = CorpState::default();
    for pair in &section.pairs {
        match pair.key {
            "alert" => state.alert = number(pair, errors).unwrap_or_default(),
            "since" => state.since = number(pair, errors).unwrap_or_default(),
            "used" => state.used.extend(software_number(pair, errors)),
            "patched" => state.patched.extend(software_number(pair, errors)),
            _ => {}
        }
    }
    state
}

fn software_number<T: FromStr>(
    pair: &Pair<'_>,
    errors: &mut Vec<ContentError>,
) -> Option<(String, T)> {
    let parsed = pair.value.rsplit_once(':').and_then(|(name, value)| {
        let name = name.trim();
        let value = value.trim().parse().ok()?;
        (!name.is_empty()).then(|| (name.to_owned(), value))
    });
    if parsed.is_none() {
        let (key, value) = (pair.key, pair.value);
        let usage = "use a software name, a colon, and a number";
        errors.push(problem(pair.line, format!("bad {key}: {value} ({usage})")));
    }
    parsed
}

fn parse_file(section: &Section<'_>, errors: &mut Vec<ContentError>) -> File {
    allow(section, &["line", "value"], errors);
    let value = section
        .pairs
        .iter()
        .find(|pair| pair.key == "value")
        .and_then(|pair| number(pair, errors))
        .unwrap_or(0);
    let lines: Vec<String> = section
        .pairs
        .iter()
        .filter(|pair| pair.key == "line")
        .map(|pair| pair.value.to_owned())
        .collect();
    if lines.is_empty() {
        let (kind, name) = (section.kind, section.id);
        errors.push(problem(section.line, format!("{kind} {name} has no line")));
    }
    let mut file = File::new(section.id, lines);
    file.value = value;
    file
}

pub(super) fn allow(section: &Section<'_>, keys: &[&str], errors: &mut Vec<ContentError>) {
    for pair in section
        .pairs
        .iter()
        .filter(|pair| !keys.contains(&pair.key))
    {
        let (kind, id, key) = (section.kind, section.id, pair.key);
        errors.push(problem(
            pair.line,
            format!("unknown key in {kind} {id}: {key}"),
        ));
    }
}

fn program(pair: &Pair<'_>, errors: &mut Vec<ContentError>) -> Option<Verb> {
    let verb = commands::find(pair.value)
        .map(|command| command.verb)
        .filter(|verb| verb.program().is_some());
    if verb.is_none() {
        errors.push(problem(
            pair.line,
            format!("unknown program: {}", pair.value),
        ));
    }
    verb
}

fn read_account(section: &Section<'_>, account: &mut Account, errors: &mut Vec<ContentError>) {
    allow(
        section,
        &[
            "know",
            "blueprint",
            "created",
            "warned",
            "muted",
            "ignore",
            "block",
        ],
        errors,
    );
    let values = |key: &'static str| section.pairs.iter().filter(move |pair| pair.key == key);
    account.know = values("know")
        .filter_map(|pair| program(pair, errors))
        .collect();
    account.blueprints = values("blueprint")
        .filter_map(|pair| program_version(pair, errors))
        .collect();
    let mut time = |key| -> u64 {
        let pair = values(key).next();
        pair.and_then(|pair| number(pair, errors)).unwrap_or(0)
    };
    let social = &mut account.social;
    social.created = time("created");
    social.muted = time("muted");
    social.warned = values("warned").any(|pair| pair.value == "yes");
    social.ignores = values("ignore").map(|pair| pair.value.to_owned()).collect();
    social.blocks = values("block").map(|pair| pair.value.to_owned()).collect();
}

fn posting_of(pair: &Pair<'_>, errors: &mut Vec<ContentError>) -> Option<(String, u64, u64)> {
    let words: Vec<&str> = pair.value.split_whitespace().collect();
    let parsed = match words.as_slice() {
        [fixer, day, slot] => day
            .parse()
            .ok()
            .zip(slot.parse().ok())
            .map(|(day, slot)| ((*fixer).to_owned(), day, slot)),
        _ => None,
    };
    if parsed.is_none() {
        let (key, value) = (pair.key, pair.value);
        let usage = "use a fixer, a day, and a slot";
        errors.push(problem(pair.line, format!("bad {key}: {value} ({usage})")));
    }
    parsed
}

fn reputation(pair: &Pair<'_>, errors: &mut Vec<ContentError>) -> Option<(String, u32)> {
    let words: Vec<&str> = pair.value.split_whitespace().collect();
    let parsed = match words.as_slice() {
        [fixer, rep] => rep.parse().ok().map(|rep| ((*fixer).to_owned(), rep)),
        _ => None,
    };
    if parsed.is_none() {
        let value = pair.value;
        errors.push(problem(
            pair.line,
            format!("bad rep: {value} (use a fixer and a number)"),
        ));
    }
    parsed
}

pub(super) fn number<T: FromStr>(pair: &Pair<'_>, errors: &mut Vec<ContentError>) -> Option<T> {
    let value = pair.value.parse().ok();
    if value.is_none() {
        errors.push(problem(pair.line, format!("bad number: {}", pair.value)));
    }
    value
}

pub(super) fn problem(line: usize, message: String) -> ContentError {
    ContentError { line, message }
}

pub(super) fn checked<T>(mut errors: Vec<ContentError>, value: T) -> Result<T, Vec<ContentError>> {
    errors.sort_by_key(|error| error.line);
    if errors.is_empty() {
        Ok(value)
    } else {
        Err(errors)
    }
}

#[cfg(test)]
mod tests {
    use crate::testing::{hired_world, plain, world};
    use crate::{PlayerId, World};

    fn ready(world: &mut World, lines: &[&str]) -> PlayerId {
        let (player, _) = world.connect("a");
        for line in ["nyx", "ghost"].iter().chain(lines) {
            world.command(player, line);
        }
        player
    }

    fn file<'a>(saves: &'a [(String, String)], path: &str) -> Option<&'a str> {
        saves
            .iter()
            .find(|(name, _)| name == path)
            .map(|(_, text)| text.as_str())
    }

    #[test]
    fn an_account_is_saved_as_plain_text() {
        let mut world = world();
        world.set_time(500);
        let player = ready(
            &mut world,
            &["get motd.txt", "go back-room", "get motd.txt"],
        );
        world.leave(player);
        let saves = world.saves();
        let expected = [
            "# A TTYmud account. The server writes this file.",
            "",
            "[account a]",
            "created = 500",
            "know = scan",
            "know = probe",
            "know = crack",
            "know = scrub",
            "know = mask",
            "",
            "[handle nyx]",
            "kit = ghost",
            "heat = 0",
            "cooled = 0",
            "integrity = 100",
            "credits = 0",
            "ram = 8",
            "cpu = 4",
            "channels = city",
            "load = scan",
            "load = probe",
            "load = crack",
            "load = scrub",
            "load = mask",
            "",
            "[file motd.txt]",
            "line = Welcome to the Relay.",
            "line = Be polite.",
            "",
        ]
        .join("\n");
        assert_eq!(file(&saves, "accounts/a.ini"), Some(expected.as_str()));
    }

    #[test]
    fn an_account_comes_back_from_its_save() {
        let mut world = hired_world();
        let player = ready(
            &mut world,
            &[
                "get motd.txt",
                "go back-room",
                "stash motd.txt",
                "join trade",
                "leave city",
            ],
        );
        if let Some(state) = world.players.get_mut(&player) {
            state.heat = 2;
            state.cooled = 7;
            state.rig.integrity = 61;
            state.credits = 120;
            state.rig.ram = 12;
            state.rig.cpu = 5;
            state.rep.insert("pell".to_owned(), 4);
        }
        world.leave(player);
        let saves = world.saves();
        let mut fresh = hired_world();
        for (path, text) in &saves {
            fresh.restore(path, text).unwrap();
        }
        let (player, outputs) = fresh.connect("a");
        assert_eq!(plain(&outputs)[0], "Welcome back, nyx.");
        let status = plain(&fresh.command(player, "status"));
        assert_eq!(
            status[..3],
            ["Heat: 2/5", "Credits: 120 cr", "Integrity: 61/100"]
        );
        assert_eq!(
            plain(&fresh.command(player, "status | grep gb")),
            ["RAM: 8/12 GB"]
        );
        assert_eq!(
            plain(&fresh.command(player, "status | grep cycles")),
            ["Cycles: 5/5"]
        );
        assert_eq!(
            plain(&fresh.command(player, "channels | grep \"in this\"")),
            ["#trade: Buy and sell. You are in this channel."]
        );
        assert_eq!(fresh.players.get(&player).map(|state| state.rep.clone()), {
            let mut rep = std::collections::BTreeMap::new();
            rep.insert("pell".to_owned(), 4);
            Some(rep)
        });
        assert_eq!(
            plain(&fresh.command(player, "stash")),
            ["Your stash: motd.txt"]
        );
        assert_eq!(
            plain(&fresh.command(player, "contract"))
                .last()
                .map(String::as_str),
            Some("Next: Type go gate to connect to the lab.")
        );
    }

    #[test]
    fn the_burn_list_comes_back_from_its_save() {
        let mut world = world();
        world.set_time(1_700_000_000);
        let player = ready(&mut world, &["go back-room", "go gate", "go core"]);
        if let Some(state) = world.players.get_mut(&player) {
            state.heat = 4;
            if let Some(run) = state.run.as_mut() {
                run.trace = 98;
            }
        }
        world.command(player, "scan");
        let saves = world.saves();
        assert_eq!(
            file(&saves, "burns.ini"),
            Some(
                "# The burn list. The server writes this file.\n\n[burn nyx]\nwhen = 1700000000\n"
            )
        );
        let mut fresh = world_from(&saves);
        let (player, _) = fresh.connect("b");
        assert_eq!(
            plain(&fresh.command(player, "nyx")),
            ["The handle nyx burned. Nobody can use it again."]
        );
    }

    fn world_from(saves: &[(String, String)]) -> World {
        let mut fresh = world();
        for (path, text) in saves {
            fresh.restore(path, text).unwrap();
        }
        fresh
    }

    #[test]
    fn stolen_data_and_sales_come_back_from_a_save() {
        let mut world = hired_world();
        let player = ready(&mut world, &["go market"]);
        let mut file = crate::content::File::new("payroll.db", vec!["Pay.".to_owned()]);
        file.value = 40;
        if let Some(state) = world.players.get_mut(&player) {
            state.rig.files.push(file.clone());
            let clients = vec!["Clients.".to_owned()];
            state
                .rig
                .files
                .push(crate::content::File::new("clients.db", clients));
        }
        world.command(player, "sell payroll.db");
        if let Some(state) = world.players.get_mut(&player) {
            state.rig.files[0].value = 100;
            state.rig.files.push(file);
        }
        world.leave(player);
        let mut fresh = world_from(&world.saves());
        let (player, _) = fresh.connect("a");
        for line in ["go relay", "go market"] {
            fresh.command(player, line);
        }
        assert_eq!(
            plain(&fresh.command(player, "sell")),
            ["Usage: sell <file>. The fence wants: clients.db (100 cr), payroll.db (40 cr)"]
        );
        assert_eq!(
            plain(&fresh.command(player, "sell payroll.db")),
            ["The fence already bought payroll.db from you in this job."]
        );
    }

    #[test]
    fn a_job_from_the_board_comes_back_from_a_save() {
        let day = 20_000 * 86_400 + 100;
        let mut world = world();
        world.set_time(day);
        let player = ready(&mut world, &["go bazaar", "accept 1"]);
        if let Some(state) = world.players.get_mut(&player) {
            state.done.push((20_000, "pell".to_owned(), 2));
        }
        let before = plain(&world.command(player, "contract"));
        world.leave(player);
        let saves = world.saves();
        let text = file(&saves, "accounts/a.ini").unwrap_or_default();
        assert!(text.contains("\njob = pell 20000 0\n"), "{text}");
        assert!(text.contains("\ndone = pell 20000 2\n"), "{text}");
        let mut fresh = world_from(&saves);
        fresh.set_time(day + 86_400);
        let (player, _) = fresh.connect("a");
        assert_eq!(plain(&fresh.command(player, "contract")), before);
        assert_eq!(
            fresh.players.get(&player).map(|state| state.done.clone()),
            Some(vec![(20_000, "pell".to_owned(), 2)])
        );
    }

    #[test]
    fn an_account_from_before_credits_has_no_credits() {
        let mut world = world();
        let text =
            "[account a]\n[handle nyx]\nkit = ghost\nheat = 0\ncooled = 0\nintegrity = 100\n";
        world.restore("accounts/a.ini", text).unwrap();
        let (player, _) = world.connect("a");
        assert_eq!(
            plain(&world.command(player, "status | grep credits")),
            ["Credits: 0 cr"]
        );
        assert_eq!(
            plain(&world.command(player, "channels | grep \"in this\"")),
            ["#city: All of the test city. You are in this channel."]
        );
    }

    #[test]
    fn a_new_world_saves_its_seed_one_time() {
        let mut world = World::new(crate::Content::parse(crate::testing::CITY).unwrap(), 42);
        let saves = world.saves();
        assert_eq!(
            file(&saves, "world.ini"),
            Some("# The TTYmud world. The server writes this file.\n\n[world test]\nseed = 42\n")
        );
        assert_eq!(world.saves(), Vec::<(String, String)>::new());
    }

    #[test]
    fn the_seed_comes_back_from_its_save() {
        let mut world = world();
        world
            .restore("world.ini", "[world test]\nseed = 7\n")
            .unwrap();
        assert_eq!(world.seed, 7);
        assert_eq!(world.saves(), Vec::<(String, String)>::new());
    }

    #[test]
    fn a_world_file_must_be_for_this_city() {
        let mut world = world();
        let errors = |world: &mut World, text: &str| -> Vec<String> {
            let result = world.restore("world.ini", text);
            result
                .unwrap_err()
                .iter()
                .map(ToString::to_string)
                .collect()
        };
        assert_eq!(
            errors(&mut world, "[world other]\nseed = 7\n"),
            ["line 1: world other is not this city: test"]
        );
        assert_eq!(
            errors(&mut world, "[world test]\nseed = x\ncolor = red\n"),
            [
                "line 2: bad number: x",
                "line 3: unknown key in world test: color"
            ]
        );
        assert_eq!(errors(&mut world, ""), ["world.ini has no world section"]);
    }

    #[test]
    fn saves_give_only_what_changed() {
        let mut world = world();
        let _ = world.saves();
        let player = ready(&mut world, &[]);
        assert_eq!(world.saves().len(), 1);
        assert_eq!(world.saves(), Vec::<(String, String)>::new());
        world.command(player, "look");
        assert_eq!(world.saves().len(), 1);
    }

    #[test]
    fn a_restored_handle_name_is_taken() {
        let mut world = world();
        let player = ready(&mut world, &[]);
        world.leave(player);
        let mut fresh = world_from(&world.saves());
        let (player, _) = fresh.connect("b");
        assert_eq!(
            plain(&fresh.command(player, "NYX")),
            ["The handle NYX is taken."]
        );
    }

    #[test]
    fn bad_save_files_fail_with_line_numbers() {
        let mut world = world();
        let text = "[account a]\nknow = nuke\n[handle nyx]\nkit = wizard\nheat = hot\n";
        let errors: Vec<String> = world
            .restore("accounts/a.ini", text)
            .unwrap_err()
            .iter()
            .map(ToString::to_string)
            .collect();
        assert_eq!(
            errors,
            [
                "line 2: unknown program: nuke",
                "line 3: handle nyx has no cooled",
                "line 3: handle nyx has no integrity",
                "line 4: unknown kit: wizard",
                "line 5: bad number: hot",
            ]
        );
        assert!(world.restore("other.ini", "").is_err());
        assert!(world.restore("accounts/a.ini", "[account b]\n").is_err());
    }
}
