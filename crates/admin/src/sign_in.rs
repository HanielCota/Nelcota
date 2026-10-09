//! How the app's users can sign in: a read-only summary of the auth settings.
//! They come from the environment (`NELCOTA_*`), so the panel shows what is
//! on and which variable turns the rest on, instead of a form that could not
//! save anything.
use crate::AdminState;
use axum::{Json, extract::State};
use serde_json::Value;

/// `GET /admin/api/sign-in`
pub async fn get(State(state): State<AdminState>) -> Json<Value> {
    Json(serde_json::to_value(&*state.sign_in).unwrap_or(Value::Null))
}
