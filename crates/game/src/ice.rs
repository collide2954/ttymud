pub(crate) const FREEZE_TICKS: u32 = 6;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Action {
    Alert,
    TraceBurst,
    DeepTrace,
    Burn,
    Spike,
    Tar,
    Bite,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Effect {
    Alert,
    Trace(u32),
    Damage(u32),
    Drain,
    Freeze(u32),
}

impl Action {
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Alert => "ALERT",
            Self::TraceBurst => "TRACE BURST",
            Self::DeepTrace => "DEEP TRACE",
            Self::Burn => "BURN",
            Self::Spike => "SPIKE",
            Self::Tar => "TAR",
            Self::Bite => "BITE",
        }
    }

    pub(crate) fn ticks(self) -> u32 {
        match self {
            Self::Spike => 1,
            Self::TraceBurst | Self::Burn | Self::Tar | Self::Bite => 2,
            Self::Alert => 3,
            Self::DeepTrace => 4,
        }
    }

    pub(crate) fn effect(self) -> Effect {
        match self {
            Self::Alert => Effect::Alert,
            Self::TraceBurst => Effect::Trace(8),
            Self::DeepTrace => Effect::Trace(20),
            Self::Burn => Effect::Damage(20),
            Self::Spike => Effect::Damage(8),
            Self::Tar => Effect::Drain,
            Self::Bite => Effect::Freeze(FREEZE_TICKS),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum IceKind {
    Sentry,
    Warden,
    Hound,
    Scorch,
    Tarpit,
    Lockjaw,
}

impl IceKind {
    const ALL: [Self; 6] = [
        Self::Sentry,
        Self::Warden,
        Self::Hound,
        Self::Scorch,
        Self::Tarpit,
        Self::Lockjaw,
    ];

    pub(crate) fn find(word: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.id() == word)
    }

    fn id(self) -> &'static str {
        match self {
            Self::Sentry => "sentry",
            Self::Warden => "warden",
            Self::Hound => "hound",
            Self::Scorch => "scorch",
            Self::Tarpit => "tarpit",
            Self::Lockjaw => "lockjaw",
        }
    }

    pub(crate) fn title(self) -> &'static str {
        match self {
            Self::Sentry => "Sentry",
            Self::Warden => "Warden",
            Self::Hound => "Hound",
            Self::Scorch => "Scorch",
            Self::Tarpit => "Tarpit",
            Self::Lockjaw => "Lockjaw",
        }
    }

    pub(crate) fn actions(self) -> &'static [Action] {
        match self {
            Self::Hound => &[Action::TraceBurst, Action::DeepTrace],
            Self::Scorch => &[Action::Burn, Action::Spike],
            Self::Tarpit => &[Action::Tar],
            Self::Lockjaw => &[Action::Bite],
            Self::Sentry | Self::Warden => &[],
        }
    }

    pub(crate) fn local(self) -> bool {
        matches!(self, Self::Scorch | Self::Tarpit | Self::Lockjaw)
    }

    pub(crate) fn weakness(self) -> &'static str {
        match self {
            Self::Sentry => "it trusts old signatures",
            Self::Warden => "its rule table has a hole",
            Self::Hound => "it leaks its trace buffer",
            Self::Scorch => "it does not check its heap",
            Self::Tarpit => "it never frees its own buffers",
            Self::Lockjaw => "its jaw resets on a bad packet",
        }
    }

    pub(crate) fn integrity(self, version: u32) -> u32 {
        let base = match self {
            Self::Sentry => 20,
            Self::Tarpit => 25,
            Self::Warden => 40,
            Self::Hound | Self::Scorch | Self::Lockjaw => 30,
        };
        base + 10 * version.saturating_sub(1)
    }
}

#[cfg(test)]
mod tests {
    use super::{Action, Effect, IceKind};

    #[test]
    fn kinds_are_found_by_their_content_name() {
        assert_eq!(IceKind::find("hound"), Some(IceKind::Hound));
        assert_eq!(IceKind::find("lockjaw"), Some(IceKind::Lockjaw));
        assert_eq!(IceKind::find("Hound"), None);
    }

    #[test]
    fn a_higher_version_has_more_integrity() {
        assert_eq!(IceKind::Sentry.integrity(1), 20);
        assert_eq!(IceKind::Sentry.integrity(3), 40);
        assert_eq!(IceKind::Tarpit.integrity(2), 35);
    }

    #[test]
    fn tarpit_and_lockjaw_stay_on_their_host_and_telegraph_their_attacks() {
        assert!(IceKind::Tarpit.local() && IceKind::Lockjaw.local());
        assert!(!IceKind::Hound.local());
        assert_eq!(IceKind::Tarpit.actions(), [Action::Tar]);
        assert_eq!(Action::Tar.effect(), Effect::Drain);
        assert_eq!(Action::Bite.effect(), Effect::Freeze(6));
        assert_eq!((Action::Tar.ticks(), Action::Bite.ticks()), (2, 2));
    }
}
