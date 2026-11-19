const INSTRUCTION_LIMIT: usize = 20;
const DESCRIPTION_LIMIT: usize = 25;

const INSTRUCTION_VERBS: &[&str] = &[
    "add", "change", "check", "collect", "compile", "connect", "copy", "crack", "delete", "do",
    "download", "find", "fix", "follow", "format", "get", "give", "install", "keep", "leave",
    "load", "make", "move", "open", "prepare", "put", "read", "remove", "run", "scan", "select",
    "sell", "send", "set", "start", "stop", "tell", "type", "use", "wait", "watch", "write",
];

pub(crate) fn sentences(text: &str) -> Vec<&str> {
    let mut found = Vec::new();
    let mut start = 0;
    let mut chars = text.char_indices().peekable();
    while let Some((_, c)) = chars.next() {
        if !matches!(c, '.' | '!' | '?') {
            continue;
        }
        while chars
            .next_if(|&(_, next)| matches!(next, '"' | '\'' | ')' | '\u{201d}' | '\u{2019}'))
            .is_some()
        {}
        let end = match chars.peek() {
            None => text.len(),
            Some(&(index, next)) if next.is_whitespace() => index,
            Some(_) => continue,
        };
        push_trimmed(&mut found, &text[start..end]);
        start = end;
    }
    push_trimmed(&mut found, &text[start..]);
    found
}

fn push_trimmed<'a>(found: &mut Vec<&'a str>, piece: &'a str) {
    let piece = piece.trim();
    if !piece.is_empty() {
        found.push(piece);
    }
}

pub(crate) fn word_count(sentence: &str) -> usize {
    sentence
        .split_whitespace()
        .filter(|word| word.chars().any(char::is_alphanumeric))
        .count()
}

fn is_instruction(sentence: &str) -> bool {
    sentence
        .split_whitespace()
        .next()
        .map(|word| {
            word.trim_matches(|c: char| !c.is_alphanumeric())
                .to_lowercase()
        })
        .is_some_and(|word| INSTRUCTION_VERBS.contains(&word.as_str()))
}

pub(crate) fn word_limit(sentence: &str) -> usize {
    if is_instruction(sentence) {
        INSTRUCTION_LIMIT
    } else {
        DESCRIPTION_LIMIT
    }
}

pub(crate) fn opening(sentence: &str) -> String {
    let words: Vec<&str> = sentence.split_whitespace().take(6).collect();
    format!("{} ...", words.join(" "))
}

#[cfg(test)]
mod tests {
    use super::{is_instruction, opening, sentences, word_count, word_limit};

    #[test]
    fn splits_at_sentence_ends() {
        assert_eq!(
            sentences("One two. Three four! Five?"),
            ["One two.", "Three four!", "Five?"]
        );
    }

    #[test]
    fn keeps_versions_and_decimals_whole() {
        assert_eq!(
            sentences("Go from v2.1 to v2.2 now."),
            ["Go from v2.1 to v2.2 now."]
        );
    }

    #[test]
    fn keeps_closing_quotes_with_the_sentence() {
        assert_eq!(
            sentences("It says \"stop.\" Then it waits."),
            ["It says \"stop.\"", "Then it waits."]
        );
    }

    #[test]
    fn keeps_text_without_an_end_mark() {
        assert_eq!(sentences("These parts:"), ["These parts:"]);
    }

    #[test]
    fn counts_words_but_not_symbols() {
        assert_eq!(word_count("Trace -> 31% now"), 3);
    }

    #[test]
    fn known_verb_starts_an_instruction() {
        assert!(is_instruction("Make sure that it fails."));
        assert!(is_instruction("**Use** the key."));
        assert!(!is_instruction("The server makes a key."));
    }

    #[test]
    fn instructions_have_a_lower_limit() {
        assert_eq!(word_limit("Run the tests."), 20);
        assert_eq!(word_limit("The tests run."), 25);
    }

    #[test]
    fn opening_shows_the_first_six_words() {
        assert_eq!(
            opening("one two three four five six seven"),
            "one two three four five six ..."
        );
    }
}
