use std::fmt;

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
    pub(crate) hosts: Vec<HostDef>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct HostDef {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) description: String,
    pub(crate) routes: Vec<String>,
    pub(crate) files: Vec<File>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct File {
    pub(crate) name: String,
    pub(crate) lines: Vec<String>,
}

impl Content {
    pub fn parse(text: &str) -> Result<Self, Vec<ContentError>> {
        let text = text.strip_prefix('\u{feff}').unwrap_or(text);
        let mut reader = Reader::default();
        for section in reader.sections(text) {
            reader.section(&section);
        }
        reader.finish()
    }
}

struct Section<'a> {
    kind: &'a str,
    id: &'a str,
    line: usize,
    pairs: Vec<Pair<'a>>,
}

struct Pair<'a> {
    key: &'a str,
    value: &'a str,
    line: usize,
}

#[derive(Clone, Copy)]
enum Name {
    Plain,
    File,
}

#[derive(Default)]
struct Reader {
    errors: Vec<ContentError>,
    city: Option<usize>,
    city_name: Option<String>,
    start: Option<(String, usize)>,
    hosts: Vec<(HostDef, usize)>,
    routes: Vec<(String, String, usize)>,
    files: Vec<(String, File, usize)>,
}

impl Reader {
    fn error(&mut self, line: usize, message: impl Into<String>) {
        self.errors.push(ContentError {
            line,
            message: message.into(),
        });
    }

    fn sections<'a>(&mut self, text: &'a str) -> Vec<Section<'a>> {
        let mut sections: Vec<Section<'a>> = Vec::new();
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
                    _ => self.error(
                        line,
                        "section needs a type and a name, for example [host relay]",
                    ),
                }
            } else if let Some((key, value)) = text.split_once('=') {
                match sections.last_mut() {
                    Some(section) => section.pairs.push(Pair {
                        key: key.trim(),
                        value: value.trim(),
                        line,
                    }),
                    None => self.error(line, "key before the first section"),
                }
            } else {
                self.error(line, format!("cannot read this line: {text}"));
            }
        }
        sections
    }

    fn section(&mut self, section: &Section<'_>) {
        match section.kind {
            "city" => self.city(section),
            "host" => self.host(section),
            "file" => self.file(section),
            other => self.error(section.line, format!("unknown section type: {other}")),
        }
    }

    fn city(&mut self, section: &Section<'_>) {
        if self.city.is_some() {
            self.error(section.line, "more than one city section");
            return;
        }
        self.city = Some(section.line);
        self.allow(section, &["name", "start"]);
        self.check_name(section.line, section.id, Name::Plain);
        self.city_name = self.single(section, "name").map(|(name, _)| name);
        self.start = self.single(section, "start");
    }

    fn host(&mut self, section: &Section<'_>) {
        if self.hosts.iter().any(|(host, _)| host.id == section.id) {
            let id = section.id;
            self.error(section.line, format!("host {id} is defined more than once"));
            return;
        }
        self.allow(section, &["name", "desc", "route"]);
        self.check_name(section.line, section.id, Name::Plain);
        let name = self.single(section, "name");
        let description = self.single(section, "desc");
        let routes = values(section, "route");
        for (target, line) in &routes {
            self.routes
                .push((section.id.to_owned(), target.clone(), *line));
        }
        let host = HostDef {
            id: section.id.to_owned(),
            name: name.unwrap_or_default().0,
            description: description.unwrap_or_default().0,
            routes: routes.into_iter().map(|(target, _)| target).collect(),
            files: Vec::new(),
        };
        self.hosts.push((host, section.line));
    }

    fn file(&mut self, section: &Section<'_>) {
        self.allow(section, &["line"]);
        let Some((host, name)) = section.id.split_once('/') else {
            self.error(
                section.line,
                "file needs a host and a name, for example [file relay/motd.txt]",
            );
            return;
        };
        self.check_name(section.line, host, Name::Plain);
        self.check_name(section.line, name, Name::File);
        let lines: Vec<String> = values(section, "line")
            .into_iter()
            .map(|(text, _)| text)
            .collect();
        if lines.is_empty() {
            let id = section.id;
            self.error(section.line, format!("file {id} has no line"));
        }
        let file = File {
            name: name.to_owned(),
            lines,
        };
        self.files.push((host.to_owned(), file, section.line));
    }

    fn allow(&mut self, section: &Section<'_>, keys: &[&str]) {
        for pair in &section.pairs {
            if !keys.contains(&pair.key) {
                let (kind, id, key) = (section.kind, section.id, pair.key);
                self.error(pair.line, format!("unknown key in {kind} {id}: {key}"));
            }
        }
    }

    fn single(&mut self, section: &Section<'_>, key: &str) -> Option<(String, usize)> {
        let (kind, id) = (section.kind, section.id);
        let mut found = section.pairs.iter().filter(|pair| pair.key == key);
        let Some(first) = found.next() else {
            self.error(section.line, format!("{kind} {id} has no {key}"));
            return None;
        };
        if let Some(repeat) = found.next() {
            self.error(
                repeat.line,
                format!("{key} is set more than once in {kind} {id}"),
            );
        }
        Some((first.value.to_owned(), first.line))
    }

    fn check_name(&mut self, line: usize, name: &str, rule: Name) {
        let dots = matches!(rule, Name::File);
        let valid = !name.is_empty()
            && name.chars().all(|c| {
                c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || (dots && c == '.')
            });
        if !valid {
            let allowed = if dots {
                "a-z, 0-9, -, and ."
            } else {
                "a-z, 0-9, and -"
            };
            self.error(line, format!("bad name: {name} (use {allowed})"));
        }
    }

    fn has_host(&self, id: &str) -> bool {
        self.hosts.iter().any(|(host, _)| host.id == id)
    }

    fn finish(mut self) -> Result<Content, Vec<ContentError>> {
        for (host, file, line) in std::mem::take(&mut self.files) {
            let position = self.hosts.iter().position(|(def, _)| def.id == host);
            match position {
                None => self.error(line, format!("file is on an unknown host: {host}")),
                Some(index)
                    if self.hosts[index]
                        .0
                        .files
                        .iter()
                        .any(|f| f.name == file.name) =>
                {
                    let name = &file.name;
                    self.error(
                        line,
                        format!("file {host}/{name} is defined more than once"),
                    );
                }
                Some(index) => self.hosts[index].0.files.push(file),
            }
        }
        for (from, to, line) in std::mem::take(&mut self.routes) {
            if from == to {
                self.error(line, format!("host {from} has a route to itself"));
            } else if !self.has_host(&to) {
                self.error(line, format!("route to an unknown host: {to}"));
            }
        }
        if self.city.is_none() {
            self.error(0, "no city section");
        }
        if let Some((start, line)) = self.start.clone() {
            if self.has_host(&start) {
                for (id, line) in unreachable(&self.hosts, &start) {
                    self.error(
                        line,
                        format!("host {id} cannot be reached from the start host"),
                    );
                }
            } else {
                self.error(line, format!("start is an unknown host: {start}"));
            }
        }
        self.errors.sort_by_key(|error| error.line);
        if !self.errors.is_empty() {
            return Err(self.errors);
        }
        Ok(Content {
            city: self.city_name.unwrap_or_default(),
            start: self.start.unwrap_or_default().0,
            hosts: self.hosts.into_iter().map(|(host, _)| host).collect(),
        })
    }
}

