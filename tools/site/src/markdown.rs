use std::fmt::Write;

pub(crate) struct Page {
    pub(crate) title: String,
    pub(crate) summary: String,
    pub(crate) sections: Vec<(String, String)>,
    pub(crate) body: String,
}

enum Open {
    Paragraph(Vec<String>),
    Quote(Vec<String>),
    List { ordered: bool, items: Vec<String> },
    Table(Vec<Vec<String>>),
    Code(Vec<String>),
}

struct Renderer<'a> {
    link: &'a dyn Fn(&str) -> String,
    open: Option<Open>,
    page: Page,
    ids: Vec<String>,
}

pub(crate) fn render(markdown: &str, link: &dyn Fn(&str) -> String) -> Page {
    let mut renderer = Renderer {
        link,
        open: None,
        page: Page {
            title: String::new(),
            summary: String::new(),
            sections: Vec::new(),
            body: String::new(),
        },
        ids: Vec::new(),
    };
    for line in markdown.lines() {
        renderer.line(line);
    }
    renderer.close();
    renderer.page
}

impl Renderer<'_> {
    fn line(&mut self, raw: &str) {
        let text = raw.trim();
        if text.is_empty() {
            self.close();
            return;
        }
        let code = raw.strip_prefix("    ").or_else(|| raw.strip_prefix('\t'));
        let prose = matches!(
            self.open,
            Some(Open::Paragraph(_) | Open::Quote(_) | Open::List { .. })
        );
        if let Some(code) = code.filter(|_| !prose) {
            if let Some(Open::Code(lines)) = &mut self.open {
                lines.push(code.to_owned());
            } else {
                self.start(Open::Code(vec![code.to_owned()]));
            }
        } else if let Some((level, title)) = heading(text) {
            self.close();
            self.heading(level, title);
        } else if text.starts_with('|') {
            if let Some(Open::Table(rows)) = &mut self.open {
                rows.push(cells(text));
            } else {
                self.start(Open::Table(vec![cells(text)]));
            }
        } else if let Some((ordered, item)) = list_item(text) {
            match &mut self.open {
                Some(Open::List {
                    ordered: kind,
                    items,
                }) if *kind == ordered => {
                    items.push(item.to_owned());
                }
                _ => self.start(Open::List {
                    ordered,
                    items: vec![item.to_owned()],
                }),
            }
        } else if let Some(quote) = text.strip_prefix('>') {
            if let Some(Open::Quote(lines)) = &mut self.open {
                lines.push(quote.trim_start().to_owned());
            } else {
                self.start(Open::Quote(vec![quote.trim_start().to_owned()]));
            }
        } else {
            match &mut self.open {
                Some(Open::Paragraph(lines) | Open::Quote(lines)) => lines.push(text.to_owned()),
                Some(Open::List { items, .. }) => {
                    if let Some(last) = items.last_mut() {
                        last.push(' ');
                        last.push_str(text);
                    }
                }
                _ => self.start(Open::Paragraph(vec![text.to_owned()])),
            }
        }
    }

    fn start(&mut self, open: Open) {
        self.close();
        self.open = Some(open);
    }

    fn close(&mut self) {
        let Some(open) = self.open.take() else {
            return;
        };
        let html = match open {
            Open::Paragraph(lines) => {
                let text = lines.join(" ");
                if self.page.summary.is_empty() {
                    self.page.summary = plain(&text);
                }
                format!("<p>{}</p>\n", self.inline(&text))
            }
            Open::Quote(lines) => {
                let text = lines.join(" ");
                let class = if text.starts_with("CAUTION:") {
                    " class=\"caution\""
                } else {
                    ""
                };
                format!(
                    "<blockquote{class}><p>{}</p></blockquote>\n",
                    self.inline(&text)
                )
            }
            Open::List { ordered, items } => {
                let tag = if ordered { "ol" } else { "ul" };
                let items = items.iter().fold(String::new(), |mut html, item| {
                    let _ = writeln!(html, "<li>{}</li>", self.inline(item));
                    html
                });
                format!("<{tag}>\n{items}</{tag}>\n")
            }
            Open::Table(rows) => self.table(&rows),
            Open::Code(lines) => format!("<pre><code>{}</code></pre>\n", escape(&lines.join("\n"))),
        };
        self.page.body.push_str(&html);
    }

    fn heading(&mut self, level: usize, title: &str) {
        let id = self.unique(&slug(title));
        if level == 1 && self.page.title.is_empty() {
            self.page.title = plain(title);
        }
        if level == 2 {
            self.page.sections.push((id.clone(), plain(title)));
        }
        let html = self.inline(title);
        let _ = writeln!(self.page.body, "<h{level} id=\"{id}\">{html}</h{level}>");
    }

    fn unique(&mut self, id: &str) -> String {
        let mut found = id.to_owned();
        let mut count = 0;
        while self.ids.contains(&found) {
            count += 1;
            found = format!("{id}-{count}");
        }
        self.ids.push(found.clone());
        found
    }

    fn table(&self, rows: &[Vec<String>]) -> String {
        let (head, body) = match rows {
            [head, rule, body @ ..] if rule.iter().all(|cell| is_rule(cell)) => (Some(head), body),
            _ => (None, rows),
        };
        let row = |cells: &[String], tag: &str| -> String {
            let cells = cells.iter().fold(String::new(), |mut html, cell| {
                let _ = write!(html, "<{tag}>{}</{tag}>", self.inline(cell));
                html
            });
            format!("<tr>{cells}</tr>")
        };
        let mut html = String::from("<div class=\"table\"><table>\n");
        if let Some(head) = head {
            let _ = writeln!(html, "<thead>{}</thead>", row(head, "th"));
        }
        html.push_str("<tbody>\n");
        for cells in body {
            html.push_str(&row(cells, "td"));
            html.push('\n');
        }
        html.push_str("</tbody></table></div>\n");
        html
    }

    fn inline(&self, text: &str) -> String {
        let mut html = String::new();
        let mut rest = text;
        while let Some(start) = rest.find('`') {
            let Some(length) = rest[start + 1..].find('`') else {
                break;
            };
            html.push_str(&self.prose(&rest[..start]));
            let code = &rest[start + 1..start + 1 + length];
            let _ = write!(html, "<code>{}</code>", escape(code));
            rest = &rest[start + 2 + length..];
        }
        html.push_str(&self.prose(rest));
        html
    }

    fn prose(&self, text: &str) -> String {
        let mut html = String::new();
        let mut rest = text;
        while let Some((before, label, url, after)) = split_link(rest) {
            let target = escape(&(self.link)(url));
            html.push_str(&strong(before));
            let _ = write!(html, "<a href=\"{target}\">{}</a>", strong(label));
            rest = after;
        }
        html.push_str(&strong(rest));
        html
    }
}

