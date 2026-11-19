use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Instant;

use ttymud_game::is_chat;

pub(crate) const MAX_CONNECTIONS: usize = 256;
pub(crate) const MAX_PER_ADDRESS: usize = 8;

pub(crate) const TOO_FAST: &str = "You are sending commands too fast. Wait a moment.";
pub(crate) const TALKING_TOO_FAST: &str = "You are talking too fast. Wait a moment.";

#[derive(Debug, Clone, Copy)]
struct Bucket {
    capacity: f64,
    rate: f64,
    tokens: f64,
    last: Instant,
}

impl Bucket {
    fn new(capacity: f64, rate: f64, now: Instant) -> Self {
        Self {
            capacity,
            rate,
            tokens: capacity,
            last: now,
        }
    }

    fn take(&mut self, now: Instant) -> bool {
        let elapsed = now.saturating_duration_since(self.last).as_secs_f64();
        self.tokens = elapsed.mul_add(self.rate, self.tokens).min(self.capacity);
        self.last = now;
        if self.tokens < 1.0 {
            return false;
        }
        self.tokens -= 1.0;
        true
    }
}

#[derive(Debug)]
pub(crate) struct Limiter {
    commands: Bucket,
    chat: Bucket,
}

impl Limiter {
    pub(crate) fn new(now: Instant) -> Self {
        Self {
            commands: Bucket::new(20.0, 10.0, now),
            chat: Bucket::new(5.0, 0.5, now),
        }
    }

    pub(crate) fn check(&mut self, line: &str, now: Instant) -> Option<&'static str> {
        if !self.commands.take(now) {
            return Some(TOO_FAST);
        }
        if is_chat(line) && !self.chat.take(now) {
            return Some(TALKING_TOO_FAST);
        }
        None
    }

    pub(crate) fn completion(&mut self, now: Instant) -> bool {
        self.commands.take(now)
    }
}

#[derive(Debug)]
pub(crate) struct Limits {
    max_total: usize,
    max_per_address: usize,
    total: usize,
    by_address: HashMap<IpAddr, usize>,
}

impl Limits {
    pub(crate) fn new(max_total: usize, max_per_address: usize) -> Self {
        Self {
            max_total,
            max_per_address,
            total: 0,
            by_address: HashMap::new(),
        }
    }

    fn admit(&mut self, address: IpAddr) -> bool {
        let count = self.by_address.entry(address).or_default();
        if self.total >= self.max_total || *count >= self.max_per_address {
            return false;
        }
        *count += 1;
        self.total += 1;
        true
    }

    fn release(&mut self, address: IpAddr) {
        if let Some(count) = self.by_address.get_mut(&address) {
            *count = count.saturating_sub(1);
            if *count == 0 {
                self.by_address.remove(&address);
            }
            self.total = self.total.saturating_sub(1);
        }
    }
}

pub(crate) struct Admission {
    limits: Arc<Mutex<Limits>>,
    address: IpAddr,
}

impl Admission {
    pub(crate) fn new(limits: &Arc<Mutex<Limits>>, address: IpAddr) -> Option<Self> {
        let admitted = limits
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .admit(address);
        admitted.then(|| Self {
            limits: Arc::clone(limits),
            address,
        })
    }
}

impl Drop for Admission {
    fn drop(&mut self) {
        self.limits
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .release(self.address);
    }
}

#[cfg(test)]
mod tests {
    use std::net::{IpAddr, Ipv4Addr};
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, Instant};

    use super::{Admission, Bucket, Limiter, Limits, TALKING_TOO_FAST, TOO_FAST};

    fn address(last: u8) -> IpAddr {
        IpAddr::V4(Ipv4Addr::new(10, 0, 0, last))
    }

    #[test]
    fn a_bucket_allows_a_burst_and_then_refills() {
        let start = Instant::now();
        let mut bucket = Bucket::new(3.0, 1.0, start);
        assert!((0..3).all(|_| bucket.take(start)));
        assert!(!bucket.take(start));
        let later = start + Duration::from_secs(1);
        assert!(bucket.take(later));
        assert!(!bucket.take(later));
    }

    #[test]
    fn a_bucket_never_holds_more_than_its_capacity() {
        let start = Instant::now();
        let mut bucket = Bucket::new(3.0, 1.0, start);
        let much_later = start + Duration::from_secs(100);
        assert_eq!((0..10).filter(|_| bucket.take(much_later)).count(), 3);
    }

    #[test]
    fn the_limiter_slows_down_commands_after_a_burst() {
        let now = Instant::now();
        let mut limiter = Limiter::new(now);
        assert!((0..20).all(|_| limiter.check("look", now).is_none()));
        assert_eq!(limiter.check("look", now), Some(TOO_FAST));
    }

    #[test]
    fn chat_has_a_smaller_burst_than_commands() {
        let now = Instant::now();
        let mut limiter = Limiter::new(now);
        assert!((0..5).all(|_| limiter.check("say hi", now).is_none()));
        assert_eq!(limiter.check("'hi", now), Some(TALKING_TOO_FAST));
        assert_eq!(limiter.check("look", now), None);
    }

    #[test]
    fn completions_use_the_command_limit_but_not_the_chat_limit() {
        let now = Instant::now();
        let mut limiter = Limiter::new(now);
        assert!((0..15).all(|_| limiter.completion(now)));
        assert!((0..5).all(|_| limiter.check("say hi", now).is_none()));
        assert!(!limiter.completion(now));
    }

    #[test]
    fn each_address_has_a_limit() {
        let mut limits = Limits::new(100, 2);
        assert!(limits.admit(address(1)));
        assert!(limits.admit(address(1)));
        assert!(!limits.admit(address(1)));
        assert!(limits.admit(address(2)));
    }

    #[test]
    fn the_server_has_a_total_limit() {
        let mut limits = Limits::new(2, 8);
        assert!(limits.admit(address(1)));
        assert!(limits.admit(address(2)));
        assert!(!limits.admit(address(3)));
    }

    #[test]
    fn an_admission_frees_its_place_when_it_ends() {
        let limits = Arc::new(Mutex::new(Limits::new(1, 1)));
        let first = Admission::new(&limits, address(1));
        assert!(first.is_some());
        assert!(Admission::new(&limits, address(1)).is_none());
        drop(first);
        assert!(Admission::new(&limits, address(1)).is_some());
    }
}
