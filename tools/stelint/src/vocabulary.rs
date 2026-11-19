use crate::{markdown, words};

const SUFFIXES: &[&str] = &["", "s", "es", "d", "ed", "ly"];

#[derive(Debug, Default)]
pub struct Rules {
    banned: Vec<Banned>,
}

#[derive(Debug)]
struct Banned {
    words: Vec<String>,
    replacement: String,
}

impl Rules {
    #[must_use]
    pub fn from_style_guide(style_guide: &str) -> Self {
        let mut banned = Vec::new();
        let mut in_table = false;
        for line in style_guide.lines().map(str::trim) {
            if !line.starts_with('|') {
                in_table = false;
                continue;
            }
            let cells = markdown::cells(line);
            if cells.first().is_some_and(|cell| cell == "Do not use") {
                in_table = true;
            } else if in_table && let [left, right, ..] = cells.as_slice() {
                let replacement = markdown::inline(right);
                for phrase in markdown::code_span_contents(left) {
                    banned.extend(variants(phrase).into_iter().map(|words| Banned {
                        words,
                        replacement: replacement.clone(),
                    }));
                }
            }
        }
        Self { banned }
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.banned.is_empty()
    }

    pub(crate) fn unapproved(&self, text: &str) -> Vec<(String, &str)> {
        let tokens = words::lowercase_words(text);
        let mut found = Vec::new();
        for start in 0..tokens.len() {
            let rest = &tokens[start..];
            for banned in &self.banned {
                if rest.starts_with(&banned.words) {
                    found.push((banned.words.join(" "), banned.replacement.as_str()));
                }
            }
        }
        found
    }
}

fn variants(phrase: &str) -> Vec<Vec<String>> {
    let words: Vec<String> = phrase.split_whitespace().map(str::to_lowercase).collect();
    match words.as_slice() {
        [word] => SUFFIXES
            .iter()
            .map(|suffix| vec![format!("{word}{suffix}")])
            .collect(),
        _ => vec![words],
    }
}
