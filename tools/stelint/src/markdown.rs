#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    Heading,
    Prose,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Block {
    pub(crate) line: usize,
    pub(crate) kind: Kind,
    pub(crate) text: String,
}

pub(crate) fn blocks(markdown: &str) -> Vec<Block> {
    let mut parser = Parser::default();
    for (index, line) in markdown.lines().enumerate() {
        parser.line(index + 1, line);
    }
    parser.finish()
}

#[derive(Default)]
struct Parser {
    blocks: Vec<Block>,
    open: Option<(usize, String)>,
    in_fence: bool,
}

impl Parser {
    fn line(&mut self, number: usize, raw: &str) {
        let text = raw.trim();
        if text.starts_with("```") || text.starts_with("~~~") {
            self.close();
            self.in_fence = !self.in_fence;
            return;
        }
        if self.in_fence || (is_indented(raw) && self.open.is_none()) {
            return;
        }
        if text.is_empty() {
            self.close();
        } else if let Some(title) = heading(text) {
            self.close();
            self.push(number, Kind::Heading, title);
        } else if text.starts_with('|') {
            self.close();
            if !is_separator(text) {
                for cell in cells(text) {
                    self.push(number, Kind::Prose, &cell);
                }
            }
        } else if let Some(item) = list_item(text) {
            self.close();
            self.open = Some((number, item.to_owned()));
        } else {
            let text = text.strip_prefix('>').map_or(text, str::trim_start);
            match &mut self.open {
                Some((_, open)) => {
                    open.push(' ');
                    open.push_str(text);
                }
                None => self.open = Some((number, text.to_owned())),
            }
        }
    }

    fn push(&mut self, line: usize, kind: Kind, raw: &str) {
        let text = inline(raw);
        if !text.is_empty() {
            self.blocks.push(Block { line, kind, text });
        }
    }

    fn close(&mut self) {
        if let Some((line, text)) = self.open.take() {
            self.push(line, Kind::Prose, &text);
        }
    }

    fn finish(mut self) -> Vec<Block> {
        self.close();
        self.blocks
    }
}

fn is_indented(raw: &str) -> bool {
    raw.starts_with("    ") || raw.starts_with('\t')
}

fn heading(text: &str) -> Option<&str> {
    let hashes = text.chars().take_while(|&c| c == '#').count();
    let rest = &text[hashes..];
    (hashes > 0 && (rest.is_empty() || rest.starts_with(' '))).then(|| rest.trim())
}

fn is_separator(row: &str) -> bool {
    row.chars().all(|c| matches!(c, '|' | '-' | ':' | ' '))
}

fn cells(row: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut cell = String::new();
    let mut chars = row.chars().peekable();
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
        .filter(|cell| !cell.is_empty())
        .collect()
}

fn list_item(text: &str) -> Option<&str> {
    let bullet = ["- ", "* ", "+ "]
        .iter()
        .find_map(|marker| text.strip_prefix(marker));
    if bullet.is_some() {
        return bullet;
    }
    let digits = text.chars().take_while(char::is_ascii_digit).count();
    if digits == 0 {
        return None;
    }
    let rest = &text[digits..];
    rest.strip_prefix(". ").or_else(|| rest.strip_prefix(") "))
}

fn inline(raw: &str) -> String {
    let text = links(&code_spans(raw)).replace('*', "");
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn code_spans(text: &str) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(start) = rest.find('`') {
        let after = &rest[start + 1..];
        let Some(end) = after.find('`') else {
            break;
        };
        out.push_str(&rest[..start]);
        out.push_str("CODE");
        rest = &after[end + 1..];
    }
    out.push_str(rest);
    out
}

fn links(text: &str) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(open) = rest.find('[') {
        let Some(middle) = rest[open..].find("](").map(|at| open + at) else {
            break;
        };
        let Some(close) = rest[middle..].find(')').map(|at| middle + at) else {
            break;
        };
        out.push_str(&rest[..open]);
        out.push_str(&rest[open + 1..middle]);
        rest = &rest[close + 1..];
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::{Block, Kind, blocks};

    fn texts(markdown: &str) -> Vec<String> {
        blocks(markdown)
            .into_iter()
            .map(|block| block.text)
            .collect()
    }

    #[test]
    fn joins_paragraph_lines() {
        assert_eq!(texts("One\ntwo.\n\nThree."), ["One two.", "Three."]);
    }

    #[test]
    fn indented_line_continues_an_open_paragraph() {
        assert_eq!(texts("One\n    two."), ["One two."]);
    }

    #[test]
    fn skips_fenced_and_indented_code() {
        assert_eq!(
            texts("Text.\n\n```\ncode; here\n```\n\n    more; code\n\nEnd."),
            ["Text.", "End."]
        );
    }

    #[test]
    fn code_span_becomes_one_word() {
        assert_eq!(
            texts("Run `cargo test --workspace` now."),
            ["Run CODE now."]
        );
    }

    #[test]
    fn link_keeps_its_text_only() {
        assert_eq!(
            texts("Read [the guide](docs/style.md)."),
            ["Read the guide."]
        );
    }

    #[test]
    fn emphasis_marks_go_away() {
        assert_eq!(texts("**Trace** is *high*."), ["Trace is high."]);
    }

    #[test]
    fn list_items_are_separate_blocks() {
        assert_eq!(
            texts("- One.\n- Two.\n1. Three."),
            ["One.", "Two.", "Three."]
        );
    }

    #[test]
    fn table_cells_are_separate_blocks() {
        assert_eq!(
            texts("| Name | Meaning |\n| --- | --- |\n| pipe | The `\\|` key. |"),
            ["Name", "Meaning", "pipe", "The CODE key."]
        );
    }

    #[test]
    fn quote_marker_goes_away() {
        assert_eq!(
            texts("> CAUTION: Keep it secret."),
            ["CAUTION: Keep it secret."]
        );
    }

    #[test]
    fn heading_has_its_own_kind_and_line() {
        let found = blocks("# Title\n\nText.");
        assert_eq!(
            found[0],
            Block {
                line: 1,
                kind: Kind::Heading,
                text: "Title".to_owned()
            }
        );
        assert_eq!((found[1].line, found[1].kind), (3, Kind::Prose));
    }

    #[test]
    fn hash_without_space_is_not_a_heading() {
        assert_eq!(blocks("#relay is open.")[0].kind, Kind::Prose);
    }
}
