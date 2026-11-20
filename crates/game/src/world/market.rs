use std::collections::BTreeMap;

pub(super) const FULL: u32 = 100;
pub(super) const FLOOR: u32 = 30;
const DROP: u32 = 10;
const RECOVERY: u64 = 2 * 60 * 60;

#[derive(Debug, Default)]
pub(super) struct Market {
    prices: BTreeMap<String, (u32, u64)>,
}

impl Market {
    pub(super) fn percent(&self, name: &str, now: u64) -> u32 {
        let Some(&(percent, since)) = self.prices.get(name) else {
            return FULL;
        };
        let steps = now.saturating_sub(since) / RECOVERY;
        let back = u32::try_from(steps).map_or(FULL, |steps| steps.saturating_mul(DROP));
        percent.saturating_add(back).min(FULL)
    }

    pub(super) fn price(&self, name: &str, value: u32, now: u64) -> u32 {
        value * self.percent(name, now) / FULL
    }

    pub(super) fn sell(&mut self, name: &str, now: u64) {
        let percent = self.percent(name, now).saturating_sub(DROP).max(FLOOR);
        self.prices.insert(name.to_owned(), (percent, now));
    }

    pub(super) fn low(&self, now: u64) -> impl Iterator<Item = (&str, u32, u64)> {
        self.prices
            .iter()
            .filter(move |(name, _)| self.percent(name, now) < FULL)
            .map(|(name, &(percent, since))| (name.as_str(), percent, since))
    }

    pub(super) fn restore(&mut self, name: &str, percent: u32, since: u64) {
        self.prices.insert(name.to_owned(), (percent, since));
    }
}

#[cfg(test)]
mod tests {
    use super::Market;

    const HOUR: u64 = 3_600;

    #[test]
    fn each_sale_lowers_the_price_by_ten_points_to_thirty_percent() {
        let mut market = Market::default();
        let seen: Vec<u32> = (0..9)
            .map(|_| {
                market.sell("payroll.db", 0);
                market.percent("payroll.db", 0)
            })
            .collect();
        assert_eq!(seen, [90, 80, 70, 60, 50, 40, 30, 30, 30]);
        assert_eq!(market.price("payroll.db", 40, 0), 12);
        assert_eq!(market.percent("clients.db", 0), 100);
    }

    #[test]
    fn the_price_comes_back_ten_points_every_two_hours() {
        let mut market = Market::default();
        for _ in 0..7 {
            market.sell("payroll.db", 0);
        }
        let times = [0, HOUR, 2 * HOUR, 13 * HOUR, 14 * HOUR, 100 * HOUR];
        assert_eq!(
            times.map(|at| market.percent("payroll.db", at)),
            [30, 30, 40, 90, 100, 100]
        );
        market.sell("payroll.db", 3 * HOUR);
        assert_eq!(market.percent("payroll.db", 3 * HOUR), 30);
    }

    #[test]
    fn only_the_low_prices_are_kept() {
        let mut market = Market::default();
        market.sell("payroll.db", 0);
        market.sell("clients.db", 4 * HOUR);
        let low: Vec<(&str, u32, u64)> = market.low(4 * HOUR).collect();
        assert_eq!(low, [("clients.db", 90, 4 * HOUR)]);
    }
}
