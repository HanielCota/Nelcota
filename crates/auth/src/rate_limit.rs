//! In-memory rate limit per key (IP, email), in a fixed 1-minute window.
//!
//! Simple on purpose: a single binary per install needs no Redis.

use std::{
    collections::HashMap,
    sync::Mutex,
    time::{Duration, Instant},
};

const WINDOW: Duration = Duration::from_secs(60);
const MAX_KEYS: usize = 50_000;

pub struct RateLimiter {
    limit: u32,
    buckets: Mutex<HashMap<String, (Instant, u32)>>,
}

impl RateLimiter {
    pub fn new(limit_per_minute: u32) -> Self {
        RateLimiter {
            limit: limit_per_minute.max(1),
            buckets: Mutex::new(HashMap::new()),
        }
    }

    /// `Ok` while the window has room; `Err(wait)` otherwise.
    pub fn check(&self, key: &str) -> Result<(), Duration> {
        self.check_at(key, Instant::now())
    }

    fn check_at(&self, key: &str, now: Instant) -> Result<(), Duration> {
        let mut buckets = self.buckets.lock().unwrap_or_else(|e| e.into_inner());
        if buckets.len() >= MAX_KEYS {
            buckets.retain(|_, (start, _)| now.duration_since(*start) < WINDOW);
        }
        let (start, count) = buckets.entry(key.to_owned()).or_insert((now, 0));
        if now.duration_since(*start) >= WINDOW {
            *start = now;
            *count = 0;
        }
        if *count >= self.limit {
            return Err(WINDOW.saturating_sub(now.duration_since(*start)));
        }
        *count += 1;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocks_above_the_limit_and_frees_in_the_next_window() {
        let limiter = RateLimiter::new(2);
        let t0 = Instant::now();
        assert!(limiter.check_at("ip", t0).is_ok());
        assert!(limiter.check_at("ip", t0).is_ok());
        assert!(limiter.check_at("ip", t0).is_err());
        assert!(limiter.check_at("other-ip", t0).is_ok());
        assert!(limiter.check_at("ip", t0 + WINDOW).is_ok());
    }
}
