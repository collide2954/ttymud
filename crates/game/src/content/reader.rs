use super::{ContentError, ContractDef, File, HostDef, IceDef, NetworkDef};
use crate::contract::Event;
use crate::heat::Security;
use crate::ice::IceKind;
use crate::ini::{self, Section};

#[derive(Clone, Copy)]
enum Name {
    Plain,
    File,
}

#[derive(Default)]
pub(super) struct Reader {
    pub(super) errors: Vec<ContentError>,
    pub(super) city: Option<usize>,
    pub(super) city_id: String,
    pub(super) city_name: Option<String>,
    pub(super) start: Option<(String, usize)>,
    pub(super) home: Option<(String, usize)>,
    pub(super) burns: Option<(String, usize)>,
    pub(super) contract: Option<(String, usize)>,
    pub(super) contracts: Vec<(ContractDef, usize, usize)>,
    pub(super) networks: Vec<(NetworkDef, usize)>,
    pub(super) hosts: Vec<(HostDef, usize)>,
    pub(super) members: Vec<(String, String, usize)>,
    pub(super) routes: Vec<(String, String, usize)>,
    pub(super) files: Vec<(String, File, usize)>,
    pub(super) ice: Vec<(String, IceDef, usize)>,
}

impl Reader {
    pub(super) fn read(text: &str) -> Self {
        let (sections, errors) = ini::read(text);
        let mut reader = Self {
            errors,
            ..Self::default()
        };
        for section in &sections {
            reader.section(section);
        }
        reader
    }

    pub(super) fn error(&mut self, line: usize, message: impl Into<String>) {
        self.errors.push(ContentError {
            line,
            message: message.into(),
        });
    }

    fn section(&mut self, section: &Section<'_>) {
        match section.kind {
            "city" => self.city(section),
            "network" => self.network(section),
            "host" => self.host(section),
            "file" => self.file(section),
            "ice" => self.ice(section),
            "contract" => self.contract(section),
            other => self.error(section.line, format!("unknown section type: {other}")),
        }
    }

    fn city(&mut self, section: &Section<'_>) {
        if self.city.is_some() {
            self.error(section.line, "more than one city section");
            return;
        }
        self.city = Some(section.line);
        section.id.clone_into(&mut self.city_id);
        self.allow(section, &["name", "start", "home", "burns", "contract"]);
        self.check_name(section.line, section.id, Name::Plain);
        self.city_name = self.single(section, "name").map(|(name, _)| name);
        self.start = self.single(section, "start");
        self.home = self.optional(section, "home");
        self.burns = self.optional(section, "burns");
        self.contract = self.optional(section, "contract");
    }

    fn contract(&mut self, section: &Section<'_>) {
        let id = section.id;
        if self.contracts.iter().any(|(def, ..)| def.id == id) {
            self.error(
                section.line,
                format!("contract {id} is defined more than once"),
            );
            return;
        }
        self.allow(
            section,
            &["fixer", "network", "goal", "offer", "step", "pay"],
        );
        self.check_name(section.line, id, Name::Plain);
        let fixer = self.single(section, "fixer");
        let network = self.single(section, "network");
        let goal = self.single(section, "goal");
        let pay = match self.optional(section, "pay") {
            None => 0,
            Some((word, line)) => word.parse().unwrap_or_else(|_| {
                self.error(line, format!("bad pay: {word} (use a number of credits)"));
                0
            }),
        };
        let offer: Vec<String> = values(section, "offer")
            .into_iter()
            .map(|(text, _)| text)
            .collect();
        if offer.is_empty() {
            self.error(section.line, format!("contract {id} has no offer"));
        }
        let steps = values(section, "step")
            .into_iter()
            .filter_map(|(text, line)| self.step(&text, line))
            .collect();
        if let (Some((fixer, _)), Some((network, at)), Some((goal, goal_at))) =
            (fixer, network, goal)
        {
            let def = ContractDef {
                id: id.to_owned(),
                fixer,
                network,
                goal,
                offer,
                steps,
                pay,
            };
            self.contracts.push((def, at, goal_at));
        }
    }

    fn step(&mut self, text: &str, line: usize) -> Option<(Event, String)> {
        let Some((word, hint)) = text
            .split_once(':')
            .filter(|(_, hint)| !hint.trim().is_empty())
        else {
            let message = "a step needs an event and a hint, for example step = scan: Type scan.";
            self.error(line, message);
            return None;
        };
        let word = word.trim();
        let Some(event) = Event::find(word) else {
            let events = "home, enter, scan, probe, crack, download, scrub, patch, or exit";
            self.error(line, format!("unknown step: {word} (use {events})"));
            return None;
        };
        Some((event, hint.trim().to_owned()))
    }

    fn network(&mut self, section: &Section<'_>) {
        if self
            .networks
            .iter()
            .any(|(network, _)| network.id == section.id)
        {
            let id = section.id;
            self.error(
                section.line,
                format!("network {id} is defined more than once"),
            );
            return;
        }
        self.allow(section, &["name", "security"]);
        self.check_name(section.line, section.id, Name::Plain);
        let name = self.single(section, "name").unwrap_or_default().0;
        let security = match self.optional(section, "security") {
            None => Security::Low,
            Some((word, line)) => Security::find(&word).unwrap_or_else(|| {
                let levels = "low, medium, high, or top";
                self.error(
                    line,
                    format!("unknown security level: {word} (use {levels})"),
                );
                Security::Low
            }),
        };
        let network = NetworkDef {
            id: section.id.to_owned(),
            name,
            security,
        };
        self.networks.push((network, section.line));
    }