fn heading(text: &str) -> Option<(usize, &str)> {
    let level = text.chars().take_while(|&c| c == '#').count();
    let rest = text[level..].strip_prefix(' ')?;
    (1..=6).contains(&level).then(|| (level, rest.trim()))
}

fn list_item(text: &str) -> Option<(bool, &str)> {
    if let Some(item) = ["- ", "* ", "+ "]
        .iter()
        .find_map(|marker| text.strip_prefix(marker))
    {
        return Some((false, item));
    }
    let digits = text.chars().take_while(char::is_ascii_digit).count();
    let item = text[digits..].strip_prefix(". ").filter(|_| digits > 0)?;
    Some((true, item))
}

fn cells(row: &str) -> Vec<String> {
    let inner = row.strip_prefix('|').unwrap_or(row);
    let inner = inner.strip_suffix('|').unwrap_or(inner);
    let mut found = Vec::new();
    let mut cell = String::new();
    let mut chars = inner.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' if chars.peek() == Some(&'|') => {
                cell.push('|');
                chars.next();
            }
            '|' => found.push(std::mem::take(&mut cell)),
            _ => cell.push(c),
        }
    }
    found.push(cell);
    found
        .into_iter()
        .map(|cell| cell.trim().to_owned())
        .collect()
}

fn is_rule(cell: &str) -> bool {
    !cell.is_empty() && cell.chars().all(|c| matches!(c, '-' | ':'))
}

fn split_link(text: &str) -> Option<(&str, &str, &str, &str)> {
    let open = text.find('[')?;
    let middle = open + text[open..].find("](")?;
    let close = middle + text[middle..].find(')')?;
    Some((
        &text[..open],
        &text[open + 1..middle],
        &text[middle + 2..close],
        &text[close + 1..],
    ))
}

fn strong(text: &str) -> String {
    let parts: Vec<&str> = text.split("**").collect();
    if parts.len().is_multiple_of(2) {
        return escape(text);
    }
    parts
        .iter()
        .enumerate()
        .map(|(index, part)| {
            if index % 2 == 1 {
                format!("<strong>{}</strong>", escape(part))
            } else {
                escape(part)
            }
        })
        .collect()
}

fn plain(text: &str) -> String {
    let mut found = String::new();
    let mut rest = text;
    while let Some((before, label, _, after)) = split_link(rest) {
        found.push_str(before);
        found.push_str(label);
        rest = after;
    }
    found.push_str(rest);
    found.replace("**", "").replace('`', "")
}

