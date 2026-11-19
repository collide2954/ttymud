use std::time::{Duration, Instant};

pub(crate) const TICK: Duration = Duration::from_secs(2);

pub(crate) struct Ticker {
    every: Duration,
    next: Instant,
}

impl Ticker {
    pub(crate) fn new(every: Duration, now: Instant) -> Self {
        Self {
            every,
            next: now + every,
        }
    }

    pub(crate) fn left(&self, now: Instant) -> Duration {
        self.next.saturating_duration_since(now)
    }

    pub(crate) fn due(&mut self, now: Instant) -> bool {
        if now < self.next {
            return false;
        }
        self.next += self.every;
        if self.next <= now {
            self.next = now + self.every;
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::Ticker;

    const EVERY: Duration = Duration::from_secs(2);

    #[test]
    fn a_tick_is_due_after_each_interval() {
        let start = Instant::now();
        let mut ticker = Ticker::new(EVERY, start);
        assert_eq!(ticker.left(start), EVERY);
        assert!(!ticker.due(start + Duration::from_secs(1)));
        assert_eq!(
            ticker.left(start + Duration::from_secs(1)),
            Duration::from_secs(1)
        );
        assert!(ticker.due(start + EVERY));
        assert_eq!(ticker.left(start + EVERY), EVERY);
    }

    #[test]
    fn a_late_loop_skips_the_ticks_that_it_missed() {
        let start = Instant::now();
        let mut ticker = Ticker::new(EVERY, start);
        let late = start + Duration::from_secs(7);
        assert!(ticker.due(late));
        assert!(!ticker.due(late));
        assert_eq!(ticker.left(late), EVERY);
    }

    #[test]
    fn no_time_is_left_when_a_tick_is_due() {
        let start = Instant::now();
        let ticker = Ticker::new(EVERY, start);
        assert_eq!(ticker.left(start + Duration::from_secs(3)), Duration::ZERO);
    }
}
