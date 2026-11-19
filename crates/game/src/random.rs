#[derive(Debug, Clone)]
pub(crate) struct Random(u64);

impl Random {
    pub(crate) fn new(seed: u64) -> Self {
        Self(seed)
    }

    pub(crate) fn draw(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut value = self.0;
        value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        value ^ (value >> 31)
    }

    pub(crate) fn below(&mut self, limit: usize) -> usize {
        let limit = u64::try_from(limit).unwrap_or(u64::MAX).max(1);
        usize::try_from(self.draw() % limit).unwrap_or(0)
    }

    pub(crate) fn shuffle<T>(&mut self, items: &mut [T]) {
        for last in (1..items.len()).rev() {
            let other = self.below(last + 1);
            items.swap(last, other);
        }
    }
}

pub(crate) fn mix(parts: &[u64]) -> u64 {
    parts.iter().fold(0x2545_f491_4f6c_dd1d, |seed, &part| {
        Random::new(seed ^ part.rotate_left(17)).draw()
    })
}

#[cfg(test)]
mod tests {
    use super::{Random, mix};

    #[test]
    fn the_same_seed_gives_the_same_numbers() {
        let (mut a, mut b) = (Random::new(7), Random::new(7));
        let first: Vec<u64> = (0..5).map(|_| a.draw()).collect();
        let second: Vec<u64> = (0..5).map(|_| b.draw()).collect();
        assert_eq!(first, second);
    }

    #[test]
    fn different_seeds_give_different_numbers() {
        assert_ne!(Random::new(1).draw(), Random::new(2).draw());
    }

    #[test]
    fn shuffle_keeps_the_items_and_follows_the_seed() {
        let mut items: Vec<u32> = (0..10).collect();
        Random::new(3).shuffle(&mut items);
        let mut again: Vec<u32> = (0..10).collect();
        Random::new(3).shuffle(&mut again);
        assert_eq!(items, again);
        assert_ne!(items, (0..10).collect::<Vec<u32>>());
        items.sort_unstable();
        assert_eq!(items, (0..10).collect::<Vec<u32>>());
    }

    #[test]
    fn mix_gives_a_different_seed_for_each_part() {
        assert_eq!(mix(&[1, 2]), mix(&[1, 2]));
        assert_ne!(mix(&[1, 2]), mix(&[2, 1]));
        assert_ne!(mix(&[1, 2]), mix(&[1, 3]));
    }

    #[test]
    fn below_stays_under_its_limit() {
        let mut random = Random::new(0);
        assert!((0..1000).all(|_| random.below(3) < 3));
    }
}
