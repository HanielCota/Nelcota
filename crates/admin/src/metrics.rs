//! API traffic per minute for the overview's charts: requests, refusals,
//! server errors and a latency histogram, for the last 24 hours. Kept in
//! memory like the denied log (D97): no write per request, nothing to purge,
//! one series per process (one panel per project). Lost on restart, which the
//! overview says.
use crate::{AdminState, ApiError, denied::rfc3339};
use axum::{
    Json,
    extract::{Query, State},
};
use serde::Deserialize;
use std::{
    collections::VecDeque,
    sync::Mutex,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

/// Minutes kept: 24 hours.
pub const RETENTION_MINUTES: u64 = 24 * 60;

/// Upper bounds (ms) of the latency buckets; the last one is open-ended.
const BUCKETS_MS: [u32; 11] = [5, 10, 25, 50, 100, 250, 500, 1_000, 2_500, 5_000, 10_000];

/// Which part of the API a request hit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Area {
    Rest,
    Auth,
    Storage,
    Other,
}

impl Area {
    pub fn of(path: &str) -> Self {
        if path.starts_with("/rest/") {
            Area::Rest
        } else if path.starts_with("/auth/") {
            Area::Auth
        } else if path.starts_with("/storage/") {
            Area::Storage
        } else {
            Area::Other
        }
    }
}

#[derive(Debug, Clone, Default)]
struct Minute {
    /// Unix time / 60.
    index: u64,
    requests: u32,
    refused: u32,
    errors: u32,
    rest: u32,
    auth: u32,
    storage: u32,
    latency: [u32; BUCKETS_MS.len() + 1],
}

impl Minute {
    fn add(&mut self, other: &Minute) {
        self.requests += other.requests;
        self.refused += other.refused;
        self.errors += other.errors;
        self.rest += other.rest;
        self.auth += other.auth;
        self.storage += other.storage;
        for (a, b) in self.latency.iter_mut().zip(other.latency) {
            *a += b;
        }
    }

    /// The bucket bound under which 95% of the requests finished.
    fn p95_ms(&self) -> Option<u32> {
        let total: u32 = self.latency.iter().sum();
        if total == 0 {
            return None;
        }
        let target = (u64::from(total) * 95).div_ceil(100);
        let mut seen = 0u64;
        for (i, count) in self.latency.iter().enumerate() {
            seen += u64::from(*count);
            if seen >= target {
                // The open-ended bucket reports its lower bound.
                return Some(
                    BUCKETS_MS
                        .get(i)
                        .copied()
                        .unwrap_or(BUCKETS_MS[BUCKETS_MS.len() - 1]),
                );
            }
        }
        None
    }
}

pub struct Metrics {
    started: u64,
    minutes: Mutex<VecDeque<Minute>>,
}

impl Default for Metrics {
    fn default() -> Self {
        Metrics {
            started: now_secs(),
            minutes: Mutex::default(),
        }
    }
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

impl Metrics {
    /// Counts one finished request.
    pub fn record(&self, area: Area, status: u16, elapsed: Duration) {
        self.record_at(now_secs() / 60, area, status, elapsed);
    }

    fn record_at(&self, index: u64, area: Area, status: u16, elapsed: Duration) {
        let mut minutes = self.minutes.lock().unwrap_or_else(|e| e.into_inner());
        if minutes.back().is_none_or(|m| m.index != index) {
            minutes.push_back(Minute {
                index,
                ..Minute::default()
            });
        }
        while minutes
            .front()
            .is_some_and(|m| m.index + RETENTION_MINUTES <= index)
        {
            minutes.pop_front();
        }
        let minute = minutes.back_mut().expect("pushed above");
        minute.requests += 1;
        match status {
            401 | 403 | 429 => minute.refused += 1,
            500..=599 => minute.errors += 1,
            _ => {}
        }
        match area {
            Area::Rest => minute.rest += 1,
            Area::Auth => minute.auth += 1,
            Area::Storage => minute.storage += 1,
            Area::Other => {}
        }
        let ms = u32::try_from(elapsed.as_millis()).unwrap_or(u32::MAX);
        let bucket = BUCKETS_MS
            .iter()
            .position(|bound| ms < *bound)
            .unwrap_or(BUCKETS_MS.len());
        minute.latency[bucket] += 1;
    }

