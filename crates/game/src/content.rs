mod checks;
mod reader;
mod shops;
mod targets;
#[cfg(test)]
mod tests;

use std::fmt;

use reader::Reader;

use crate::commands::Verb;
use crate::contract::Event;
use crate::heat::Security;
use crate::ice::IceKind;
use crate::target::Layer;

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
    pub(crate) contract: Option<String>,
    pub(crate) networks: Vec<NetworkDef>,
    pub(crate) hosts: Vec<HostDef>,
    pub(crate) contracts: Vec<ContractDef>,
    pub(crate) shops: Vec<ShopDef>,
    pub(crate) corps: Vec<CorpDef>,
    pub(crate) templates: Vec<TemplateDef>,
    pub(crate) fixers: Vec<FixerDef>,
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
    pub(crate) files: Vec<File>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ShopDef {
    pub(crate) host: String,
    pub(crate) programs: Vec<(Verb, u32)>,
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
}

impl File {
    pub(crate) fn new(name: impl Into<String>, lines: Vec<String>) -> Self {
        Self {
            name: name.into(),
            lines,
            value: 0,
        }
    }
}

impl Content {
    pub fn parse(text: &str) -> Result<Self, Vec<ContentError>> {
        let text = text.strip_prefix('\u{feff}').unwrap_or(text);
        Reader::read(text).finish()
    }
}
