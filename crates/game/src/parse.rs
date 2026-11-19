#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Stage {
    pub(crate) name: String,
    pub(crate) args: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ParseError {
    OpenQuote,
    EmptyStage,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Tail {
    pub(crate) words: Vec<String>,
    pub(crate) word: String,
}

pub(crate) fn pipeline(line: &str) -> Result<Vec<Stage>, ParseError> {
    feed(line)?.finish()
}

pub(crate) fn tail(line: &str) -> Option<Tail> {
    let builder = feed(line).ok().filter(|builder| !builder.quoted)?;
    Some(Tail {
        words: builder.words,
        word: builder.word,
    })
}

fn feed(line: &str) -> Result<Builder, ParseError> {
    let mut builder = Builder::default();
    for c in line.chars() {
        match c {
            '"' => builder.toggle_quote(),
            '|' if !builder.quoted => builder.end_stage()?,
            c if c.is_whitespace() && !builder.quoted => builder.end_word(),
            c => builder.push(c),
        }
    }
    Ok(builder)
}

#[derive(Default)]
struct Builder {
    stages: Vec<Stage>,
    words: Vec<String>,
    word: String,
    in_word: bool,
    quoted: bool,
}

impl Builder {
    fn toggle_quote(&mut self) {
        self.quoted = !self.quoted;
        self.in_word = true;
    }

    fn push(&mut self, c: char) {
        self.word.push(c);
        self.in_word = true;
    }

    fn end_word(&mut self) {
        if self.in_word {
            self.words.push(std::mem::take(&mut self.word));
            self.in_word = false;
        }
    }

    fn end_stage(&mut self) -> Result<(), ParseError> {
        self.end_word();
        let mut words = std::mem::take(&mut self.words).into_iter();
        let name = words.next().ok_or(ParseError::EmptyStage)?;
        self.stages.push(Stage {
            name,
            args: words.collect(),
        });
        Ok(())
    }

    fn finish(mut self) -> Result<Vec<Stage>, ParseError> {
        if self.quoted {
            return Err(ParseError::OpenQuote);
        }
        self.end_word();
        if self.stages.is_empty() && self.words.is_empty() {
            return Ok(Vec::new());
        }
        self.end_stage()?;
        Ok(self.stages)
    }
}

#[cfg(test)]
mod tests {
    use super::{ParseError, Stage, Tail, pipeline, tail};

    fn stage(name: &str, args: &[&str]) -> Stage {
        Stage {
            name: name.to_owned(),
            args: args.iter().map(|&arg| arg.to_owned()).collect(),
        }
    }

    fn open(words: &[&str], word: &str) -> Tail {
        Tail {
            words: words.iter().map(|&word| word.to_owned()).collect(),
            word: word.to_owned(),
        }
    }

    #[test]
    fn tail_gives_the_words_of_the_last_stage_and_the_open_word() {
        assert_eq!(tail("go ma"), Some(open(&["go"], "ma")));
        assert_eq!(tail("read a.txt  b"), Some(open(&["read", "a.txt"], "b")));
        assert_eq!(tail("go "), Some(open(&["go"], "")));
        assert_eq!(tail(""), Some(open(&[], "")));
    }

    #[test]
    fn tail_starts_again_after_a_pipe() {
        assert_eq!(tail("ls | gr"), Some(open(&[], "gr")));
        assert_eq!(tail("ls|"), Some(open(&[], "")));
        assert_eq!(tail("grep \"a | b\" | re"), Some(open(&[], "re")));
    }

    #[test]
    fn tail_gives_nothing_in_an_open_quote_or_after_an_empty_stage() {
        assert_eq!(tail("grep \"a b"), None);
        assert_eq!(tail("| gr"), None);
    }

    #[test]
    fn splits_words_at_spaces() {
        assert_eq!(pipeline("go  market"), Ok(vec![stage("go", &["market"])]));
    }

    #[test]
    fn splits_stages_at_pipes() {
        assert_eq!(
            pipeline("ls | grep .txt"),
            Ok(vec![stage("ls", &[]), stage("grep", &[".txt"])])
        );
    }

    #[test]
    fn pipes_need_no_spaces() {
        assert_eq!(pipeline("ls|grep x"), pipeline("ls | grep x"));
    }

    #[test]
    fn quotes_group_words_and_keep_pipes() {
        assert_eq!(
            pipeline("grep \"a | b\""),
            Ok(vec![stage("grep", &["a | b"])])
        );
    }

    #[test]
    fn empty_quotes_give_an_empty_argument() {
        assert_eq!(pipeline("grep \"\""), Ok(vec![stage("grep", &[""])]));
    }

    #[test]
    fn blank_lines_give_no_stages() {
        assert_eq!(pipeline("  \t "), Ok(Vec::new()));
    }

    #[test]
    fn empty_stages_fail() {
        for line in ["ls |", "| ls", "ls || ls"] {
            assert_eq!(pipeline(line), Err(ParseError::EmptyStage), "{line}");
        }
    }

    #[test]
    fn open_quotes_fail() {
        assert_eq!(pipeline("grep \"abc"), Err(ParseError::OpenQuote));
    }
}
