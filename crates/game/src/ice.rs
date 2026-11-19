#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Action {
    Alert,
    TraceBurst,
    DeepTrace,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Effect {
    Alert,
    Trace(u32),
}

impl Action {
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Alert => "ALERT",
            Self::TraceBurst => "TRACE BURST",
            Self::DeepTrace => "DEEP TRACE",
        }
    }

    pub(crate) fn ticks(self) -> u32 {
        match self {
            Self::Alert => 3,
            Self::TraceBurst => 2,
            Self::DeepTrace => 4,
        }
    }

    pub(crate) fn effect(self) -> Effect {
        match self {
            Self::Alert => Effect::Alert,
            Self::TraceBurst => Effect::Trace(8),
            Self::DeepTrace => Effect::Trace(20),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum IceKind {
    Sentry,
    Warden,
    Hound,
    Scorch,
}

impl IceKind {
    const ALL: [Self; 4] = [Self::Sentry, Self::Warden, Self::Hound, Self::Scorch];

    pub(crate) fn find(word: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.id() == word)
    }

    fn id(self) -> &'static str {
        match self {
            Self::Sentry => "sentry",
            Self::Warden => "warden",
            Self::Hound => "hound",
            Self::Scorch => "scorch",
        }
    }

    pub(crate) fn title(self) -> &'static str {
        match self {
            Self::Sentry => "Sentry",
            Self::Warden => "Warden",
            Self::Hound => "Hound",
            Self::Scorch => "Scorch",
        }
    }

    pub(crate) fn actions(self) -> &'static [Action] {
        match self {
            Self::Hound => &[Action::TraceBurst, Action::DeepTrace],
            Self::Sentry | Self::Warden | Self::Scorch => &[],
        }
    }

    pub(crate) fn weakness(self) -> &'static str {
        match self {
            Self::Sentry => "it trusts old signatures",
            Self::Warden => "its rule table has a hole",
            Self::Hound => "it leaks its trace buffer",
            Self::Scorch => "it does not check its heap",
        }
    }

    pub(crate) fn integrity(self, version: u32) -> u32 {
        let base = match self {
            Self::Sentry => 20,
            Self::Warden => 40,
            Self::Hound | Self::Scorch => 30,
        };
        base + 10 * version.saturating_sub(1)
    }
}

#[cfg(test)]
mod tests {
    use super::IceKind;

    #[test]
    fn kinds_are_found_by_their_content_name() {
        assert_eq!(IceKind::find("hound"), Some(IceKind::Hound));
        assert_eq!(IceKind::find("Hound"), None);
    }

    #[test]
    fn a_higher_version_has_more_integrity() {
        assert_eq!(IceKind::Sentry.integrity(1), 20);
        assert_eq!(IceKind::Sentry.integrity(3), 40);
    }
}