fn values(section: &Section<'_>, key: &str) -> Vec<(String, usize)> {
    section
        .pairs
        .iter()
        .filter(|pair| pair.key == key)
        .map(|pair| (pair.value.to_owned(), pair.line))
        .collect()
}

fn unreachable(hosts: &[(HostDef, usize)], start: &str) -> Vec<(String, usize)> {
    let mut seen = vec![false; hosts.len()];
    let mut queue: Vec<usize> = hosts
        .iter()
        .position(|(host, _)| host.id == start)
        .into_iter()
        .collect();
    for &index in &queue {
        seen[index] = true;
    }
    while let Some(current) = queue.pop() {
        let here = &hosts[current].0;
        for (index, (host, _)) in hosts.iter().enumerate() {
            let linked = here.routes.contains(&host.id) || host.routes.contains(&here.id);
            if linked && !seen[index] {
                seen[index] = true;
                queue.push(index);
            }
        }
    }
    hosts
        .iter()
        .zip(seen)
        .filter(|(_, reached)| !reached)
        .map(|((host, line), _)| (host.id.clone(), *line))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{Content, File};

    const CITY: &str = "\
# A small city.
[city test]
name = Test City
start = a

[host a]
name = Host A
desc = The first host.
route = b

[host b]
name = Host B
desc = The second host.

[file a/notes.txt]
line = One.
line = Two.
";

    fn errors(text: &str) -> Vec<String> {
        match Content::parse(text) {
            Ok(_) => Vec::new(),
            Err(errors) => errors.iter().map(ToString::to_string).collect(),
        }
    }

    #[test]
    fn reads_the_city_and_its_hosts() {
        let content = Content::parse(CITY).unwrap();
        assert_eq!(content.city, "Test City");
        assert_eq!(content.start, "a");
        let ids: Vec<&str> = content.hosts.iter().map(|host| host.id.as_str()).collect();
        assert_eq!(ids, ["a", "b"]);
        assert_eq!(content.hosts[0].name, "Host A");
        assert_eq!(content.hosts[0].description, "The first host.");
        assert_eq!(content.hosts[0].routes, ["b"]);
    }

    #[test]
    fn a_byte_order_mark_is_ignored() {
        assert_eq!(
            Content::parse(&format!("\u{feff}{CITY}")),
            Content::parse(CITY)
        );
    }

    #[test]
    fn files_belong_to_their_host() {
        let content = Content::parse(CITY).unwrap();
        let notes = File {
            name: "notes.txt".to_owned(),
            lines: vec!["One.".to_owned(), "Two.".to_owned()],
        };
        assert_eq!(content.hosts[0].files, [notes]);
        assert_eq!(content.hosts[1].files, Vec::<File>::new());
    }

    #[test]
    fn unreadable_lines_and_stray_keys_fail_with_line_numbers() {
        assert_eq!(
            errors(&format!("name = x\n{CITY}oops\n")),
            [
                "line 1: key before the first section",
                "line 19: cannot read this line: oops",
            ]
        );
    }

    #[test]
    fn section_headers_need_a_type_and_a_name() {
        let found = errors("[city]\n");
        assert!(found.contains(
            &"line 1: section needs a type and a name, for example [host relay]".to_owned()
        ));
    }

    #[test]
    fn unknown_sections_and_keys_fail() {
        let text = CITY.replace("desc = The first", "desk = The first") + "[zone z]\n";
        assert_eq!(
            errors(&text),
            [
                "line 6: host a has no desc",
                "line 8: unknown key in host a: desk",
                "line 18: unknown section type: zone",
            ]
        );
    }

    #[test]
    fn missing_and_repeated_keys_fail() {
        let text = "[city t]\nname = T\nname = U\n[host a]\nname = A\n";
        assert_eq!(
            errors(text),
            [
                "line 1: city t has no start",
                "line 3: name is set more than once in city t",
                "line 4: host a has no desc",
            ]
        );
    }

    #[test]
    fn bad_references_fail() {
        let text = "[city t]\nname = T\nstart = nowhere\n[host a]\nname = A\ndesc = A.\n\
                    route = ghost\nroute = a\n[file b/x.txt]\nline = X.\n[file x.txt]\nline = X.\n";
        assert_eq!(
            errors(text),
            [
                "line 3: start is an unknown host: nowhere",
                "line 7: route to an unknown host: ghost",
                "line 8: host a has a route to itself",
                "line 9: file is on an unknown host: b",
                "line 11: file needs a host and a name, for example [file relay/motd.txt]",
            ]
        );
    }

    #[test]
    fn duplicates_fail() {
        let text = "[city t]\nname = T\nstart = a\n[city u]\nname = U\nstart = a\n\
                    [host a]\nname = A\ndesc = A.\n[host a]\nname = A\ndesc = A.\n\
                    [file a/x.txt]\nline = X.\n[file a/x.txt]\nline = Y.\n";
        assert_eq!(
            errors(text),
            [
                "line 4: more than one city section",
                "line 10: host a is defined more than once",
                "line 15: file a/x.txt is defined more than once",
            ]
        );
    }

    #[test]
    fn files_need_at_least_one_line() {
        let text = "[city t]\nname = T\nstart = a\n[host a]\nname = A\ndesc = A.\n[file a/x.txt]\n";
        assert_eq!(errors(text), ["line 7: file a/x.txt has no line"]);
    }

    #[test]
    fn a_city_section_is_necessary() {
        assert_eq!(
            errors("[host a]\nname = A\ndesc = A.\n"),
            ["no city section"]
        );
    }

    #[test]
    fn unreachable_hosts_fail() {
        let text = "[city t]\nname = T\nstart = a\n[host a]\nname = A\ndesc = A.\n\
                    [host b]\nname = B\ndesc = B.\n";
        assert_eq!(
            errors(text),
            ["line 7: host b cannot be reached from the start host"]
        );
    }

    #[test]
    fn a_route_connects_both_hosts() {
        let text = "[city t]\nname = T\nstart = a\n[host a]\nname = A\ndesc = A.\n\
                    [host b]\nname = B\ndesc = B.\nroute = a\n";
        assert_eq!(errors(text), Vec::<String>::new());
    }

    #[test]
    fn bad_names_fail() {
        let text = "[city t]\nname = T\nstart = a\n[host a]\nname = A\ndesc = A.\n\
                    [host B!]\nname = B\ndesc = B.\nroute = a\n[file a/No.txt]\nline = X.\n";
        assert_eq!(
            errors(text),
            [
                "line 7: bad name: B! (use a-z, 0-9, and -)",
                "line 11: bad name: No.txt (use a-z, 0-9, -, and .)",
            ]
        );
    }
}
