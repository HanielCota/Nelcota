//! Password hashing with argon2id (RustCrypto defaults, which follow the OWASP
//! recommendation: m=19 MiB, t=2, p=1), in PHC format.
//!
//! The computation is expensive on purpose: it runs in `spawn_blocking` with a
//! cap on concurrent hashes, to fit a 1 GB VPS without stalling the runtime.

use argon2::{
    Argon2,
    password_hash::{PasswordHasher, PasswordVerifier, phc::PasswordHash},
};
use std::sync::Arc;
use tokio::sync::Semaphore;

pub struct Passwords {
    permits: Arc<Semaphore>,
    /// Hash of a random password: a login for an unknown email verifies against
    /// it, so the response time does not reveal whether the email exists.
    dummy: String,
}

impl Passwords {
    pub fn new(max_concurrent: usize) -> Self {
        let mut random = [0u8; 32];
        getrandom::fill(&mut random).expect("system randomness source unavailable");
        Passwords {
            permits: Arc::new(Semaphore::new(max_concurrent.max(1))),
            dummy: hash_blocking(&random).expect("argon2 with default parameters cannot fail"),
        }
    }

    pub async fn hash(&self, password: String) -> Option<String> {
        self.compute(move || hash_blocking(password.as_bytes()))
            .await?
    }

    /// The worker owns its permit, including after its caller is cancelled.
    async fn compute<T: Send + 'static>(
        &self,
        work: impl FnOnce() -> T + Send + 'static,
    ) -> Option<T> {
        let permit = self.permits.clone().acquire_owned().await.ok()?;
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            work()
        })
        .await
        .ok()
    }

    /// `phc = None` (unknown user or no password) always fails, but takes the
    /// same time as a real verification.
    pub async fn verify(&self, password: String, phc: Option<String>) -> bool {
        let exists = phc.is_some();
        let phc = phc.unwrap_or_else(|| self.dummy.clone());
        let ok = self
            .compute(move || {
                PasswordHash::new(&phc).is_ok_and(|parsed| {
                    Argon2::default()
                        .verify_password(password.as_bytes(), &parsed)
                        .is_ok()
                })
            })
            .await
            .unwrap_or(false);
        ok && exists
    }
}

/// Synchronous hash (for the CLI, outside the server).
pub fn hash_password(password: &str) -> Option<String> {
    hash_blocking(password.as_bytes())
}

/// Synchronous verification for the CLI. Server requests use [`Passwords`].
pub fn verify_password(password: &str, phc: &str) -> bool {
    PasswordHash::new(phc).is_ok_and(|parsed| {
        Argon2::default()
            .verify_password(password.as_bytes(), &parsed)
            .is_ok()
    })
}

fn hash_blocking(password: &[u8]) -> Option<String> {
    Argon2::default()
        .hash_password(password)
        .ok()
        .map(|hash| hash.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn cancelled_caller_keeps_the_worker_permit_until_completion() {
        let passwords = Arc::new(Passwords::new(1));
        let (started, running) = tokio::sync::oneshot::channel();
        let (release, wait) = std::sync::mpsc::channel();
        let worker = passwords.clone();
        let caller = tokio::spawn(async move {
            worker
                .compute(move || {
                    started.send(()).unwrap();
                    wait.recv().unwrap();
                })
                .await
        });
        running.await.unwrap();
        caller.abort();
        let _ = caller.await;
        assert!(passwords.permits.try_acquire().is_err());
        release.send(()).unwrap();
        let permit = tokio::time::timeout(
            std::time::Duration::from_secs(2),
            passwords.permits.acquire(),
        )
        .await
        .unwrap()
        .unwrap();
        drop(permit);
    }

    #[tokio::test]
    async fn phc_argon2id_hash_and_verification() {
        let passwords = Passwords::new(2);
        let hash = passwords.hash("strong-password-123".into()).await.unwrap();
        assert!(
            hash.starts_with("$argon2id$v=19$m=19456,t=2,p=1$"),
            "{hash}"
        );
        assert!(
            passwords
                .verify("strong-password-123".into(), Some(hash.clone()))
                .await
        );
        assert!(!passwords.verify("wrong-password".into(), Some(hash)).await);
        assert!(!passwords.verify("anything".into(), None).await);
    }
}
