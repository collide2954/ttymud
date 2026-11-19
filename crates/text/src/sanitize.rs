pub const MAX_LENGTH: usize = 400;

const MAX_MARKS: usize = 2;

#[must_use]
pub fn clean(text: &str) -> String {
    filter(text, usize::MAX)
}

#[must_use]
pub fn sanitize(text: &str) -> String {
    filter(text, MAX_LENGTH)
}

fn filter(text: &str, limit: usize) -> String {
    let mut kept = String::new();
    let mut length = 0;
    let mut marks = 0;
    for c in text.chars() {
        let c = if c == '\t' { ' ' } else { c };
        if is_removed(c) {
            continue;
        }
        if is_mark(c) {
            if length == 0 || marks == MAX_MARKS {
                continue;
            }
            marks += 1;
        } else {
            marks = 0;
        }
        if length == limit {
            break;
        }
        kept.push(c);
        length += 1;
    }
    kept
}

fn is_removed(c: char) -> bool {
    c.is_control()
        || matches!(
            c,
            '\u{ad}'
                | '\u{61c}'
                | '\u{115f}'
                | '\u{1160}'
                | '\u{180e}'
                | '\u{200b}'..='\u{200f}'
                | '\u{202a}'..='\u{202e}'
                | '\u{2060}'..='\u{2064}'
                | '\u{2066}'..='\u{2069}'
                | '\u{3164}'
                | '\u{feff}'
                | '\u{ffa0}'
                | '\u{fff9}'..='\u{fffb}'
                | '\u{e0000}'..='\u{e007f}'
        )
}

fn is_mark(c: char) -> bool {
    matches!(
        c,
        '\u{300}'..='\u{36f}'
            | '\u{1ab0}'..='\u{1aff}'
            | '\u{1dc0}'..='\u{1dff}'
            | '\u{20d0}'..='\u{20ff}'
            | '\u{fe20}'..='\u{fe2f}'
    )
}

#[cfg(test)]
mod tests {
    use super::{MAX_LENGTH, clean, sanitize};

    #[test]
    fn plain_text_stays_the_same() {
        assert_eq!(sanitize("Hello, Port Ostra!"), "Hello, Port Ostra!");
    }

    #[test]
    fn escape_characters_are_removed() {
        assert_eq!(sanitize("\u{1b}[31mred\u{1b}[0m"), "[31mred[0m");
    }

    #[test]
    fn c1_controls_are_removed() {
        assert_eq!(sanitize("a\u{9b}31mb"), "a31mb");
    }

    #[test]
    fn tabs_become_spaces_and_other_controls_go() {
        assert_eq!(sanitize("go\tmarket\r\n\u{7}"), "go market");
    }

    #[test]
    fn bidi_controls_are_removed() {
        assert_eq!(sanitize("abc\u{202e}fed\u{2066}x\u{2069}"), "abcfedx");
    }

    #[test]
    fn invisible_characters_are_removed() {
        assert_eq!(
            sanitize("a\u{200b}b\u{feff}c\u{2060}d\u{ad}e\u{3164}"),
            "abcde"
        );
    }

    #[test]
    fn tag_characters_are_removed() {
        assert_eq!(sanitize("hi\u{e0041}\u{e0042}"), "hi");
    }

    #[test]
    fn two_combining_marks_on_a_letter_stay() {
        assert_eq!(sanitize("e\u{323}\u{302}"), "e\u{323}\u{302}");
    }

    #[test]
    fn more_combining_marks_are_removed() {
        assert_eq!(
            sanitize("z\u{300}\u{301}\u{302}\u{303}a"),
            "z\u{300}\u{301}a"
        );
    }

    #[test]
    fn a_mark_without_a_letter_is_removed() {
        assert_eq!(sanitize("\u{301}x"), "x");
    }

    #[test]
    fn long_text_is_cut_at_the_limit() {
        let long = "x".repeat(MAX_LENGTH + 100);
        assert_eq!(sanitize(&long).chars().count(), MAX_LENGTH);
    }

    #[test]
    fn clean_removes_the_same_characters_without_a_length_limit() {
        let long = "x".repeat(MAX_LENGTH + 100);
        assert_eq!(clean(&long), long);
        assert_eq!(clean("a\u{1b}[2Jb\u{202e}c"), "a[2Jbc");
    }

    #[test]
    fn other_scripts_and_emoji_stay() {
        assert_eq!(sanitize("Привет 你好 🙂"), "Привет 你好 🙂");
    }
}
