//! Isolated crypto costs; no database or network latency is included.
use nelcota_auth::{JwtVerifier, Keys, Passwords, generate_ed25519_private_key};
use std::time::Instant;

fn main() {
    let keys = Keys::new(Some(&generate_ed25519_private_key()), None).unwrap();
    let token = keys
        .sign(&serde_json::json!({
            "role":"anon", "exp":jsonwebtoken::get_current_timestamp()+900
        }))
        .unwrap();
    let mut verification_us = Vec::with_capacity(20_000);
    for _ in 0..20_000 {
        let started = Instant::now();
        std::hint::black_box(keys.verify(std::hint::black_box(&token)).unwrap());
        verification_us.push(started.elapsed().as_secs_f64() * 1_000_000.0);
    }
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let hashing_ms = runtime.block_on(async {
        let workers = Passwords::new(2);
        let mut samples = Vec::with_capacity(30);
        for _ in 0..30 {
            let mut bytes = [0u8; 32];
            getrandom::fill(&mut bytes).unwrap();
            let password: String = bytes.iter().map(|value| format!("{value:02x}")).collect();
            let started = Instant::now();
            let hash = workers.hash(password).await.unwrap();
            assert!(hash.starts_with("$argon2id$v=19$m=19456,t=2,p=1$"));
            samples.push(started.elapsed().as_secs_f64() * 1000.0);
        }
        samples
    });
    println!(
        "{}",
        serde_json::json!({"verification_us":verification_us,"hashing_ms":hashing_ms,
        "hash_workers":2,"argon2":{"memory_kib":19456,"iterations":2,"lanes":1},"samples_include":"operation latency only"})
    );
}
