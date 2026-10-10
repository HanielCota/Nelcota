//! `cargo bench -p nelcota-auth --bench rate_limit`: fresh-key admission at
//! 1,000 and 50,000 tracked keys, against the actual public limiter.
use nelcota_auth::RateLimiter;
use std::{hint::black_box, time::Instant};

fn measure(keys: usize) {
    let limiter = RateLimiter::new(30);
    for i in 0..keys {
        assert!(limiter.check(&format!("seed:{i}")).is_ok());
    }
    let mut samples = Vec::new();
    let mut accepted = 0;
    for sample in 0..3 {
        let names: Vec<_> = (0..100).map(|i| format!("new:{sample}:{i}")).collect();
        let start = Instant::now();
        for key in &names {
            accepted += usize::from(black_box(limiter.check(black_box(key))).is_ok());
        }
        samples.push(start.elapsed());
    }
    samples.sort();
    assert_eq!(accepted, if keys == 50_000 { 0 } else { 300 });
    println!(
        "keys={keys}, median_100_checks={:?}, accepted_new_keys={accepted}",
        samples[1]
    );
}

fn main() {
    measure(1_000);
    measure(50_000);
}
