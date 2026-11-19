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

    const PROGRAMS: [(Verb, Self); 5] = [
        (Verb::Scan, Self::Scan),
        (Verb::Probe, Self::Probe),
        (Verb::Crack, Self::Crack),
        (Verb::Scrub, Self::Scrub),
        (Verb::Patch, Self::Patch),
    ];

    pub(crate) fn of(verb: Verb) -> Option<Self> {
        let found = Self::PROGRAMS.iter().find(|&&(program, _)| program == verb);
        found.map(|&(_, event)| event)
    }

    pub(crate) fn program(self) -> Option<Verb> {
        let found = Self::PROGRAMS.iter().find(|&&(_, event)| event == self);
        found.map(|&(program, _)| program)
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

    #[test]
    fn program_events_name_their_program() {
        assert_eq!(Event::Scrub.program(), Some(Verb::Scrub));
        assert_eq!(Event::Exit.program(), None);
    }
}
