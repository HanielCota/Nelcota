//! In-memory rate limit per key (IP, email): `limit` requests per minute.
//!
//! Uses governor's GCRA: the budget refills continuously (one request every
//! `60 / limit` seconds) instead of resetting at the turn of a fixed minute,
//! which let a client spend almost twice the limit across the boundary.
//! Still in memory: a single binary per install needs no Redis (D26).

use crate::AuthState;
use nelcota_core::ApiError;

use std::{
    collections::HashMap,
    num::NonZeroU32,
    sync::{
        RwLock,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};

use governor::{
    Quota,
    clock::{Clock, DefaultClock, Reference},
    middleware::NoOpMiddleware,
    state::{InMemoryState, NotKeyed},
};

/// Refreshes and PKCE redemptions get this many times the per-IP budget of
/// password sign-in (`NELCOTA_AUTH_RATE_LIMIT_PER_MINUTE`).
pub const SESSION_RATE_LIMIT_FACTOR: u32 = 10;

/// Capacity is enforced without evicting a key's active rate limit.
const MAX_KEYS: usize = 50_000;
const CLEANUP_INTERVAL: Duration = Duration::from_secs(1);
const FULL_REFILL: Duration = Duration::from_secs(60);

type Direct<C> =
    governor::RateLimiter<NotKeyed, InMemoryState, C, NoOpMiddleware<<C as Clock>::Instant>>;

struct Entry<C: Clock> {
    limiter: Direct<C>,
    last_accepted: AtomicU64,
}

struct State<C: Clock> {
    entries: HashMap<String, Entry<C>>,
    last_cleanup: C::Instant,
    #[cfg(test)]
    cleanups: usize,
}

/// Generic over the clock only so tests can drive time by hand.
pub struct RateLimiter<C: Clock + Clone = DefaultClock> {
    state: RwLock<State<C>>,
    quota: Quota,
    clock: C,
    start: C::Instant,
    capacity: usize,
}

impl RateLimiter {
    pub fn new(limit_per_minute: u32) -> Self {
        Self::with_clock(limit_per_minute, DefaultClock::default())
    }
}

impl<C: Clock + Clone> RateLimiter<C> {
    fn with_clock(limit_per_minute: u32, clock: C) -> Self {
        Self::with_capacity(limit_per_minute, clock, MAX_KEYS)
    }

    fn with_capacity(limit_per_minute: u32, clock: C, capacity: usize) -> Self {
        let limit = NonZeroU32::new(limit_per_minute).unwrap_or(NonZeroU32::MIN);
        let start = clock.now();
        RateLimiter {
            state: RwLock::new(State {
                entries: HashMap::new(),
                last_cleanup: start,
                #[cfg(test)]
                cleanups: 0,
            }),
            quota: Quota::per_minute(limit),
            clock,
            start,
            capacity,
        }
    }

    fn check_entry(&self, entry: &Entry<C>) -> Result<(), Duration> {
        entry
            .limiter
            .check()
            .map_err(|not_until| not_until.wait_time_from(self.clock.now()))?;
        entry.last_accepted.fetch_max(
            self.clock.now().duration_since(self.start).as_u64(),
            Ordering::Relaxed,
        );
        Ok(())
    }

    /// `Ok` while the key has budget; `Err(wait)` until its budget refills.
    /// Unknown keys are also refused while capacity is full, with a retry hint.
    pub fn check(&self, key: &str) -> Result<(), Duration> {
        {
            let state = self.state.read().unwrap_or_else(|e| e.into_inner());
            if let Some(entry) = state.entries.get(key) {
                return self.check_entry(entry);
            }
        }

        // Serialize new-key admission only. Existing keys share a read lock
        // and governor still atomically enforces each independent GCRA budget.
        let mut state = self.state.write().unwrap_or_else(|e| e.into_inner());
        if let Some(entry) = state.entries.get(key) {
            return self.check_entry(entry);
        }
        let now = self.clock.now();
        if state.entries.len() >= self.capacity
            && now.duration_since(state.last_cleanup) >= CLEANUP_INTERVAL.into()
        {
            let elapsed = now.duration_since(self.start).as_u64();
            state.entries.retain(|_, entry| {
                elapsed.saturating_sub(entry.last_accepted.load(Ordering::Relaxed))
                    < FULL_REFILL.as_nanos() as u64
            });
            state.last_cleanup = now;
            #[cfg(test)]
            {
                state.cleanups += 1;
            }
        }
        // Fail closed for unknown keys while full. Dropping a recent entry
        // would let its next attempt obtain a fresh burst and bypass the quota.
        if state.entries.len() >= self.capacity {
            return Err(CLEANUP_INTERVAL);
        }
        let entry = state
            .entries
            .entry(key.to_owned())
            .or_insert_with(|| Entry {
                limiter: governor::RateLimiter::direct_with_clock(self.quota, self.clock.clone()),
                last_accepted: AtomicU64::new(now.duration_since(self.start).as_u64()),
            });
        self.check_entry(entry)
    }
}

pub(crate) fn limit(state: &AuthState, key: &str) -> Result<(), ApiError> {
    state
        .limiter
        .check(key)
        .map_err(|wait| ApiError::rate_limited(wait.as_secs()))
}

/// The budget of refreshes and PKCE redemptions (`session_limiter`).
pub(crate) fn limit_sessions(state: &AuthState, key: &str) -> Result<(), ApiError> {
    state
        .session_limiter
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

    #[test]
    fn high_cardinality_is_bounded_without_resetting_existing_budget() {
        let (limiter, _) = limiter(1);
        for i in 0..MAX_KEYS {
            assert!(limiter.check(&format!("key:{i}")).is_ok());
        }
        for i in 0..300 {
            assert!(
                limiter.check(&format!("new:{i}")).is_err(),
                "capacity is full"
            );
        }
        assert_eq!(limiter.check("key:0"), Err(Duration::from_secs(60)));
        let state = limiter.state.read().unwrap();
        assert_eq!(state.entries.len(), MAX_KEYS);
        assert_eq!(
            state.cleanups, 0,
            "a frozen clock must not trigger repeated scans"
        );
    }

    #[test]
    fn cleanup_is_amortized_and_only_drops_fully_refilled_keys() {
        let clock = FakeRelativeClock::default();
        let limiter = RateLimiter::with_capacity(1, clock.clone(), 2);
        assert!(limiter.check("a").is_ok());
        assert!(limiter.check("b").is_ok());
        clock.advance(Duration::from_secs(1));
        for i in 0..100 {
            assert!(limiter.check(&format!("new:{i}")).is_err());
        }
        assert_eq!(limiter.state.read().unwrap().cleanups, 1);
        assert_eq!(limiter.check("a"), Err(Duration::from_secs(59)));
        clock.advance(Duration::from_secs(59));
        assert!(limiter.check("new").is_ok());
        assert_eq!(limiter.state.read().unwrap().entries.len(), 1);
        assert_eq!(limiter.state.read().unwrap().cleanups, 2);
        // The evicted key had fully recovered, so its normal budget is valid.
        assert!(limiter.check("a").is_ok());
        assert_eq!(limiter.check("a"), Err(Duration::from_secs(60)));
    }

    #[test]
    fn active_keys_keep_their_budget_during_capacity_pressure() {
        let clock = FakeRelativeClock::default();
        let limiter = RateLimiter::with_capacity(2, clock.clone(), 2);
        assert!(limiter.check("account").is_ok());
        assert!(limiter.check("account").is_ok());
        assert!(limiter.check("ip").is_ok());
        clock.advance(Duration::from_secs(30));
        assert!(limiter.check("account").is_ok());
        assert!(limiter.check("ip").is_ok());
        clock.advance(Duration::from_secs(30));
        assert!(
            limiter.check("new").is_err(),
            "both tracked keys still have state"
        );
        assert!(limiter.check("account").is_ok());
        assert!(
            limiter.check("account").is_err(),
            "cleanup must not restore its full burst"
        );
        assert!(
            limiter.check("ip").is_ok(),
            "other keys retain independent budgets"
        );
    }

    #[test]
    fn concurrent_admission_never_exceeds_capacity() {
        let limiter = std::sync::Arc::new(RateLimiter::with_capacity(
            1,
            FakeRelativeClock::default(),
            8,
        ));
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(32));
        let tasks: Vec<_> = (0..32)
            .map(|i| {
                let limiter = limiter.clone();
                let barrier = barrier.clone();
                std::thread::spawn(move || {
                    barrier.wait();
                    limiter.check(&format!("key:{i}")).is_ok()
                })
            })
            .collect();
        assert_eq!(
            tasks
                .into_iter()
                .map(|task| task.join().unwrap())
                .filter(|accepted| *accepted)
                .count(),
            8
        );
        assert_eq!(limiter.state.read().unwrap().entries.len(), 8);
    }
}
