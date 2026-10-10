//! The API requests refused recently (401, 403, 429), kept in memory for the
//! panel's "Recently blocked" list. It answers "why does my app get 403?"
//! without a log pipeline: a bounded ring buffer, lost on restart, never
//! written to the database. Only the path is kept, not the query string
//! (filters may carry personal data), and never a token.
use crate::{AdminState, contracts::DeniedRequest};
use axum::{Json, extract::State};
use std::{collections::VecDeque, sync::Mutex};

/// How many refusals are kept.
pub const CAPACITY: usize = 100;

#[derive(Default)]
pub struct DeniedLog {
    entries: Mutex<VecDeque<DeniedRequest>>,
}

impl DeniedLog {
    pub fn record(&self, entry: DeniedRequest) {
        let mut entries = self.entries.lock().unwrap_or_else(|e| e.into_inner());
        if entries.len() == CAPACITY {
            entries.pop_back();
        }
        entries.push_front(entry);
    }

    /// Newest first.
    pub fn recent(&self) -> Vec<DeniedRequest> {
        let entries = self.entries.lock().unwrap_or_else(|e| e.into_inner());
        entries
            .iter()
            .map(|e| DeniedRequest {
                at: e.at.clone(),
                method: e.method.clone(),
                path: e.path.clone(),
                status: e.status,
                code: e.code.clone(),
                message: e.message.clone(),
                role: e.role.clone(),
                user_id: e.user_id.clone(),
                email: e.email.clone(),
            })
            .collect()
    }
}

/// The current UTC time as RFC 3339 (`2026-10-09T14:03:07Z`).
pub fn now_rfc3339() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    rfc3339(secs)
}

/// Seconds since the epoch as RFC 3339, UTC (days to civil date, Howard
/// Hinnant's algorithm).
pub(crate) fn rfc3339(secs: u64) -> String {
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    )
}

/// `GET /admin/api/denied`
pub async fn list(State(state): State<AdminState>) -> Json<crate::contracts::DeniedRequests> {
    Json(crate::contracts::DeniedRequests {
        capacity: CAPACITY,
        requests: state.denied.recent(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(path: &str) -> DeniedRequest {
        DeniedRequest {
            at: "2026-10-09T00:00:00Z".into(),
            method: "GET".into(),
            path: path.into(),
            status: 401,
            code: "db_error".into(),
            message: "permission denied".into(),
            role: "anon".into(),
            user_id: None,
            email: None,
        }
    }

    #[test]
    fn formats_utc_timestamps() {
        assert_eq!(rfc3339(0), "1970-01-01T00:00:00Z");
        assert_eq!(rfc3339(951_782_400), "2000-02-29T00:00:00Z");
        assert_eq!(rfc3339(1_791_557_012), "2026-10-09T14:43:32Z");
    }

    #[test]
    fn keeps_the_newest_up_to_capacity() {
        let log = DeniedLog::default();
        for i in 0..CAPACITY + 5 {
            log.record(entry(&format!("/rest/v1/t{i}")));
        }
        let recent = log.recent();
        assert_eq!(recent.len(), CAPACITY);
        assert_eq!(recent[0].path, format!("/rest/v1/t{}", CAPACITY + 4));
        assert_eq!(recent[CAPACITY - 1].path, "/rest/v1/t5");
    }
}
