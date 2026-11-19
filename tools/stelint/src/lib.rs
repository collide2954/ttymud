mod markdown;
mod sentence;
mod words;

use markdown::{Block, Kind};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Issue {
    pub line: usize,
    pub message: String,
}

const PARAGRAPH_LIMIT: usize = 6;

#[must_use]
pub fn check(markdown: &str) -> Vec<Issue> {
    let mut issues = Vec::new();
    for block in markdown::blocks(markdown) {
        semicolons(&block, &mut issues);
        contractions(&block, &mut issues);
        if block.kind == Kind::Prose {
            let sentences = sentence::sentences(&block.text);
            long_sentences(&block, &sentences, &mut issues);
            long_paragraph(&block, &sentences, &mut issues);
        }
    }
    issues
}

fn semicolons(block: &Block, issues: &mut Vec<Issue>) {
    if block.text.contains(';') {
        issues.push(issue(block, "semicolon: write two sentences".to_owned()));
    }
}

fn contractions(block: &Block, issues: &mut Vec<Issue>) {
    for word in words::contractions(&block.text) {
        issues.push(issue(
            block,
            format!("contraction \"{word}\": write the full words"),
        ));
    }
}

fn long_sentences(block: &Block, sentences: &[&str], issues: &mut Vec<Issue>) {
    for text in sentences {
        let words = sentence::word_count(text);
        let limit = sentence::word_limit(text);
        if words > limit {
            let opening = sentence::opening(text);
            issues.push(issue(
                block,
                format!("sentence has {words} words (max {limit}): {opening}"),
            ));
        }
    }
}

fn long_paragraph(block: &Block, sentences: &[&str], issues: &mut Vec<Issue>) {
    if sentences.len() > PARAGRAPH_LIMIT {
        let count = sentences.len();
        issues.push(issue(
            block,
            format!("paragraph has {count} sentences (max {PARAGRAPH_LIMIT})"),
        ));
    }
}

fn issue(block: &Block, message: String) -> Issue {
    Issue {
        line: block.line,
        message,
    }
}

#[cfg(test)]
mod tests {
    use super::check;

    fn messages(text: &str) -> Vec<String> {
        check(text).into_iter().map(|issue| issue.message).collect()
    }

    fn assert_clean(text: &str) {
        assert_eq!(messages(text), Vec::<String>::new());
    }

    fn words(count: usize) -> String {
        vec!["word"; count].join(" ")
    }

    #[test]
    fn description_of_25_words_passes() {
        assert_clean(&format!("{}.", words(25)));
    }

    #[test]
    fn description_of_26_words_fails() {
        let found = messages(&format!("{}.", words(26)));
        assert_eq!(found.len(), 1);
        assert!(found[0].starts_with("sentence has 26 words (max 25)"));
    }

    #[test]
    fn instruction_of_20_words_passes() {
        assert_clean(&format!("Use {}.", words(19)));
    }

    #[test]
    fn instruction_of_21_words_fails() {
        let found = messages(&format!("Use {}.", words(20)));
        assert_eq!(found.len(), 1);
        assert!(found[0].starts_with("sentence has 21 words (max 20)"));
    }

    #[test]
    fn headings_have_no_sentence_limit() {
        assert_clean(&format!("# {}", words(30)));
    }

    #[test]
    fn code_has_no_sentence_limit() {
        assert_clean(&format!(
            "```\n{}.\n```\n\n    {}.\n\nRun `{}` now.",
            words(30),
            words(30),
            words(30)
        ));
    }

    #[test]
    fn paragraph_of_6_sentences_passes() {
        assert_clean(&"Short. ".repeat(6));
    }

    #[test]
    fn paragraph_of_7_sentences_fails() {
        assert_eq!(
            messages(&"Short. ".repeat(7)),
            ["paragraph has 7 sentences (max 6)"]
        );
    }

    #[test]
    fn semicolon_fails() {
        assert_eq!(
            messages("One part; two parts."),
            ["semicolon: write two sentences"]
        );
    }

    #[test]
    fn semicolon_in_a_heading_fails() {
        assert_eq!(messages("# One; two"), ["semicolon: write two sentences"]);
    }

    #[test]
    fn semicolon_in_code_passes() {
        assert_clean("Type `a; b` now.");
    }

    #[test]
    fn contractions_fail() {
        assert_eq!(
            messages("Don't stop. It's late. They\u{2019}re here."),
            [
                "contraction \"Don't\": write the full words",
                "contraction \"It's\": write the full words",
                "contraction \"They\u{2019}re\": write the full words",
            ]
        );
    }

    #[test]
    fn possessives_pass() {
        assert_clean("The player's rig and the players' rigs.");
    }

    #[test]
    fn issue_gives_the_first_line_of_its_paragraph() {
        let text = format!("Short.\n\n{}\n{}.", words(13), words(13));
        let issues = check(&text);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].line, 3);
    }
}
