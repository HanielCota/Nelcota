//! In-memory rate limit per key (IP, email): `limit` requests per minute.
//!
//! Uses governor's GCRA: the budget refills continuously (one request every
//! `60 / limit` seconds) instead of resetting at the turn of a fixed minute,
//! which let a client spend almost twice the limit across the boundary.
//! Still in memory: a single binary per install needs no Redis (D26).

use crate::AuthState;
use nelcota_core::ApiError;

use std::{num::NonZeroU32, time::Duration};

use governor::{
    Quota,
    clock::{Clock, DefaultClock},
    middleware::NoOpMiddleware,
    state::keyed::DefaultKeyedStateStore,
};

/// Above this many tracked keys, keys idle for a full refill are dropped.
const MAX_KEYS: usize = 50_000;

type Keyed<C> = governor::RateLimiter<
    String,
    DefaultKeyedStateStore<String>,
    C,
    NoOpMiddleware<<C as Clock>::Instant>,
>;

/// Generic over the clock only so tests can drive time by hand.
pub struct RateLimiter<C: Clock + Clone = DefaultClock> {
    inner: Keyed<C>,
    clock: C,
}

impl RateLimiter {
    pub fn new(limit_per_minute: u32) -> Self {
        Self::with_clock(limit_per_minute, DefaultClock::default())
    }
}

impl<C: Clock + Clone> RateLimiter<C> {
    fn with_clock(limit_per_minute: u32, clock: C) -> Self {
        let limit = NonZeroU32::new(limit_per_minute).unwrap_or(NonZeroU32::MIN);
        let inner = governor::RateLimiter::new(
            Quota::per_minute(limit),
            DefaultKeyedStateStore::default(),
            clock.clone(),
        );
        RateLimiter { inner, clock }
    }

    /// `Ok` while the key has budget; `Err(wait)` with the time until it has again.
    pub fn check(&self, key: &str) -> Result<(), Duration> {
        if self.inner.len() >= MAX_KEYS {
            self.inner.retain_recent();
        }
        self.inner
            .check_key(&key.to_owned())
            .map_err(|not_until| not_until.wait_time_from(self.clock.now()))
    }
}

pub(crate) fn limit(state: &AuthState, key: &str) -> Result<(), ApiError> {
    state
        .limiter
        .check(key)
        .map_err(|wait| ApiError::rate_limited(wait.as_secs()))
}

#[cfg(test)]
mod tests {
    use governor::clock::FakeRelativeClock;

    use super::*;

    fn limiter(limit: u32) -> (RateLimiter<FakeRelativeClock>, FakeRelativeClock) {
        let clock = FakeRelativeClock::default();
        (RateLimiter::with_clock(limit, clock.clone()), clock)
    }

    #[test]
    fn blocks_above_the_limit_and_keys_are_independent() {
        let (limiter, _) = limiter(2);
        assert!(limiter.check("ip").is_ok());
        assert!(limiter.check("ip").is_ok());
        let wait = limiter.check("ip").unwrap_err();
        // 2 per minute: one more request is allowed 30 s later.
        assert_eq!(wait, Duration::from_secs(30));
        assert!(limiter.check("other-ip").is_ok());
    }

    #[test]
    fn budget_refills_gradually() {
        let (limiter, clock) = limiter(2);
        assert!(limiter.check("ip").is_ok());
        assert!(limiter.check("ip").is_ok());
        clock.advance(Duration::from_secs(30));
        assert!(limiter.check("ip").is_ok());
        assert!(limiter.check("ip").is_err());
        clock.advance(Duration::from_secs(60));
        assert!(limiter.check("ip").is_ok());
        assert!(limiter.check("ip").is_ok());
        assert!(limiter.check("ip").is_err());
    }

    #[test]
    fn no_double_budget_across_a_minute_boundary() {
        // The old fixed window allowed `limit` at 0:59 and `limit` again at 1:00.
        let (limiter, clock) = limiter(10);
        clock.advance(Duration::from_secs(59));
        for _ in 0..10 {
            assert!(limiter.check("ip").is_ok());
        }
        clock.advance(Duration::from_secs(1));
        assert!(
            limiter.check("ip").is_err(),
            "only 1 s of refill, not a fresh minute"
        );
    }

    #[test]
    fn zero_limit_still_allows_one_per_minute() {
        let (limiter, _) = limiter(0);
        assert!(limiter.check("ip").is_ok());
        assert!(limiter.check("ip").is_err());
    }
}
