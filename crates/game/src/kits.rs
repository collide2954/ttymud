use crate::commands::Verb;
use crate::matching::{self, Found};

pub(crate) const RAM: u32 = 8;
pub(crate) const CYCLES: u32 = 4;

#[derive(Debug)]
pub(crate) struct Kit {
    pub(crate) id: &'static str,
    pub(crate) summary: &'static str,
    pub(crate) loads: [Verb; 5],
    pub(crate) knows: &'static [Verb],
}

pub(crate) const KITS: [Kit; 4] = [
    Kit {
        id: "breaker",
        summary: "Breaks ICE and takes the hits.",
        loads: [
            Verb::Scan,
            Verb::Probe,
            Verb::Crack,
            Verb::Patch,
            Verb::Firewall,
        ],
        knows: &[Verb::Decoy],
    },
    Kit {
        id: "ghost",
        summary: "Stays hidden from sentries.",
        loads: [
            Verb::Scan,
            Verb::Probe,
            Verb::Crack,
            Verb::Scrub,
            Verb::Mask,
        ],
        knows: &[],
    },
    Kit {
        id: "phreak",
        summary: "Slows the trace.",
        loads: [
            Verb::Scan,
            Verb::Probe,
            Verb::Crack,
            Verb::Scrub,
            Verb::Proxy,
        ],
        knows: &[],
    },
    Kit {
        id: "face",
        summary: "Talks past wardens.",
        loads: [
            Verb::Scan,
            Verb::Probe,
            Verb::Crack,
            Verb::Patch,
            Verb::Pretext,
        ],
        knows: &[],
    },
];

pub(crate) fn find(word: &str) -> Option<&'static Kit> {
    let ids: Vec<&str> = KITS.iter().map(|kit| kit.id).collect();
    match matching::resolve(word, &ids) {
        Found::One(id) => KITS.iter().find(|kit| kit.id == id),
        Found::Many(_) | Found::Nothing(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{KITS, RAM, find};
    use crate::commands::Verb;

    #[test]
    fn each_kit_fills_the_ram_exactly() {
        for kit in &KITS {
            let used: u32 = kit
                .loads
                .iter()
                .filter_map(|verb| verb.program())
                .map(|program| program.ram)
                .sum();
            assert_eq!(used, RAM, "{}", kit.id);
        }
    }

    #[test]
    fn kits_are_found_by_a_unique_prefix() {
        assert_eq!(find("GH").map(|kit| kit.id), Some("ghost"));
        assert_eq!(find("x").map(|kit| kit.id), None);
        assert!(find("face").is_some_and(|kit| kit.loads.contains(&Verb::Pretext)));
    }
}
