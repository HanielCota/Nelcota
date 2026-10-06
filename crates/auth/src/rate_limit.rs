//! Rate limit em memória, por chave (IP, email), em janela fixa de 1 minuto.
//!
//! Simples de propósito: um único binário por instalação dispensa Redis.

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

    /// `Ok` se ainda cabe na janela; `Err(espera)` caso contrário.
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
    fn bloqueia_acima_do_limite_e_libera_na_janela_seguinte() {
        let limiter = RateLimiter::new(2);
        let t0 = Instant::now();
        assert!(limiter.check_at("ip", t0).is_ok());
        assert!(limiter.check_at("ip", t0).is_ok());
        assert!(limiter.check_at("ip", t0).is_err());
        assert!(limiter.check_at("outro-ip", t0).is_ok());
        assert!(limiter.check_at("ip", t0 + WINDOW).is_ok());
    }
}
