use super::ShopDef;
use super::reader::{Name, Reader};
use crate::commands::{self, TOP_VERSION, Verb};
use crate::ini::{Pair, Section};
use crate::kits::{CYCLES, RAM};

const SHOP_VERSION: u32 = 2;

impl Reader {
    pub(super) fn shop(&mut self, section: &Section<'_>) {
        let host = section.id;
        if self.shops.iter().any(|(shop, _)| shop.host == host) {
            self.error(
                section.line,
                format!("shop {host} is defined more than once"),
            );
            return;
        }
        self.allow(section, &["program", "blueprint", "ram", "cpu"]);
        self.check_name(section.line, host, Name::Plain);
        let mut shop = ShopDef {
            host: host.to_owned(),
            programs: Vec::new(),
            blueprints: Vec::new(),
            ram: Vec::new(),
            cpu: Vec::new(),
        };
        for pair in &section.pairs {
            match pair.key {
                "program" => shop.programs.extend(self.program_price(pair)),
                "blueprint" => shop.blueprints.extend(self.blueprint_price(pair)),
                "ram" => {
                    let last = shop.ram.last().map_or(RAM, |&(size, _)| size);
                    shop.ram.extend(self.step_price(pair, last, "GB", "a size"));
                }
                "cpu" => {
                    let last = shop.cpu.last().map_or(CYCLES, |&(cycles, _)| cycles);
                    shop.cpu
                        .extend(self.step_price(pair, last, "cycles", "cycles"));
                }
                _ => {}
            }
        }
        self.shops.push((shop, section.line));
    }

    fn priced<'a>(&mut self, pair: &Pair<'a>, item: &str, example: &str) -> Option<(&'a str, u32)> {
        let words: Vec<&str> = pair.value.split_whitespace().collect();
        let [name, price] = words.as_slice() else {
            let key = pair.key;
            let usage = format!("{key} needs {item} and a price, for example {key} = {example}");
            self.error(pair.line, usage);
            return None;
        };
        Some((name, self.price(pair.line, price)?))
    }

    fn price(&mut self, line: usize, word: &str) -> Option<u32> {
        let price = word.parse().ok();
        if price.is_none() {
            let message = format!("bad price: {word} (use a number of credits)");
            self.error(line, message);
        }
        price
    }

    fn program_price(&mut self, pair: &Pair<'_>) -> Option<(Verb, u32)> {
        let (name, price) = self.priced(pair, "a name", "mask 400")?;
        let verb = commands::find(name)
            .map(|command| command.verb)
            .filter(|verb| verb.program().is_some());
        if verb.is_none() {
            self.error(pair.line, format!("unknown program: {name}"));
        }
        Some((verb?, price))
    }

    fn blueprint_price(&mut self, pair: &Pair<'_>) -> Option<(Verb, u32, u32)> {
        let words: Vec<&str> = pair.value.split_whitespace().collect();
        let (name, version, price) = match words.as_slice() {
            [name, price] => (*name, None, *price),
            [name, version, price] => (*name, Some(*version), *price),
            _ => {
                let usage = "blueprint needs a program, an optional version, and a price, for example blueprint = crack 3 2500";
                self.error(pair.line, usage.to_owned());
                return None;
            }
        };
        let verb = commands::find(name)
            .map(|command| command.verb)
            .filter(|verb| verb.power().is_some());
        if verb.is_none() {
            let names: Vec<&str> = commands::versioned().map(commands::name_of).collect();
            let (last, rest) = names.split_last().unwrap_or((&"", &[]));
            let names = rest.join(", ");
            let message = format!("bad blueprint: {name} (use {names}, or {last})");
            self.error(pair.line, message);
        }
        let version = match version.map(str::parse) {
            None => SHOP_VERSION,
            Some(Ok(version)) if (SHOP_VERSION..=TOP_VERSION).contains(&version) => version,
            Some(_) => {
                let word = version.unwrap_or_default();
                let range = format!("use a version from {SHOP_VERSION} to {TOP_VERSION}");
                self.error(
                    pair.line,
                    format!("bad blueprint version: {word} ({range})"),
                );
                return None;
            }
        };
        Some((verb?, version, self.price(pair.line, price)?))
    }

    fn step_price(
        &mut self,
        pair: &Pair<'_>,
        last: u32,
        unit: &str,
        item: &str,
    ) -> Option<(u32, u32)> {
        let example = if pair.key == "ram" {
            "12 900"
        } else {
            "5 1200"
        };
        let (amount, price) = self.priced(pair, item, example)?;
        match amount.parse() {
            Ok(amount) if amount > last => Some((amount, price)),
            _ => {
                let key = pair.key;
                let message = format!("each {key} step must be more than {last} {unit}: {amount}");
                self.error(pair.line, message);
                None
            }
        }
    }
}
