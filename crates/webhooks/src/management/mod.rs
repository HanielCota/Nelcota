//! Administrator-only configuration and delivery inspection. Capture DDL,
//! validation and history each have one owner; no HTTP requests happen here.
mod config;
mod history;
pub use config::{create, delete, list, rotate, tables, update};
pub use history::{clear_history, detail, history, replay};

use crate::{
    Error, invalid,
    models::{Delivery, Endpoint},
};
use tokio_postgres::Row;
use uuid::Uuid;

const ENDPOINTS: &str = include_str!("endpoints.sql");
const DELIVERIES: &str = include_str!("deliveries.sql");

fn endpoint(row: &Row) -> Endpoint {
    Endpoint {
        id: row.get("id"),
        name: row.get("name"),
        schema: row.get("schema_name"),
        table: row.get("table_name"),
        table_available: row.get("table_available"),
        url: row.get("url"),
        events: row.get("events"),
        enabled: row.get("enabled"),
        max_attempts: row.get("max_attempts"),
        created_at: row.get("created_at"),
    }
}
fn delivery(row: &Row) -> Delivery {
    Delivery {
        id: row.get("id"),
        endpoint_id: row.get("endpoint_id"),
        endpoint_name: row.get("endpoint_name"),
        endpoint_enabled: row.get("endpoint_enabled"),
        event_id: row.get("event_id"),
        replay_of: row.get("replay_of"),
        schema: row.get("schema_name"),
        table: row.get("table_name"),
        event: row.get("event"),
        url: row.get("url"),
        status: row.get("status"),
        attempts: row.get("attempts"),
        max_attempts: row.get("max_attempts"),
        created_at: row.get("created_at"),
        next_attempt_at: row.get("next_attempt_at"),
        finished_at: row.get("finished_at"),
        last_status: row.get("last_status"),
        last_error: row.get("last_error"),
    }
}
fn uuid(value: &str) -> Result<Uuid, Error> {
    Uuid::parse_str(value).map_err(|_| invalid("invalid_id", "Invalid id"))
}
fn delivery_id(value: &str) -> Result<i64, Error> {
    value
        .parse::<i64>()
        .ok()
        .filter(|v| *v > 0)
        .ok_or_else(|| invalid("invalid_id", "Invalid id"))
}
