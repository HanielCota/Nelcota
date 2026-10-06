//! Hash de senhas com argon2id (parâmetros padrão do RustCrypto, que seguem a
//! recomendação da OWASP: m=19 MiB, t=2, p=1), no formato PHC.
//!
//! O cálculo é caro de propósito: roda em `spawn_blocking` e com um limite de
//! hashes simultâneos, para caber em VPS de 1 GB sem travar o runtime.

use argon2::{
    Argon2,
    password_hash::{PasswordHasher, PasswordVerifier, phc::PasswordHash},
};
use tokio::sync::Semaphore;

pub struct Passwords {
    permits: Semaphore,
    /// Hash de uma senha aleatória: login de email inexistente verifica contra
    /// ele, para o tempo de resposta não revelar se o email existe.
    dummy: String,
}

impl Passwords {
    pub fn new(max_concurrent: usize) -> Self {
        let mut random = [0u8; 32];
        getrandom::fill(&mut random).expect("fonte de aleatoriedade do sistema indisponível");
        Passwords {
            permits: Semaphore::new(max_concurrent.max(1)),
            dummy: hash_blocking(&random).expect("argon2 com parâmetros padrão não falha"),
        }
    }

    pub async fn hash(&self, password: String) -> Option<String> {
        let _permit = self.permits.acquire().await.ok()?;
        tokio::task::spawn_blocking(move || hash_blocking(password.as_bytes()))
            .await
            .ok()?
    }

    /// `phc = None` (usuário inexistente ou sem senha) sempre falha, mas gasta
    /// o mesmo tempo de uma verificação real.
    pub async fn verify(&self, password: String, phc: Option<String>) -> bool {
        let Ok(_permit) = self.permits.acquire().await else {
            return false;
        };
        let exists = phc.is_some();
        let phc = phc.unwrap_or_else(|| self.dummy.clone());
        let ok = tokio::task::spawn_blocking(move || {
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
    async fn hash_phc_argon2id_e_verificacao() {
        let passwords = Passwords::new(2);
        let hash = passwords.hash("senha-forte-123".into()).await.unwrap();
        assert!(
            hash.starts_with("$argon2id$v=19$m=19456,t=2,p=1$"),
            "{hash}"
        );
        assert!(
            passwords
                .verify("senha-forte-123".into(), Some(hash.clone()))
                .await
        );
        assert!(!passwords.verify("senha-errada".into(), Some(hash)).await);
        assert!(!passwords.verify("qualquer".into(), None).await);
    }
}
