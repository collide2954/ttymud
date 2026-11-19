mod sentence;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Issue {
    pub line: usize,
    pub message: String,
}

#[must_use]
pub fn check(markdown: &str) -> Vec<Issue> {
    let mut issues = Vec::new();
    for (line, paragraph) in paragraphs(markdown) {
        for text in sentence::sentences(&paragraph) {
            let words = sentence::word_count(text);
            let limit = sentence::word_limit(text);
            if words > limit {
                issues.push(Issue {
                    line,
                    message: format!(
                        "sentence has {words} words (max {limit}): {}",
                        sentence::opening(text)
                    ),
                });
            }
        }
    }
    issues
}

fn paragraphs(markdown: &str) -> Vec<(usize, String)> {
    let mut found: Vec<(usize, String)> = Vec::new();
    let mut open = false;
    for (index, line) in markdown.lines().enumerate() {
        let line_text = line.trim();
        if line_text.is_empty() {
            open = false;
        } else if open && let Some((_, text)) = found.last_mut() {
            text.push(' ');
            text.push_str(line_text);
        } else {
            found.push((index + 1, line_text.to_owned()));
            open = true;
        }
    }
    found
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
    fn issue_gives_the_first_line_of_its_paragraph() {
        let text = format!("Short.\n\n{}\n{}.", words(13), words(13));
        let issues = check(&text);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].line, 3);
    }
}