    /// `points` buckets of `step` minutes ending at the current minute,
    /// zeros where nothing happened.
    fn series(&self, now: u64, step: u64, points: u64) -> (Vec<(u64, Minute)>, Minute) {
        let minutes = self.minutes.lock().unwrap_or_else(|e| e.into_inner());
        let end = now / 60;
        let first = end + 1 - step * points;
        let mut out: Vec<(u64, Minute)> = (0..points)
            .map(|i| (first + i * step, Minute::default()))
            .collect();
        let mut total = Minute::default();
        for minute in minutes
            .iter()
            .filter(|m| m.index >= first && m.index <= end)
        {
            let slot = ((minute.index - first) / step) as usize;
            out[slot].1.add(minute);
            total.add(minute);
        }
        (out, total)
    }
}

#[derive(Deserialize)]
pub struct MetricsQuery {
    range: Option<String>,
}

/// `GET /admin/api/metrics?range=1h|24h`
pub async fn get(
    State(state): State<AdminState>,
    Query(query): Query<MetricsQuery>,
) -> Result<Json<crate::contracts::MetricsResponse>, ApiError> {
    let (range, step, points) = match query.range.as_deref() {
        None | Some("24h") => ("24h", 15, 96),
        Some("1h") => ("1h", 1, 60),
        Some(_) => {
            return Err(ApiError::bad_request("invalid_range", "range is 1h or 24h"));
        }
    };
    Ok(Json(snapshot(
        &state.metrics,
        now_secs(),
        range,
        step,
        points,
    )))
}

fn snapshot(
    metrics: &Metrics,
    now: u64,
    range: &str,
    step: u64,
    points: u64,
) -> crate::contracts::MetricsResponse {
    let (series, total) = metrics.series(now, step, points);
    crate::contracts::MetricsResponse {
        range: range.to_owned(),
        step_secs: step * 60,
        since: rfc3339(metrics.started),
        points: series
            .iter()
            .map(|(index, m)| crate::contracts::MetricsPoint {
                at: rfc3339(index * 60),
                requests: m.requests,
                refused: m.refused,
                errors: m.errors,
                p95_ms: m.p95_ms(),
            })
            .collect(),
        totals: crate::contracts::MetricsTotals {
            requests: total.requests,
            refused: total.refused,
            errors: total.errors,
            rest: total.rest,
            auth: total.auth,
            storage: total.storage,
            p95_ms: total.p95_ms(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MS: fn(u64) -> Duration = Duration::from_millis;

    #[test]
    fn counts_by_status_area_and_latency() {
        let metrics = Metrics::default();
        let minute = 29_000_000;
        metrics.record_at(minute, Area::Rest, 200, MS(3));
        metrics.record_at(minute, Area::Rest, 401, MS(7));
        metrics.record_at(minute, Area::Auth, 429, MS(40));
        metrics.record_at(minute, Area::Storage, 503, MS(900));
        let (series, total) = metrics.series(minute * 60 + 30, 1, 60);
        assert_eq!(series.len(), 60);
        let last = &series[59].1;
        assert_eq!((last.requests, last.refused, last.errors), (4, 2, 1));
        assert_eq!((total.rest, total.auth, total.storage), (2, 1, 1));
        // 95% of 4 requests = all 4: the slowest bucket (< 1000 ms).
        assert_eq!(total.p95_ms(), Some(1_000));
        assert_eq!(series[0].1.p95_ms(), None);
    }

    #[test]
    fn buckets_group_minutes_and_fill_gaps_with_zeros() {
        let metrics = Metrics::default();
        let end = 29_000_000;
        metrics.record_at(end - 20, Area::Rest, 200, MS(1));
        metrics.record_at(end - 5, Area::Rest, 200, MS(1));
        metrics.record_at(end, Area::Rest, 200, MS(1));
        let (series, total) = metrics.series(end * 60, 15, 96);
        assert_eq!(series.len(), 96);
        assert_eq!(
            series[95].0,
            end - 14,
            "the last bucket ends at the current minute"
        );
        assert_eq!(series[95].1.requests, 2);
        assert_eq!(series[94].1.requests, 1);
        assert_eq!(total.requests, 3);
    }

    #[test]
    fn keeps_only_the_last_24_hours() {
        let metrics = Metrics::default();
        let start = 29_000_000;
        metrics.record_at(start, Area::Rest, 200, MS(1));
        metrics.record_at(start + RETENTION_MINUTES, Area::Rest, 200, MS(1));
        assert_eq!(metrics.minutes.lock().unwrap().len(), 1);
    }

    #[test]
    fn slow_requests_land_in_the_open_bucket() {
        let mut minute = Minute::default();
        minute.latency[BUCKETS_MS.len()] = 10;
        assert_eq!(minute.p95_ms(), Some(10_000));
        assert_eq!(Area::of("/rest/v1/todos"), Area::Rest);
        assert_eq!(Area::of("/storage/v1/object/a/b"), Area::Storage);
        assert_eq!(Area::of("/health"), Area::Other);
    }
}