    fn host(&mut self, section: &Section<'_>) {
        if self.hosts.iter().any(|(host, _)| host.id == section.id) {
            let id = section.id;
            self.error(section.line, format!("host {id} is defined more than once"));
            return;
        }
        self.allow(section, &["name", "desc", "network", "route"]);
        self.check_name(section.line, section.id, Name::Plain);
        let name = self.single(section, "name");
        let description = self.single(section, "desc");
        let network = self.optional(section, "network");
        if let Some((network, line)) = &network {
            self.members
                .push((section.id.to_owned(), network.clone(), *line));
        }
        let routes = values(section, "route");
        for (target, line) in &routes {
            self.routes
                .push((section.id.to_owned(), target.clone(), *line));
        }
        let host = HostDef {
            id: section.id.to_owned(),
            name: name.unwrap_or_default().0,
            description: description.unwrap_or_default().0,
            network: network.map(|(network, _)| network),
            routes: routes.into_iter().map(|(target, _)| target).collect(),
            files: Vec::new(),
            ice: Vec::new(),
        };
        self.hosts.push((host, section.line));
    }

    fn ice(&mut self, section: &Section<'_>) {
        self.allow(section, &["type", "version", "guards"]);
        let Some((host, name)) = section.id.split_once('/') else {
            self.error(
                section.line,
                "ice needs a host and a name, for example [ice gate/sentry]",
            );
            return;
        };
        let guards = self.optional(section, "guards");
        self.check_name(section.line, host, Name::Plain);
        self.check_name(section.line, name, Name::Plain);
        let kind = self.single(section, "type").and_then(|(word, line)| {
            let kind = IceKind::find(&word);
            if kind.is_none() {
                self.error(
                    line,
                    format!("unknown ICE type: {word} (use sentry, warden, hound, or scorch)"),
                );
            }
            kind
        });
        let version = match self.optional(section, "version") {
            None => Some(1),
            Some((word, line)) => {
                let version = word
                    .parse()
                    .ok()
                    .filter(|version| (1..=9).contains(version));
                if version.is_none() {
                    self.error(line, format!("bad ICE version: {word} (use 1 to 9)"));
                }
                version
            }
        };
        match (kind, &guards) {
            (Some(IceKind::Warden), None) => {
                let id = section.id;
                self.error(section.line, format!("ice {id} has no guards"));
            }
            (Some(kind), Some((_, line))) if kind != IceKind::Warden => {
                self.error(*line, "only a warden can guard a route");
            }
            _ => {}
        }
        if let (Some(kind), Some(version)) = (kind, version) {
            let ice = IceDef {
                name: name.to_owned(),
                kind,
                version,
                guards: guards.map(|(target, _)| target),
            };
            self.ice.push((host.to_owned(), ice, section.line));
        }
    }

    fn file(&mut self, section: &Section<'_>) {
        self.allow(section, &["line"]);
        let Some((host, name)) = section.id.split_once('/') else {
            self.error(
                section.line,
                "file needs a host and a name, for example [file relay/motd.txt]",
            );
            return;
        };
        self.check_name(section.line, host, Name::Plain);
        self.check_name(section.line, name, Name::File);
        let lines: Vec<String> = values(section, "line")
            .into_iter()
            .map(|(text, _)| text)
            .collect();
        if lines.is_empty() {
            let id = section.id;
            self.error(section.line, format!("file {id} has no line"));
        }
        let file = File {
            name: name.to_owned(),
            lines,
        };
        self.files.push((host.to_owned(), file, section.line));
    }

    fn allow(&mut self, section: &Section<'_>, keys: &[&str]) {
        for pair in &section.pairs {
            if !keys.contains(&pair.key) {
                let (kind, id, key) = (section.kind, section.id, pair.key);
                self.error(pair.line, format!("unknown key in {kind} {id}: {key}"));
            }
        }
    }

    fn single(&mut self, section: &Section<'_>, key: &str) -> Option<(String, usize)> {
        let found = self.optional(section, key);
        if found.is_none() {
            let (kind, id) = (section.kind, section.id);
            self.error(section.line, format!("{kind} {id} has no {key}"));
        }
        found
    }

    fn optional(&mut self, section: &Section<'_>, key: &str) -> Option<(String, usize)> {
        let mut found = section.pairs.iter().filter(|pair| pair.key == key);
        let first = found.next()?;
        if let Some(repeat) = found.next() {
            let (kind, id) = (section.kind, section.id);
            self.error(
                repeat.line,
                format!("{key} is set more than once in {kind} {id}"),
            );
        }
        Some((first.value.to_owned(), first.line))
    }

    fn check_name(&mut self, line: usize, name: &str, rule: Name) {
        let dots = matches!(rule, Name::File);
        let valid = !name.is_empty()
            && name.chars().all(|c| {
                c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || (dots && c == '.')
            });
        if !valid {
            let allowed = if dots {
                "a-z, 0-9, -, and ."
            } else {
                "a-z, 0-9, and -"
            };
            self.error(line, format!("bad name: {name} (use {allowed})"));
        }
    }

    pub(super) fn has_host(&self, id: &str) -> bool {
        self.hosts.iter().any(|(host, _)| host.id == id)
    }

    pub(super) fn network_of(&self, id: &str) -> Option<&str> {
        self.hosts
            .iter()
            .find(|(host, _)| host.id == id)
            .and_then(|(host, _)| host.network.as_deref())
    }

    pub(super) fn members_of(&self, network: &str) -> usize {
        self.hosts
            .iter()
            .filter(|(host, _)| host.network.as_deref() == Some(network))
            .count()
    }
}

fn values(section: &Section<'_>, key: &str) -> Vec<(String, usize)> {
    section
        .pairs
        .iter()
        .filter(|pair| pair.key == key)
        .map(|pair| (pair.value.to_owned(), pair.line))
        .collect()
}
