use crate::content::ContentError;

pub(crate) struct Section<'a> {
    pub(crate) kind: &'a str,
    pub(crate) id: &'a str,
    pub(crate) line: usize,
    pub(crate) pairs: Vec<Pair<'a>>,
}

pub(crate) struct Pair<'a> {
    pub(crate) key: &'a str,
    pub(crate) value: &'a str,
    pub(crate) line: usize,
}

pub(crate) fn read(text: &str) -> (Vec<Section<'_>>, Vec<ContentError>) {
    let mut sections: Vec<Section<'_>> = Vec::new();
    let mut errors = Vec::new();
    let mut error = |line, message: String| errors.push(ContentError { line, message });
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
    use super::read;
    use crate::content::ContentError;

    #[test]
    fn sections_keep_their_keys_in_order() {
        let (sections, errors) = read("# note\n[host a]\nroute = b\nroute = c\n\n[file a/x]\n");
        assert_eq!(errors, Vec::<ContentError>::new());
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
}
