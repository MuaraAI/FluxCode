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

    /// Refill what time has added, then spend one token if there is one.
    pub fn take(&mut self) -> bool {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last).as_secs_f64();
        self.last = now;
        self.tokens = (self.tokens + elapsed * self.per_sec).min(self.capacity);
        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            true
        } else {
            false
        }
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
}
