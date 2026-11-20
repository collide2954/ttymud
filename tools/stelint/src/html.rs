use crate::markdown::{Block, Kind};

const BLOCKS: &[&str] = &[
    "html",
    "head",
    "body",
    "title",
    "header",
    "footer",
    "main",
    "nav",
    "section",
    "article",
    "aside",
    "div",
    "p",
    "ul",
    "ol",
    "li",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "figure",
    "figcaption",
    "table",
    "thead",
    "tbody",
    "tr",
    "td",
    "th",
    "blockquote",
    "details",
    "summary",
    "br",
    "hr",
    "dl",
    "dt",
    "dd",
];
const SKIPPED: &[&str] = &["pre", "script", "style"];

#[derive(Default)]
struct Reader {
    blocks: Vec<Block>,
    text: String,
    start: usize,
    heading: bool,
    code: bool,
    skipped: usize,
}

pub(crate) fn blocks(html: &str) -> Vec<Block> {
    let mut reader = Reader::default();
    let mut line = 1;
    let mut rest = html;
    while let Some(open) = rest.find('<') {
        reader.text(&rest[..open], line);
        line += rest[..open].matches('\n').count();
        rest = &rest[open..];
        let end = if rest.starts_with("<!--") {
            rest.find("-->").map(|at| at + 3)
        } else {
            rest.find('>').map(|at| at + 1)
        };
        let Some(end) = end else {
            break;
        };
        reader.tag(&rest[..end], line);
        line += rest[..end].matches('\n').count();
        rest = &rest[end..];
    }
    reader.text(rest, line);
    reader.flush();
    reader.blocks
}

impl Reader {
    fn text(&mut self, raw: &str, line: usize) {
        if raw.is_empty() || self.skipped > 0 || self.code {
            return;
        }
        if raw.trim().is_empty() {
            self.text.push(' ');
            return;
        }
        if self.text.trim().is_empty() {
            let leading = raw.len() - raw.trim_start().len();
            self.start = line + raw[..leading].matches('\n').count();
        }
        self.text.push_str(&decode(raw));
    }

    fn tag(&mut self, tag: &str, line: usize) {
        if tag.starts_with("<!") {
            return;
        }
        let inner = tag.trim_start_matches('<');
        let closing = inner.starts_with('/');
        let name: String = inner
            .trim_start_matches('/')
            .chars()
            .take_while(char::is_ascii_alphanumeric)
            .collect::<String>()
            .to_ascii_lowercase();
        if SKIPPED.contains(&name.as_str()) {
            if closing {
                self.skipped = self.skipped.saturating_sub(1);
            } else {
                self.flush();
                self.skipped += 1;
            }
            return;
        }
        if self.skipped > 0 {
            return;
        }
        if name == "code" {
            if !closing {
                if self.text.trim().is_empty() {
                    self.start = line;
                }
                self.text.push_str(" CODE ");
            }
            self.code = !closing;
        } else if BLOCKS.contains(&name.as_str()) {
            self.flush();
            self.heading = !closing && name.len() == 2 && name.starts_with('h');
        }
    }

    fn flush(&mut self) {
        let text = self.text.split_whitespace().collect::<Vec<_>>().join(" ");
        self.text.clear();
        if text.is_empty() {
            return;
        }
        let kind = if self.heading {
            Kind::Heading
        } else {
            Kind::Prose
        };
        self.blocks.push(Block {
            line: self.start,
            kind,
            text,
        });
    }
}

fn decode(raw: &str) -> String {
    let mut found = String::with_capacity(raw.len());
    let mut rest = raw;
    while let Some(amp) = rest.find('&') {
        found.push_str(&rest[..amp]);
        let after = &rest[amp..];
        let entity = after
            .find(';')
            .filter(|&semi| semi <= 10)
            .and_then(|semi| Some((entity(&after[1..semi])?, semi)));
        if let Some((c, semi)) = entity {
            found.push(c);
            rest = &after[semi + 1..];
        } else {
            found.push('&');
            rest = &after[1..];
        }
    }
    found.push_str(rest);
    found
}

fn entity(name: &str) -> Option<char> {
    match name {
        "amp" => Some('&'),
        "lt" => Some('<'),
        "gt" => Some('>'),
        "quot" => Some('"'),
        "apos" => Some('\''),
        "nbsp" => Some(' '),
        "middot" => Some('·'),
        "mdash" => Some('—'),
        "ndash" => Some('–'),
        _ => {
            let number = name.strip_prefix('#')?;
            let code = match number.strip_prefix(['x', 'X']) {
                Some(hex) => u32::from_str_radix(hex, 16).ok()?,
                None => number.parse().ok()?,
            };
            char::from_u32(code)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::blocks;
    use crate::markdown::{Block, Kind};

    fn texts(html: &str) -> Vec<String> {
        blocks(html).into_iter().map(|block| block.text).collect()
    }

    #[test]
    fn each_block_element_is_a_block_and_inline_tags_join_their_text() {
        assert_eq!(
            texts(
                "<p>Read the <a href=\"x\"><strong>guide</strong></a>.</p>\n<ul><li>One.</li><li>Two.</li></ul>"
            ),
            ["Read the guide.", "One.", "Two."]
        );
    }

    #[test]
    fn headings_have_their_own_kind_and_line() {
        let found = blocks("<section>\n<h2>The game</h2>\n<p>Text.</p>\n</section>");
        assert_eq!(
            found[0],
            Block {
                line: 2,
                kind: Kind::Heading,
                text: "The game".to_owned()
            }
        );
        assert_eq!((found[1].line, found[1].kind), (3, Kind::Prose));
    }

    #[test]
    fn code_blocks_scripts_and_styles_are_skipped() {
        assert_eq!(
            texts(
                "<p>Before.</p><pre><code>a; b\nc; d</code></pre><style>p { color: red; }</style><script>x();</script><p>After.</p>"
            ),
            ["Before.", "After."]
        );
    }

    #[test]
    fn inline_code_becomes_one_word() {
        assert_eq!(
            texts("<p>Type <code>help; now</code> to see the commands.</p>"),
            ["Type CODE to see the commands."]
        );
    }

    #[test]
    fn entities_are_decoded_and_comments_and_the_doctype_are_skipped() {
        assert_eq!(
            texts(
                "<!doctype html><!-- a; b --><p>A &amp; B &lt;go&gt; &quot;x&quot; &#39;y&#39; &middot; z</p>"
            ),
            ["A & B <go> \"x\" 'y' · z"]
        );
    }

    #[test]
    fn text_between_blocks_is_a_block_too() {
        assert_eq!(
            texts(
                "<div><b>nyx in Port Ostra</b></div><a href=\"#\">Go to the content</a><main></main>"
            ),
            ["nyx in Port Ostra", "Go to the content"]
        );
    }
}
