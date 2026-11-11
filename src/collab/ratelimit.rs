//! Token buckets that keep one guest from flooding the host.
//!
//! Each guest gets its own buckets, so one noisy client can only wear out its
//! own welcome — the host and everyone else keep working normally. `Open` is
//! refused with a message because the guest is waiting for an answer; `Edit`
//! and `Cursor` are dropped silently because they are high-frequency and the
//! next legitimate one will arrive a moment later.

use std::time::Instant;

pub struct Bucket {
    capacity: f64,
    per_sec: f64,
    tokens: f64,
    last: Instant,
}

impl Bucket {
    pub fn new(capacity: u32, per_sec: u32) -> Self {
        Self { capacity: capacity as f64, per_sec: per_sec as f64, tokens: capacity as f64, last: Instant::now() }
    }

    /// Refill what time has added, then spend `weight` tokens if they are there.
    ///
    /// A message is not a unit of work: one `Edit` can carry a megabyte of text
    /// the host has to run through the merge machinery for every peer. Charging
    /// by size stops that without a separate size limit.
    pub fn take_weight(&mut self, weight: f64) -> bool {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last).as_secs_f64();
        self.last = now;
        self.tokens = (self.tokens + elapsed * self.per_sec).min(self.capacity);
        let weight = weight.min(self.capacity);
        if self.tokens >= weight {
            self.tokens -= weight;
            true
        } else {
            false
        }
    }

    /// Spend one token, or none if the bucket is empty. Tests only; every real
    /// call site passes a weight.
    #[cfg(test)]
    pub fn take(&mut self) -> bool {
        self.take_weight(1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread::sleep;
    use std::time::Duration;

    #[test]
    fn allows_burst_up_to_capacity() {
        let mut b = Bucket::new(10, 1);
        for _ in 0..10 {
            assert!(b.take());
        }
    }

    #[test]
    fn throttles_after_burst() {
        let mut b = Bucket::new(10, 1);
        for _ in 0..10 {
            assert!(b.take());
        }
        assert!(!b.take());
    }

    #[test]
    fn refills_over_time() {
        let mut b = Bucket::new(10, 10);
        for _ in 0..10 {
            b.take();
        }
        assert!(!b.take());
        sleep(Duration::from_millis(350));
        let mut got = 0;
        while b.take() {
            got += 1;
        }
        assert!((2..=4).contains(&got), "expected ~3 refilled tokens, got {got}");
    }

    #[test]
    fn never_exceeds_capacity() {
        let mut b = Bucket::new(3, 1);
        sleep(Duration::from_millis(300));
        for _ in 0..3 {
            assert!(b.take());
        }
        assert!(!b.take(), "idle time must not raise the bucket above its capacity");
    }

    #[test]
    fn a_bulk_edit_costs_more_than_a_keystroke() {
        // 60 tokens: 60 keystrokes.
        let mut b = Bucket::new(60, 60);
        for _ in 0..60 {
            assert!(b.take());
        }
        assert!(!b.take(), "keystrokes alone should exhaust the bucket");

        // The same bucket fits only 20 edits of 2 KiB each (1 + 2 tokens).
        let mut b = Bucket::new(60, 60);
        let mut accepted = 0;
        while b.take_weight(1.0 + 2048.0 / 1024.0) {
            accepted += 1;
        }
        assert_eq!(accepted, 20, "a 2 KiB paste must cost three tokens, not one");
    }

    #[test]
    fn an_oversized_message_drains_instead_of_being_unaffordable() {
        let mut b = Bucket::new(10, 10);
        assert!(b.take_weight(1_000_000.0), "a weight above capacity must still be payable");
        assert!(!b.take(), "and it must leave the bucket empty");
    }
}
