//! Email and password rules for accounts. Shared by public signup
//! (`/auth/v1/signup`) and the panel, which creates users and resets
//! passwords: an account cannot follow different rules depending on who
//! created it.

/// Rejected credential; the message goes to whoever filled it in.
#[derive(Debug, PartialEq, Eq)]
pub struct InvalidCredential(pub &'static str);

/// Lowercase email with no surrounding spaces, minimally validated (the
/// database also requires lowercase and at most 254 characters).
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
        Err(InvalidCredential("invalid email"))
    }
}

pub fn validate_password(password: &str) -> Result<(), InvalidCredential> {
    match password.chars().count() {
        0..8 => Err(InvalidCredential(
            "the password needs at least 8 characters",
        )),
        8..=256 => Ok(()),
        _ => Err(InvalidCredential(
            "the password can have at most 256 characters",
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_email() {
        assert_eq!(
            normalize_email("  Ana@Example.COM ").unwrap(),
            "ana@example.com"
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
    fn password_length() {
        assert!(validate_password("1234567").is_err());
        assert!(validate_password("12345678").is_ok());
        // Counts characters, not bytes.
        assert!(validate_password("€€€€€€€€").is_ok());
        assert!(validate_password(&"x".repeat(257)).is_err());
    }
}
