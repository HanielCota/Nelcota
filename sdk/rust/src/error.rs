use std::time::Duration;

/// SDK failures. Server error codes are preserved verbatim.
#[derive(Clone, Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    #[error("{message} ({code}, HTTP {status})")]
    Http {
        status: u16,
        code: String,
        message: String,
        retry_after: Option<Duration>,
    },
    #[error("invalid SDK input: {0}")]
    Usage(String),
    #[error("network request failed: {0}")]
    Network(String),
    #[error("request timed out")]
    Timeout,
    #[error("request cancelled")]
    Cancelled,
    #[error("invalid response (HTTP {status}): {message}")]
    InvalidResponse { status: u16, message: String },
    #[error("expected {expected} row(s), got {actual}")]
    Cardinality {
        expected: &'static str,
        actual: usize,
    },
    #[error("not signed in")]
    SessionMissing,
    #[error("session storage failed: {0}")]
    SessionStorage(String),
}

impl Error {
    pub fn code(&self) -> &str {
        match self {
            Self::Http { code, .. } => code,
            Self::Usage(_) => "invalid_input",
            Self::Network(_) => "network_error",
            Self::Timeout => "timeout",
            Self::Cancelled => "aborted",
            Self::InvalidResponse { .. } => "invalid_response",
            Self::Cardinality { .. } => "not_single",
            Self::SessionMissing => "session_missing",
            Self::SessionStorage(_) => "session_storage_error",
        }
    }

    pub fn status(&self) -> Option<u16> {
        match self {
            Self::Http { status, .. } | Self::InvalidResponse { status, .. } => Some(*status),
            _ => None,
        }
    }

    pub fn retry_after(&self) -> Option<Duration> {
        match self {
            Self::Http { retry_after, .. } => *retry_after,
            _ => None,
        }
    }

    pub(crate) fn network(error: reqwest::Error) -> Self {
        if error.is_timeout() {
            Self::Timeout
        } else {
            // URLs may contain signed storage tokens. Never include them in errors.
            Self::Network(error.without_url().to_string())
        }
    }
}

pub type Result<T> = std::result::Result<T, Error>;
