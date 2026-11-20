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
