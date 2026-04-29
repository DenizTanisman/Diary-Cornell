//! Domain error type returned by Tauri commands.
//!
//! Frontend sees `{ "code": "...", "message": "..." }` — never a stack trace,
//! never a raw rusqlite/sqlx error. The `code` is stable; `message` is safe
//! to surface to the user.

use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DomainError {
    #[error("repository not initialized")]
    NotInitialized,

    #[error("invalid date: {0}")]
    InvalidDate(String),

    #[error("entry not found for {0}")]
    #[allow(dead_code)] // surfaces in FAZ 1.2 + sync error mapping
    NotFound(String),

    #[error("invalid input: {0}")]
    Validation(String),

    #[error("storage error: {0}")]
    Storage(String),

    #[error("path error: {0}")]
    Path(String),

    #[error("internal error: {0}")]
    Internal(String),
}

/// Code/message envelope frontend sees. Never embed raw error chains here.
#[derive(Serialize)]
pub struct ErrorEnvelope {
    pub code: &'static str,
    pub message: String,
}

impl DomainError {
    fn code(&self) -> &'static str {
        match self {
            Self::NotInitialized => "not_initialized",
            Self::InvalidDate(_) => "invalid_date",
            Self::NotFound(_) => "not_found",
            Self::Validation(_) => "validation",
            Self::Storage(_) => "storage",
            Self::Path(_) => "path",
            Self::Internal(_) => "internal",
        }
    }
}

impl Serialize for DomainError {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        ErrorEnvelope {
            code: self.code(),
            message: self.to_string(),
        }
        .serialize(serializer)
    }
}

impl From<rusqlite::Error> for DomainError {
    fn from(err: rusqlite::Error) -> Self {
        DomainError::Storage(err.to_string())
    }
}

impl From<anyhow::Error> for DomainError {
    fn from(err: anyhow::Error) -> Self {
        DomainError::Internal(err.to_string())
    }
}

impl From<tauri::Error> for DomainError {
    fn from(err: tauri::Error) -> Self {
        DomainError::Path(err.to_string())
    }
}

impl From<tokio::task::JoinError> for DomainError {
    fn from(err: tokio::task::JoinError) -> Self {
        DomainError::Internal(format!("task join: {err}"))
    }
}
