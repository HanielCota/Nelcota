//! Regras de email e senha das contas. Compartilhadas pelo cadastro público
//! (`/auth/v1/signup`) e pelo painel, que cria usuários e redefine senhas:
//! a mesma conta não pode ter regras diferentes conforme quem a criou.

/// Credencial recusada; a mensagem vai para quem preencheu.
#[derive(Debug, PartialEq, Eq)]
pub struct InvalidCredential(pub &'static str);

/// Email em minúsculas e sem espaços nas pontas, com validação mínima
/// (o banco também exige minúsculas e no máximo 254 caracteres).
pub fn normalize_email(email: &str) -> Result<String, InvalidCredential> {
    let email = email.trim().to_lowercase();
    let valid = email.len() <= 254
        && !email.chars().any(char::is_whitespace)
        && email.split_once('@').is_some_and(|(local, domain)| {
            !local.is_empty() && domain.contains('.') && !domain.contains('@')
        });
    if valid {
        Ok(email)
    } else {
        Err(InvalidCredential("email inválido"))
    }
}

pub fn validate_password(password: &str) -> Result<(), InvalidCredential> {
    match password.chars().count() {
        0..8 => Err(InvalidCredential(
            "a senha precisa de ao menos 8 caracteres",
        )),
        8..=256 => Ok(()),
        _ => Err(InvalidCredential(
            "a senha pode ter no máximo 256 caracteres",
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normaliza_email() {
        assert_eq!(
            normalize_email("  Ana@Exemplo.COM ").unwrap(),
            "ana@exemplo.com"
        );
        for bad in [
            "",
            "ana",
            "@x.com",
            "ana@",
            "ana@x",
            "a b@x.com",
            "a@b@x.com",
        ] {
            assert!(normalize_email(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn tamanho_da_senha() {
        assert!(validate_password("1234567").is_err());
        assert!(validate_password("12345678").is_ok());
        // Conta caracteres, não bytes.
        assert!(validate_password("çççççççç").is_ok());
        assert!(validate_password(&"x".repeat(257)).is_err());
    }
}
