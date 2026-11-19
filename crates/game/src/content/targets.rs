use super::reader::{Name, Reader};
use super::{CorpDef, File, TemplateDef};
use crate::heat::Security;
use crate::ini::{Pair, Section};
use crate::target::Layer;

impl Reader {
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
        self.allow(section, &["layer", "name", "desc"]);
        self.check_name(section.line, id, Name::Plain);
        let layer = self.single(section, "layer").and_then(|(word, line)| {
            let layer = Layer::find(&word);
            if layer.is_none() {
                self.error(
                    line,
                    format!("unknown layer: {word} (use edge, lan, or core)"),
                );
            }
            layer
        });
        let name = self.single(section, "name").unwrap_or_default().0;
        let description = self.single(section, "desc").unwrap_or_default().0;
        if let Some(layer) = layer {
            let template = TemplateDef {
                id: id.to_owned(),
                layer,
                name,
                description,
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
        if self.corps.is_empty() {
            return;
        }
        for layer in Layer::ALL {
            if !self
                .templates
                .iter()
                .any(|(template, _)| template.layer == layer)
            {
                let word = layer.word();
                self.error(0, format!("the corps need a template for the {word} layer"));
            }
        }
    }
}
