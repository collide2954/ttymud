use crate::commands::Verb;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Event {
    Home,
    Enter,
    Scan,
    Probe,
    Crack,
    Download,
    Scrub,
    Patch,
    Exit,
}

impl Event {
    const ALL: [Self; 9] = [
        Self::Home,
        Self::Enter,
        Self::Scan,
        Self::Probe,
        Self::Crack,
        Self::Download,
        Self::Scrub,
        Self::Patch,
        Self::Exit,
    ];

    pub(crate) fn find(word: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|event| event.word() == word)
    }

    pub(crate) fn of(verb: Verb) -> Option<Self> {
        match verb {
            Verb::Scan => Some(Self::Scan),
            Verb::Probe => Some(Self::Probe),
            Verb::Crack => Some(Self::Crack),
            Verb::Scrub => Some(Self::Scrub),
            Verb::Patch => Some(Self::Patch),
            _ => None,
        }
    }

    fn word(self) -> &'static str {
        match self {
            Self::Home => "home",
            Self::Enter => "enter",
            Self::Scan => "scan",
            Self::Probe => "probe",
            Self::Crack => "crack",
            Self::Download => "download",
            Self::Scrub => "scrub",
            Self::Patch => "patch",
            Self::Exit => "exit",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Event;
    use crate::commands::Verb;

    #[test]
    fn events_are_found_by_their_content_word() {
        assert_eq!(Event::find("download"), Some(Event::Download));
        assert_eq!(Event::find("Download"), None);
    }

    #[test]
    fn programs_give_their_own_event() {
        assert_eq!(Event::of(Verb::Crack), Some(Event::Crack));
        assert_eq!(Event::of(Verb::Look), None);
    }
}
