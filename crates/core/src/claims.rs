//! Claims of an already authenticated request and the roles the API accepts.
//!
//! The contract is in `docs/jwt-and-roles.md`. This module knows nothing about
//! signatures: the `nelcota-auth` crate validates the token.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

/// Postgres roles a request may assume. Any other value in the `role` claim
/// (e.g. `postgres`, `authenticator`) is rejected.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Anon,
    Authenticated,
    ServiceRole,
}

impl Role {
    /// Role name in Postgres. Comes from a closed list, never from the user.
    pub fn as_str(self) -> &'static str {
        match self {
            Role::Anon => "anon",
            Role::Authenticated => "authenticated",
            Role::ServiceRole => "service_role",
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum InvalidClaims {
    #[error("invalid claims: {0}")]
    Malformed(#[from] serde_json::Error),
    #[error("role authenticated requires the sub claim")]
    MissingSub,
}

/// Claims of a request: the role to assume, the user (if any) and the full
/// JSON, passed on to Postgres in `request.jwt.claims`.
#[derive(Clone, Debug)]
pub struct Claims {
    role: Role,
    sub: Option<Uuid>,
    payload: Value,
    json: String,
}

#[derive(Deserialize)]
struct Known {
    role: Role,
    #[serde(default)]
    sub: Option<Uuid>,
}

impl Claims {
    /// Claims of a request without a token.
    pub fn anon() -> Self {
        Claims {
            role: Role::Anon,
            sub: None,
            payload: serde_json::json!({ "role": "anon" }),
            json: r#"{"role":"anon"}"#.to_owned(),
        }
    }

    /// Interprets the payload of an already verified JWT.
    pub fn from_payload(payload: Value) -> Result<Self, InvalidClaims> {
        let known = Known::deserialize(&payload)?;
        if known.role == Role::Authenticated && known.sub.is_none() {
            return Err(InvalidClaims::MissingSub);
        }
        Ok(Claims {
            role: known.role,
            sub: known.sub,
            json: payload.to_string(),
            payload,
        })
    }

    pub fn role(&self) -> Role {
        self.role
    }

    pub fn sub(&self) -> Option<Uuid> {
        self.sub
    }

    /// Any claim of the payload (e.g. `session_id`, `email`).
    pub fn claim(&self, name: &str) -> Option<&Value> {
        self.payload.get(name)
    }

    /// JSON of the claims, as Postgres will see it in `auth.jwt()`.
    pub fn as_json(&self) -> &str {
        &self.json
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn accepts_known_roles() {
        let c = Claims::from_payload(json!({"role": "service_role"})).unwrap();
        assert_eq!(c.role(), Role::ServiceRole);
        assert_eq!(c.sub(), None);
    }

    #[test]
    fn rejects_roles_outside_the_list() {
        for role in ["postgres", "authenticator", "Anon", ""] {
            assert!(
                Claims::from_payload(json!({"role": role})).is_err(),
                "{role}"
            );
        }
        assert!(Claims::from_payload(json!({})).is_err());
    }

    #[test]
    fn authenticated_requires_uuid_sub() {
        assert!(matches!(
            Claims::from_payload(json!({"role": "authenticated"})),
            Err(InvalidClaims::MissingSub)
        ));
        assert!(Claims::from_payload(json!({"role": "authenticated", "sub": "abc"})).is_err());
        let sub = "7f9c24e8-3b12-4fef-91e0-3c7a5e4b9c1d";
        let c = Claims::from_payload(json!({"role": "authenticated", "sub": sub})).unwrap();
        assert_eq!(c.sub().unwrap().to_string(), sub);
    }
}
