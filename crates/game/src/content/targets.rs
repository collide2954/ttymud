use super::reader::{Name, Reader};
use super::{CorpDef, File, FixerDef, TemplateDef, is_script};
use crate::heat::{MAX_HEAT, Security};
use crate::ini::{Pair, Section};
use crate::target::Layer;

pub(super) struct FixerLines {
    section: usize,
    host: usize,
    corps: Vec<usize>,
}

impl Reader {
    pub(super) fn fixer(&mut self, section: &Section<'_>) {
        let id = section.id;
        if self.fixers.iter().any(|(fixer, _)| fixer.id == id) {
            self.error(
                section.line,
                format!("fixer {id} is defined more than once"),
            );
            return;
        }
        self.allow(section, &["name", "host", "corp", "max-heat"]);
        self.check_name(section.line, id, Name::Plain);
        let name = self.single(section, "name").unwrap_or_default().0;
        let (host, host_line) = self.single(section, "host").unwrap_or_default();
        let corps: Vec<&Pair<'_>> = section
            .pairs
            .iter()
            .filter(|pair| pair.key == "corp")
            .collect();
        let max_heat = self.optional(section, "max-heat").and_then(|(word, line)| {
            let heat = word.parse().ok().filter(|&heat| heat < MAX_HEAT);
            if heat.is_none() {
                let most = MAX_HEAT - 1;
                self.error(
                    line,
                    format!("bad max-heat: {word} (use a number from 0 to {most})"),
                );
            }
            heat
        });
        let lines = FixerLines {
            section: section.line,
            host: host_line,
            corps: corps.iter().map(|pair| pair.line).collect(),
        };
        let fixer = FixerDef {
            id: id.to_owned(),
            name,
            host,
            corps: corps.iter().map(|pair| pair.value.to_owned()).collect(),
            max_heat,
        };
        self.fixers.push((fixer, lines));
    }

    pub(super) fn check_fixers(&mut self) {
        let mut problems = Vec::new();
        for (fixer, lines) in &self.fixers {
            let (id, host) = (&fixer.id, &fixer.host);
            if !host.is_empty() && !self.has_host(host) {
                problems.push((
                    lines.host,
                    format!("fixer {id} is on an unknown host: {host}"),
                ));
            } else if self.network_of(host).is_some() {
                problems.push((
                    lines.host,
                    format!("fixer {id} is on a host in a network: {host}"),
                ));
            }
            let mut known = Vec::new();
            for (corp, &line) in fixer.corps.iter().zip(&lines.corps) {
                match self.corps.iter().find(|(def, _)| &def.id == corp) {
                    Some((def, _)) => known.push(def.budget),
                    None => problems.push((
                        line,
                        format!("fixer {id} works against an unknown corp: {corp}"),
                    )),
                }
            }
            if !known.is_empty() && !known.contains(&Security::Low) {
                let message = format!("fixer {id} needs a corp with a low budget for new handles");
                problems.push((lines.section, message));
            }
        }
        for (line, message) in problems {
            self.error(line, message);
        }
    }

    pub(super) fn corp(&mut self, section: &Section<'_>) {
        let id = section.id;
        if self.corps.iter().any(|(corp, _)| corp.id == id) {
            self.error(section.line, format!("corp {id} is defined more than once"));
            return;
        }
        self.allow(section, &["name", "budget", "data"]);
        self.check_name(section.line, id, Name::Plain);
        let name = self.single(section, "name").unwrap_or_default().0;
        let budget = self
            .single(section, "budget")
            .map_or(Security::Low, |(word, line)| {
                Security::find(&word).unwrap_or_else(|| {
                    let levels = "low, medium, high, or top";
                    self.error(line, format!("unknown budget: {word} (use {levels})"));
                    Security::Low
                })
            });
        let pairs: Vec<&Pair<'_>> = section
            .pairs
            .iter()
            .filter(|pair| pair.key == "data")
            .collect();
        if pairs.is_empty() {
            self.error(section.line, format!("corp {id} has no data"));
        }
        let data = pairs
            .into_iter()
            .filter_map(|pair| self.data(pair))
            .collect();
        let corp = CorpDef {
            id: id.to_owned(),
            name,
            budget,
            data,
        };
        self.corps.push((corp, section.line));
    }

    fn data(&mut self, pair: &Pair<'_>) -> Option<File> {
        let parts = pair.value.split_once(':');
        let Some((name, line)) =
            parts.filter(|(name, line)| !name.trim().is_empty() && !line.trim().is_empty())
        else {
            let example = "data = claims.db: Ten years of claims.";
            self.error(
                pair.line,
                format!("data needs a file name and a line, for example {example}"),
            );
            return None;
        };
        let name = name.trim();
        self.check_name(pair.line, name, Name::File);
        if is_script(name) {
            self.error(pair.line, format!("data cannot be a script: {name}"));
        }
        Some(File::new(name, vec![line.trim().to_owned()]))
    }

    pub(super) fn template(&mut self, section: &Section<'_>) {
        let id = section.id;
        if self.templates.iter().any(|(template, _)| template.id == id) {
            self.error(
                section.line,
                format!("template {id} is defined more than once"),
            );
            return;
        }
        self.allow(section, &["layer", "name", "desc", "software", "laptop"]);
        self.check_name(section.line, id, Name::Plain);
        let layer = self.single(section, "layer").and_then(|(word, line)| {
            let layer = Layer::find(&word);
            if layer.is_none() {
                self.error(
                    line,
                    format!("unknown layer: {word} (use edge, lan, core, or vault)"),
                );
            }
            layer
        });
        let name = self.single(section, "name").unwrap_or_default().0;
        let description = self.single(section, "desc").unwrap_or_default().0;
        let software = self.optional(section, "software").map(|(name, _)| name);
        let laptop = match self.optional(section, "laptop") {
            None => false,
            Some((word, line)) => {
                let yes = match word.as_str() {
                    "yes" => true,
                    "no" => false,
                    _ => {
                        self.error(line, format!("bad laptop: {word} (use yes or no)"));
                        false
                    }
                };
                if yes && layer != Some(Layer::Lan) {
                    self.error(line, "only a lan template can be a laptop".to_owned());
                }
                yes
            }
        };
        if let Some(layer) = layer {
            let template = TemplateDef {
                id: id.to_owned(),
                layer,
                name,
                description,
                software,
                laptop,
                files: Vec::new(),
            };
            self.templates.push((template, section.line));
        }
    }

    pub(super) fn check_templates(&mut self) {
        let clashes: Vec<(String, usize)> = self
            .templates
            .iter()
            .filter(|(template, _)| self.has_host(&template.id))
            .map(|(template, line)| (template.id.clone(), *line))
            .collect();
        for (id, line) in clashes {
            self.error(line, format!("template {id} has the same name as a host"));
        }
        let vault = self
            .templates
            .iter()
            .find(|(template, _)| template.layer == Layer::Vault)
            .map(|&(_, line)| line);
        let laptop = self.templates.iter().any(|(template, _)| template.laptop);
        if let (Some(line), false) = (vault, laptop) {
            self.error(line, "a vault template needs a laptop template".to_owned());
        }
        if self.corps.is_empty() {
            return;
        }
        for layer in Layer::BASE {
            if !self
                .templates
                .iter()
                .any(|(template, _)| template.layer == layer && !template.laptop)
            {
                let word = layer.word();
                self.error(0, format!("the corps need a template for the {word} layer"));
            }
        }
    }
}
