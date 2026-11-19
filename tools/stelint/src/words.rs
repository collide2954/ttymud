const CONTRACTED_ENDINGS: &[&str] = &["n't", "'re", "'ll", "'ve", "'d", "'m"];

const CONTRACTED_WORDS: &[&str] = &[
    "here's", "he's", "how's", "it's", "let's", "she's", "that's", "there's", "what's", "where's",
    "who's",
];

pub(crate) fn contractions(text: &str) -> Vec<&str> {
    text.split_whitespace()
        .map(|word| word.trim_matches(|c: char| !c.is_alphanumeric() && !is_apostrophe(c)))
        .filter(|word| is_contraction(word))
        .collect()
}

pub(crate) fn lowercase_words(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphanumeric() && !is_apostrophe(c))
        .map(|word| word.trim_matches(is_apostrophe))
        .filter(|word| !word.is_empty())
        .map(str::to_lowercase)
        .collect()
}

fn is_apostrophe(c: char) -> bool {
    c == '\'' || c == '\u{2019}'
}

fn is_contraction(word: &str) -> bool {
    let plain = word.replace('\u{2019}', "'").to_lowercase();
    CONTRACTED_ENDINGS
        .iter()
        .any(|ending| plain.ends_with(ending))
        || CONTRACTED_WORDS.contains(&plain.as_str())
}
