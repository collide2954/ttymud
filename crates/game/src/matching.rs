pub(crate) fn distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut table = vec![vec![0; b.len() + 1]; a.len() + 1];
    for (i, row) in table.iter_mut().enumerate() {
        row[0] = i;
    }
    for (j, cell) in table[0].iter_mut().enumerate() {
        *cell = j;
    }
    for i in 1..=a.len() {
        for j in 1..=b.len() {
            let cost = usize::from(a[i - 1] != b[j - 1]);
            let mut best = (table[i - 1][j] + 1)
                .min(table[i][j - 1] + 1)
                .min(table[i - 1][j - 1] + cost);
            if i > 1 && j > 1 && a[i - 1] == b[j - 2] && a[i - 2] == b[j - 1] {
                best = best.min(table[i - 2][j - 2] + 1);
            }
            table[i][j] = best;
        }
    }
    table[a.len()][b.len()]
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Found<'a> {
    One(&'a str),
    Many(Vec<&'a str>),
    Nothing(Option<&'a str>),
}

pub(crate) fn resolve<'a>(word: &str, names: &[&'a str]) -> Found<'a> {
    let word = word.to_lowercase();
    if let Some(&exact) = names.iter().find(|&&name| name == word) {
        return Found::One(exact);
    }
    let prefixed: Vec<&'a str> = names
        .iter()
        .copied()
        .filter(|name| name.starts_with(&word))
        .collect();
    match prefixed.len() {
        0 => Found::Nothing(closest(&word, names.iter().copied())),
        1 => Found::One(prefixed[0]),
        _ => Found::Many(prefixed),
    }
}

pub(crate) fn closest<'a>(
    word: &str,
    candidates: impl IntoIterator<Item = &'a str>,
) -> Option<&'a str> {
    let length = word.chars().count();
    let limit = if length <= 4 { 1 } else { 2 };
    candidates
        .into_iter()
        .map(|candidate| {
            let edits = distance(word, candidate);
            (edits, candidate.chars().count().abs_diff(length), candidate)
        })
        .filter(|&(edits, _, _)| edits <= limit)
        .min_by_key(|&(edits, difference, _)| (edits, difference))
        .map(|(_, _, candidate)| candidate)
}

#[cfg(test)]
mod tests {
    use super::{Found, closest, distance, resolve};

    #[test]
    fn resolve_prefers_an_exact_name() {
        assert_eq!(
            resolve("market", &["market-2", "market"]),
            Found::One("market")
        );
    }

    #[test]
    fn resolve_accepts_a_unique_prefix() {
        assert_eq!(resolve("MA", &["market", "relay"]), Found::One("market"));
    }

    #[test]
    fn resolve_gives_all_names_with_a_shared_prefix() {
        assert_eq!(
            resolve("ba", &["back-room", "bazaar"]),
            Found::Many(vec!["back-room", "bazaar"])
        );
    }

    #[test]
    fn resolve_suggests_a_near_name() {
        assert_eq!(
            resolve("markte", &["market"]),
            Found::Nothing(Some("market"))
        );
    }

    #[test]
    fn identical_words_have_no_distance() {
        assert_eq!(distance("look", "look"), 0);
    }

    #[test]
    fn each_edit_counts_one() {
        assert_eq!(distance("crak", "crack"), 1);
        assert_eq!(distance("look", "lock"), 1);
        assert_eq!(distance("ls", ""), 2);
    }

    #[test]
    fn swapped_neighbors_count_one() {
        assert_eq!(distance("gerp", "grep"), 1);
    }

    #[test]
    fn closest_picks_the_nearest_word() {
        assert_eq!(closest("lok", ["ls", "look"]), Some("look"));
    }

    #[test]
    fn closest_ignores_far_words() {
        assert_eq!(closest("xyzzy", ["look", "ls"]), None);
    }

    #[test]
    fn short_words_allow_one_edit() {
        assert_eq!(closest("lx", ["ls"]), Some("ls"));
        assert_eq!(closest("xyz", ["ls"]), None);
    }

    #[test]
    fn equal_edits_prefer_the_same_length() {
        assert_eq!(closest("lx", ["l", "ls"]), Some("ls"));
    }

    #[test]
    fn ties_go_to_the_first_candidate() {
        assert_eq!(closest("ab", ["ac", "ad"]), Some("ac"));
    }
}
