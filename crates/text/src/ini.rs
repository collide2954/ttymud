use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error {
    pub line: usize,
    pub message: String,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.line == 0 {
            f.write_str(&self.message)
        } else {
            write!(f, "line {}: {}", self.line, self.message)
        }
    }
}

pub struct Section<'a> {
    pub kind: &'a str,
    pub id: &'a str,
    pub line: usize,
    pub pairs: Vec<Pair<'a>>,
}

pub struct Pair<'a> {
    pub key: &'a str,
    pub value: &'a str,
    pub line: usize,
}

impl<'a> Section<'a> {
    pub fn single(&self, key: &str) -> Result<&Pair<'a>, Error> {
        let (kind, id) = (self.kind, self.id);
        let mut found = self.pairs.iter().filter(|pair| pair.key == key);
        match (found.next(), found.next()) {
            (Some(pair), None) if !pair.value.is_empty() => Ok(pair),
            (Some(_), Some(pair)) => Err(Error {
                line: pair.line,
                message: format!("{kind} {id} has more than one {key}"),
            }),
            _ => Err(Error {
                line: self.line,
                message: format!("{kind} {id} has no {key}"),
            }),
        }
    }

    #[must_use]
    pub fn unknown(&self, keys: &[&str]) -> Option<Error> {
        let pair = self.pairs.iter().find(|pair| !keys.contains(&pair.key))?;
        let (kind, id, key) = (self.kind, self.id, pair.key);
        Some(Error {
            line: pair.line,
            message: format!("unknown key in {kind} {id}: {key}"),
        })
    }
}

#[must_use]
pub fn read(text: &str) -> (Vec<Section<'_>>, Vec<Error>) {
    let mut sections: Vec<Section<'_>> = Vec::new();
    let mut errors = Vec::new();
    let mut error = |line, message: String| errors.push(Error { line, message });
    for (index, raw) in text.lines().enumerate() {
        let line = index + 1;
        let text = raw.trim();
        if text.is_empty() || text.starts_with('#') {
            continue;
        }
        if let Some(header) = text
            .strip_prefix('[')
            .and_then(|rest| rest.strip_suffix(']'))
        {
            match header.trim().split_once(' ') {
                Some((kind, id)) if !id.trim().is_empty() => sections.push(Section {
                    kind,
                    id: id.trim(),
                    line,
                    pairs: Vec::new(),
                }),
                _ => error(
                    line,
                    "section needs a type and a name, for example [host relay]".to_owned(),
                ),
            }
        } else if let Some((key, value)) = text.split_once('=') {
            match sections.last_mut() {
                Some(section) => section.pairs.push(Pair {
                    key: key.trim(),
                    value: value.trim(),
                    line,
                }),
                None => error(line, "key before the first section".to_owned()),
            }
        } else {
            error(line, format!("cannot read this line: {text}"));
        }
    }
    (sections, errors)
}

#[cfg(test)]
mod tests {
    use super::{Error, read};

    #[test]
    fn sections_keep_their_keys_in_order() {
        let (sections, errors) = read("# note\n[host a]\nroute = b\nroute = c\n\n[file a/x]\n");
        assert_eq!(errors, Vec::<Error>::new());
        let kinds: Vec<(&str, &str)> = sections.iter().map(|s| (s.kind, s.id)).collect();
        assert_eq!(kinds, [("host", "a"), ("file", "a/x")]);
        let routes: Vec<&str> = sections[0].pairs.iter().map(|pair| pair.value).collect();
        assert_eq!(routes, ["b", "c"]);
        assert_eq!(sections[0].pairs[1].line, 4);
    }

    #[test]
    fn bad_lines_give_errors_with_line_numbers() {
        let (_, errors) = read("key = early\n[host]\n[host a]\nnonsense\n");
        let lines: Vec<usize> = errors.iter().map(|error| error.line).collect();
        assert_eq!(lines, [1, 2, 4]);
    }

    #[test]
    fn single_gives_the_one_value_of_a_key() {
        let (sections, _) = read("[server dock]\ncity = Ostra\ndesc = A\ndesc = B\naddress =\n");
        let value = |key| {
            let found = sections[0].single(key);
            found
                .map(|pair| (pair.value, pair.line))
                .map_err(|error| error.to_string())
        };
        assert_eq!(value("city"), Ok(("Ostra", 2)));
        assert_eq!(
            value("desc"),
            Err("line 4: server dock has more than one desc".to_owned())
        );
        assert_eq!(
            value("address"),
            Err("line 1: server dock has no address".to_owned())
        );
        assert_eq!(
            value("key"),
            Err("line 1: server dock has no key".to_owned())
        );
    }

    #[test]
    fn unknown_finds_the_first_key_that_is_not_allowed() {
        let (sections, _) = read("[link dock]\naddress = a\nport = 1\nzone = 2\n");
        let found = |keys: &[&str]| sections[0].unknown(keys).map(|error| error.to_string());
        assert_eq!(
            found(&["address"]),
            Some("line 3: unknown key in link dock: port".to_owned())
        );
        assert_eq!(found(&["address", "port", "zone"]), None);
    }

    #[test]
    fn an_error_shows_its_line_unless_it_has_none() {
        let at = |line| {
            Error {
                line,
                message: "no city section".to_owned(),
            }
            .to_string()
        };
        assert_eq!(at(3), "line 3: no city section");
        assert_eq!(at(0), "no city section");
    }
}
