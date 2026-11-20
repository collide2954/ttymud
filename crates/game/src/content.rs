mod channels;
mod checks;
mod reader;
mod shops;
mod targets;
#[cfg(test)]
mod tests;

use std::fmt;

use reader::Reader;

use crate::commands::{self, TOP_VERSION, Verb};
use crate::contract::Event;
use crate::heat::Security;
use crate::ice::IceKind;
use crate::target::{Layer, Level};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentError {
    pub line: usize,
    pub message: String,
}

impl fmt::Display for ContentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.line == 0 {
            f.write_str(&self.message)
        } else {
            write!(f, "line {}: {}", self.line, self.message)
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Content {
    pub(crate) id: String,
    pub(crate) city: String,
    pub(crate) start: String,
    pub(crate) home: Option<String>,
    pub(crate) burns: Option<String>,
    pub(crate) news: Option<String>,
    pub(crate) auction: Option<String>,
    pub(crate) contract: Option<String>,
    pub(crate) networks: Vec<NetworkDef>,
    pub(crate) hosts: Vec<HostDef>,
    pub(crate) contracts: Vec<ContractDef>,
    pub(crate) shops: Vec<ShopDef>,
    pub(crate) corps: Vec<CorpDef>,
    pub(crate) templates: Vec<TemplateDef>,
    pub(crate) fixers: Vec<FixerDef>,
    pub(crate) channels: Vec<ChannelDef>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ChannelDef {
    pub(crate) id: String,
    pub(crate) desc: String,
    pub(crate) auto: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FixerDef {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) host: String,
    pub(crate) corps: Vec<String>,
    pub(crate) max_heat: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CorpDef {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) budget: Security,
    pub(crate) data: Vec<File>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TemplateDef {
    pub(crate) id: String,
    pub(crate) layer: Layer,
    pub(crate) name: String,
    pub(crate) description: String,
    pub(crate) software: Option<String>,
    pub(crate) laptop: bool,
    pub(crate) files: Vec<File>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ShopDef {
    pub(crate) host: String,
    pub(crate) programs: Vec<(Verb, u32)>,
    pub(crate) blueprints: Vec<(Verb, u32)>,
    pub(crate) ram: Vec<(u32, u32)>,
    pub(crate) cpu: Vec<(u32, u32)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ContractDef {
    pub(crate) id: String,
    pub(crate) fixer: String,
    pub(crate) network: String,
    pub(crate) goal: String,
    pub(crate) offer: Vec<String>,
    pub(crate) steps: Vec<(Event, String)>,
    pub(crate) pay: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NetworkDef {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) security: Security,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct HostDef {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) description: String,
    pub(crate) network: Option<String>,
    pub(crate) routes: Vec<String>,
    pub(crate) files: Vec<File>,
    pub(crate) ice: Vec<IceDef>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct IceDef {
    pub(crate) name: String,
    pub(crate) kind: IceKind,
    pub(crate) version: u32,
    pub(crate) guards: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct File {
    pub(crate) name: String,
    pub(crate) lines: Vec<String>,
    pub(crate) value: u32,
    pub(crate) lock: Level,
    pub(crate) key: Option<String>,
}

impl File {
    pub(crate) fn new(name: impl Into<String>, lines: Vec<String>) -> Self {
        Self {
            name: name.into(),
            lines,
            value: 0,
            lock: Level::Guest,
            key: None,
        }
    }

    pub(crate) fn blueprint(verb: Verb, version: u32) -> Self {
        let name = commands::name_of(verb);
        Self::new(
            format!("{}{BLUEPRINT}", blueprint_name(verb, version)),
            vec![format!(
                "The source code and build notes for {name} v{version}."
            )],
        )
    }

    pub(crate) fn teaches(&self) -> Option<(Verb, u32)> {
        let (program, version) = self.name.strip_suffix(BLUEPRINT)?.rsplit_once("-v")?;
        let verb = commands::find(program)?.verb;
        let version = version.parse().ok()?;
        let known = verb.power().is_some() && (2..=TOP_VERSION).contains(&version);
        known.then_some((verb, version))
    }
}

const BLUEPRINT: &str = ".blueprint";

pub(crate) fn blueprint_name(verb: Verb, version: u32) -> String {
    format!("{}-v{version}", commands::name_of(verb))
}

impl Content {
    pub fn parse(text: &str) -> Result<Self, Vec<ContentError>> {
        let text = text.strip_prefix('\u{feff}').unwrap_or(text);
        Reader::read(text).finish()
    }
}