fn slug(title: &str) -> String {
    plain(title)
        .to_lowercase()
        .chars()
        .filter_map(|c| match c {
            ' ' => Some('-'),
            c if c.is_alphanumeric() || c == '-' || c == '_' => Some(c),
            _ => None,
        })
        .collect()
}

pub(crate) fn escape(text: &str) -> String {
    let mut html = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => html.push_str("&amp;"),
            '<' => html.push_str("&lt;"),
            '>' => html.push_str("&gt;"),
            '"' => html.push_str("&quot;"),
            _ => html.push(c),
        }
    }
    html
}

#[cfg(test)]
mod tests {
    use super::render;

    fn same(url: &str) -> String {
        url.to_owned()
    }

    fn body(markdown: &str) -> String {
        render(markdown, &same).body
    }

    #[test]
    fn headings_get_ids_and_the_first_one_is_the_title() {
        let page = render(
            "# Player guide\n\n## The first run\n\n### Step `one`\n",
            &same,
        );
        assert_eq!(page.title, "Player guide");
        assert_eq!(
            page.body,
            "<h1 id=\"player-guide\">Player guide</h1>\n\
             <h2 id=\"the-first-run\">The first run</h2>\n\
             <h3 id=\"step-one\">Step <code>one</code></h3>\n"
        );
        assert_eq!(
            page.sections,
            [("the-first-run".to_owned(), "The first run".to_owned())]
        );
    }

    #[test]
    fn a_repeated_heading_gets_a_number() {
        assert_eq!(
            body("## Money\n\n## Money\n"),
            "<h2 id=\"money\">Money</h2>\n<h2 id=\"money-1\">Money</h2>\n"
        );
    }

    #[test]
    fn lines_join_into_paragraphs_and_the_first_is_the_summary() {
        let page = render("# T\n\nOne\ntwo.\n\nThree.\n", &same);
        assert_eq!(page.summary, "One two.");
        assert!(
            page.body.ends_with("<p>One two.</p>\n<p>Three.</p>\n"),
            "{}",
            page.body
        );
    }

    #[test]
    fn lists_keep_their_kind_and_their_inline_marks() {
        assert_eq!(
            body("- **Clean exit.** You leave.\n- Two\n  more.\n\n1. Go.\n2. Stop.\n"),
            "<ul>\n<li><strong>Clean exit.</strong> You leave.</li>\n<li>Two more.</li>\n</ul>\n\
             <ol>\n<li>Go.</li>\n<li>Stop.</li>\n</ol>\n"
        );
    }

    #[test]
    fn a_table_has_a_head_empty_cells_and_escaped_pipes() {
        assert_eq!(
            body(
                "| Command | Other | Action |\n| --- | --- | --- |\n| `ls` | | The `\\|` key. |\n"
            ),
            "<div class=\"table\"><table>\n\
             <thead><tr><th>Command</th><th>Other</th><th>Action</th></tr></thead>\n\
             <tbody>\n<tr><td><code>ls</code></td><td></td><td>The <code>|</code> key.</td></tr>\n\
             </tbody></table></div>\n"
        );
    }

    #[test]
    fn indented_lines_are_code_and_text_is_escaped() {
        assert_eq!(
            body("Type this:\n\n    go <place> && ls\n    back\n\nA < B & C.\n"),
            "<p>Type this:</p>\n<pre><code>go &lt;place&gt; &amp;&amp; ls\nback</code></pre>\n\
             <p>A &lt; B &amp; C.</p>\n"
        );
    }

    #[test]
    fn an_indented_line_inside_a_paragraph_continues_it() {
        assert_eq!(body("One\n    two.\n"), "<p>One two.</p>\n");
    }

    #[test]
    fn a_caution_quote_gets_its_own_class() {
        assert_eq!(
            body("> CAUTION: Keep `identity.key` secret.\n\n> A note.\n"),
            "<blockquote class=\"caution\"><p>CAUTION: Keep <code>identity.key</code> secret.</p></blockquote>\n\
             <blockquote><p>A note.</p></blockquote>\n"
        );
    }

    #[test]
    fn links_go_through_the_link_function() {
        let upper = |url: &str| url.to_uppercase();
        assert_eq!(
            render(
                "Read the [guide](a.md#b \"x\") and [**this**](c).\n",
                &upper
            )
            .body,
            "<p>Read the <a href=\"A.MD#B &quot;X&quot;\">guide</a> and <a href=\"C\"><strong>this</strong></a>.</p>\n"
        );
    }

    #[test]
    fn loose_marks_stay_as_text() {
        assert_eq!(
            body("A ** star, a `tick, and [a bracket].\n"),
            "<p>A ** star, a `tick, and [a bracket].</p>\n"
        );
    }
}
