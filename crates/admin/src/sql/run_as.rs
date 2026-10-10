//! Who a SQL editor run (or a table view) acts as: the database owner
//! (default, no RLS), a visitor (`anon`) or a real signed-in user
//! (`authenticated` with their id), so the access rules can be tried.
use crate::ApiError;
use deadpool_postgres::Pool;
use nelcota_core::Claims;
use serde::Deserialize;
use serde_json::json;

#[derive(Debug, Default, Deserialize)]
#[serde(tag = "role", rename_all = "snake_case")]
pub enum RunAs {
    #[default]
    Owner,
    Anon,
    Authenticated {
        user_id: String,
    },
}

impl RunAs {
    /// The request claims the run assumes, as the API sets them; `None` keeps
    /// the owner. A signed-in user must exist: their id and email are what
    /// `auth.uid()` and `auth.jwt()` return.
    pub(crate) async fn claims(self, pool: &Pool) -> Result<Option<Claims>, ApiError> {
        match self {
            RunAs::Owner => Ok(None),
            RunAs::Anon => Ok(Some(Claims::anon())),
            RunAs::Authenticated { user_id } => {
                let client = pool.get().await?;
                // A malformed id matches no user instead of failing the cast;
                // a well-formed one still uses the primary key.
                let row = client
                    .query_opt(
                        "SELECT id::text, email FROM auth.users
                          WHERE id = CASE WHEN $1 ~* '^[0-9a-f]{8}-([0-9a-f]{4}-){3}[0-9a-f]{12}$'
                                          THEN $1::uuid END",
                        &[&user_id],
                    )
                    .await?
                    .ok_or_else(|| ApiError::not_found("user_not_found", "user not found"))?;
                let (id, email): (String, String) = (row.get(0), row.get(1));
                let claims = Claims::from_payload(json!({
                    "role": "authenticated",
                    "aud": "authenticated",
                    "sub": id,
                    "email": email,
                }))
                .map_err(|err| ApiError::from(err.to_string()))?;
                Ok(Some(claims))
            }
        }
    }
}
