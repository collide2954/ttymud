#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Layer {
    Edge,
    Lan,
    Core,
}

impl Layer {
    pub(crate) const ALL: [Self; 3] = [Self::Edge, Self::Lan, Self::Core];

    pub(crate) fn find(word: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|layer| layer.word() == word)
    }

    pub(crate) fn word(self) -> &'static str {
        match self {
            Self::Edge => "edge",
            Self::Lan => "lan",
            Self::Core => "core",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Layer;

    #[test]
    fn layers_are_found_by_their_content_word() {
        assert_eq!(Layer::find("lan"), Some(Layer::Lan));
        assert_eq!(Layer::find("vault"), None);
    }
}
