mod address;
pub mod ini;
mod sanitize;
mod status;

pub use address::{is_address, is_server_name};
pub use sanitize::{MAX_LENGTH, clean, sanitize};
pub use status::{Mate, RunStatus, Status};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Game,
    Error,
    Chat,
    Telegraph,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Style {
    Plain,
    Title,
    Host,
    File,
    Command,
    Player,
    Ice,
    Danger,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Span {
    pub style: Style,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    pub kind: Kind,
    pub spans: Vec<Span>,
}

impl Message {
    #[must_use]
    pub fn new(kind: Kind) -> Self {
        Self {
            kind,
            spans: Vec::new(),
        }
    }

    #[must_use]
    pub fn game(text: impl Into<String>) -> Self {
        Self::new(Kind::Game).text(text)
    }

    #[must_use]
    pub fn error(text: impl Into<String>) -> Self {
        Self::new(Kind::Error).text(text)
    }

    #[must_use]
    pub fn text(self, text: impl Into<String>) -> Self {
        self.span(Style::Plain, text)
    }

    #[must_use]
    pub fn span(mut self, style: Style, text: impl Into<String>) -> Self {
        let text = text.into();
        if !text.is_empty() {
            self.spans.push(Span { style, text });
        }
        self
    }

    #[must_use]
    pub fn plain(&self) -> String {
        self.spans.iter().map(|span| span.text.as_str()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::{Kind, Message, Span, Style};

    #[test]
    fn plain_joins_the_text_of_all_spans() {
        let message = Message::game("Routes: ").span(Style::Host, "relay");
        assert_eq!(message.plain(), "Routes: relay");
    }

    #[test]
    fn spans_keep_their_styles_in_order() {
        let message = Message::new(Kind::Game)
            .span(Style::Title, "The Relay")
            .text(" is here.");
        assert_eq!(
            message.spans,
            [
                Span {
                    style: Style::Title,
                    text: "The Relay".to_owned()
                },
                Span {
                    style: Style::Plain,
                    text: " is here.".to_owned()
                },
            ]
        );
    }

    #[test]
    fn empty_text_adds_no_span() {
        assert_eq!(Message::game("").spans, Vec::<Span>::new());
    }

    #[test]
    fn error_messages_have_the_error_kind() {
        let message = Message::error("No route.");
        assert_eq!(
            (message.kind, message.plain()),
            (Kind::Error, "No route.".to_owned())
        );
    }
}
