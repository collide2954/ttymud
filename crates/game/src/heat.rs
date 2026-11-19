pub(crate) const MAX_HEAT: u32 = 5;
pub(crate) const COOLDOWN: u64 = 86_400;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Security {
    Low,
    Medium,
    High,
    Top,
}

impl Security {
    pub(crate) fn find(word: &str) -> Option<Self> {
        match word {
            "low" => Some(Self::Low),
            "medium" => Some(Self::Medium),
            "high" => Some(Self::High),
            "top" => Some(Self::Top),
            _ => None,
        }
    }

    pub(crate) fn heat(self) -> u32 {
        match self {
            Self::Low => 1,
            Self::Medium => 2,
            Self::High => 3,
            Self::Top => 5,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Security;

    #[test]
    fn levels_are_found_by_their_content_word() {
        assert_eq!(Security::find("top"), Some(Security::Top));
        assert_eq!(Security::find("Top"), None);
    }

    #[test]
    fn a_higher_level_gives_more_heat() {
        let heat: Vec<u32> = [
            Security::Low,
            Security::Medium,
            Security::High,
            Security::Top,
        ]
        .into_iter()
        .map(Security::heat)
        .collect();
        assert_eq!(heat, [1, 2, 3, 5]);
    }
}
