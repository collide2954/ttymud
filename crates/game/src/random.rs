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
}

#[cfg(test)]
mod tests {
    use super::Random;

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
    fn below_stays_under_its_limit() {
        let mut random = Random::new(0);
        assert!((0..1000).all(|_| random.below(3) < 3));
    }
}
