mod checks;
mod reader;
#[cfg(test)]
mod tests;

use std::fmt;

use reader::Reader;

use crate::ice::IceKind;

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
    pub(crate) city: String,
    pub(crate) start: String,
    pub(crate) home: Option<String>,
    pub(crate) networks: Vec<NetworkDef>,
    pub(crate) hosts: Vec<HostDef>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NetworkDef {
    pub(crate) id: String,
    pub(crate) name: String,
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
}

impl Content {
    pub fn parse(text: &str) -> Result<Self, Vec<ContentError>> {
        let text = text.strip_prefix('\u{feff}').unwrap_or(text);
        Reader::read(text).finish()
    }
}
