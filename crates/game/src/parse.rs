#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Stage {
    pub(crate) name: String,
    pub(crate) args: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Op {
    And,
    Or,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Link {
    pub(crate) op: Option<Op>,
    pub(crate) stages: Vec<Stage>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ParseError {
    OpenQuote,
    EmptyStage,
    EmptyLink,
    Chain,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Tail {
    pub(crate) words: Vec<String>,
    pub(crate) word: String,
}

pub(crate) fn pipeline(line: &str) -> Result<Vec<Stage>, ParseError> {
    let mut links = list(line)?;
    match links.len() {
        0 => Ok(Vec::new()),
        1 => Ok(links.remove(0).stages),
        _ => Err(ParseError::Chain),
    }
}

pub(crate) fn list(line: &str) -> Result<Vec<Link>, ParseError> {
    feed(line)?.finish()
}

pub(crate) fn tail(line: &str) -> Option<Tail> {
    let builder = feed(line).ok().filter(|builder| builder.quote.is_none())?;
    Some(Tail {
        words: builder.words,
        word: builder.word,
    })
}

fn feed(line: &str) -> Result<Builder, ParseError> {
    let mut builder = Builder::default();
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match (builder.quote, c) {
            (None, '"' | '\'') => builder.quote(Some(c)),
            (Some(quote), c) if c == quote => builder.quote(None),
            (None, '|') if chars.next_if_eq(&'|').is_some() => builder.end_link(Op::Or)?,
            (None, '&') if chars.next_if_eq(&'&').is_some() => builder.end_link(Op::And)?,
            (None, '|') => builder.end_stage()?,
            (None, c) if c.is_whitespace() => builder.end_word(),
            (_, c) => builder.push(c),
        }
    }
    Ok(builder)
}

#[derive(Default)]
struct Builder {
    links: Vec<Link>,
    op: Option<Op>,
    stages: Vec<Stage>,
    words: Vec<String>,
    word: String,
    in_word: bool,
    quote: Option<char>,
}

impl Builder {
    fn quote(&mut self, quote: Option<char>) {
        self.quote = quote;
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

    fn end_link(&mut self, next: Op) -> Result<(), ParseError> {
        self.close_link()?;
        self.op = Some(next);
        Ok(())
    }

    fn close_link(&mut self) -> Result<(), ParseError> {
        self.end_word();
        if self.stages.is_empty() && self.words.is_empty() {
            return Err(ParseError::EmptyLink);
        }
        self.end_stage()?;
        self.links.push(Link {
            op: self.op.take(),
            stages: std::mem::take(&mut self.stages),
        });
        Ok(())
    }

    fn finish(mut self) -> Result<Vec<Link>, ParseError> {
        if self.quote.is_some() {
            return Err(ParseError::OpenQuote);
        }
        self.end_word();
        if self.links.is_empty() && self.stages.is_empty() && self.words.is_empty() {
            return Ok(Vec::new());
        }
        self.close_link()?;
        Ok(self.links)
    }
}

#[cfg(test)]
mod tests {
    use super::{Link, Op, ParseError, Stage, Tail, list, pipeline, tail};

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
        assert_eq!(pipeline("grep ''"), Ok(vec![stage("grep", &[""])]));
    }

    #[test]
    fn single_quotes_keep_double_quotes_and_pipes() {
        assert_eq!(
            pipeline("echo 'status | grep \"Trace: 9\"' >> watch.sh"),
            Ok(vec![stage(
                "echo",
                &["status | grep \"Trace: 9\"", ">>", "watch.sh"]
            )])
        );
        assert_eq!(
            pipeline("grep \"it's\""),
            Ok(vec![stage("grep", &["it's"])])
        );
    }

    #[test]
    fn blank_lines_give_no_stages() {
        assert_eq!(pipeline("  \t "), Ok(Vec::new()));
    }

    #[test]
    fn empty_stages_fail() {
        for line in ["ls |", "| ls", "ls | && ls"] {
            assert_eq!(pipeline(line), Err(ParseError::EmptyStage), "{line}");
        }
    }

    #[test]
    fn open_quotes_fail() {
        assert_eq!(pipeline("grep \"abc"), Err(ParseError::OpenQuote));
        assert_eq!(pipeline("grep 'abc"), Err(ParseError::OpenQuote));
        assert_eq!(tail("echo 'a b"), None);
    }

    fn link(op: Option<Op>, stages: Vec<Stage>) -> Link {
        Link { op, stages }
    }

    #[test]
    fn a_list_splits_at_and_and_or() {
        assert_eq!(
            list("scan | probe && crack sentry || jackout"),
            Ok(vec![
                link(None, vec![stage("scan", &[]), stage("probe", &[])]),
                link(Some(Op::And), vec![stage("crack", &["sentry"])]),
                link(Some(Op::Or), vec![stage("jackout", &[])]),
            ])
        );
        assert_eq!(list("a&&b||c"), list("a && b || c"));
        assert_eq!(list("  "), Ok(Vec::new()));
    }

    #[test]
    fn quotes_keep_and_and_or_and_one_ampersand_is_a_character() {
        assert_eq!(
            list("echo 'a && b || c'"),
            Ok(vec![link(None, vec![stage("echo", &["a && b || c"])])])
        );
        assert_eq!(pipeline("grep &"), Ok(vec![stage("grep", &["&"])]));
    }

    #[test]
    fn each_side_of_and_and_or_needs_a_command() {
        for line in ["&& ls", "ls &&", "ls || || ls", "ls ||  "] {
            assert_eq!(list(line), Err(ParseError::EmptyLink), "{line}");
        }
    }

    #[test]
    fn a_pipeline_is_not_a_list() {
        assert_eq!(pipeline("scan && crack"), Err(ParseError::Chain));
        assert_eq!(pipeline("ls || ls"), Err(ParseError::Chain));
    }

    #[test]
    fn tail_starts_again_after_and_and_or() {
        assert_eq!(tail("scan && cr"), Some(open(&[], "cr")));
        assert_eq!(tail("scan || go m"), Some(open(&["go"], "m")));
    }
}
