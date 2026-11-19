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

pub(crate) fn closest<'a>(
    word: &str,
    candidates: impl IntoIterator<Item = &'a str>,
) -> Option<&'a str> {
    let limit = if word.chars().count() <= 4 { 1 } else { 2 };
    candidates
        .into_iter()
        .map(|candidate| (distance(word, candidate), candidate))
        .filter(|&(edits, _)| edits <= limit)
        .min_by_key(|&(edits, _)| edits)
        .map(|(_, candidate)| candidate)
}

#[cfg(test)]
mod tests {
    use super::{closest, distance};

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
    fn ties_go_to_the_first_candidate() {
        assert_eq!(closest("ab", ["ac", "ad"]), Some("ac"));
    }
}
