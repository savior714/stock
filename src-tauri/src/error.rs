use serde::{Deserialize, Serialize};
use std::fmt;

pub type AppResult<T> = Result<T, AppError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AppErrorCode {
    Validation,
    NotFound,
    Conflict,
    Database,
    ProviderRateLimited,
    ProviderUnavailable,
    InvalidMarketData,
    InsufficientData,
    Cancelled,
    Internal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppError {
    pub code: AppErrorCode,
    pub message: String,
    pub detail: Option<String>,
    pub retryable: bool,
}

impl AppError {
    pub fn new(code: AppErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            detail: None,
            retryable: false,
        }
    }

    pub fn validation(message: impl Into<String>) -> Self {
        Self::new(AppErrorCode::Validation, message)
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new(AppErrorCode::NotFound, message)
    }

    pub fn conflict(message: impl Into<String>) -> Self {
        Self::new(AppErrorCode::Conflict, message)
    }

    pub fn database(message: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            code: AppErrorCode::Database,
            message: message.into(),
            detail: Some(detail.into()),
            retryable: false,
        }
    }

    pub fn internal(message: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            code: AppErrorCode::Internal,
            message: message.into(),
            detail: Some(detail.into()),
            retryable: false,
        }
    }

    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    pub fn retryable(mut self, retryable: bool) -> Self {
        self.retryable = retryable;
        self
    }
}

impl fmt::Display for AppError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.detail {
            Some(detail) => write!(formatter, "{}: {}", self.message, detail),
            None => formatter.write_str(&self.message),
        }
    }
}

impl std::error::Error for AppError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_error_code_as_snake_case() {
        let error = AppError::new(AppErrorCode::InvalidMarketData, "bad bar");
        let value = serde_json::to_value(error).expect("error must serialize");

        assert_eq!(value["code"], "invalid_market_data");
        assert_eq!(value["retryable"], false);
    }

    #[test]
    fn creates_conflict_error() {
        let error = AppError::conflict("duplicate watchlist");

        assert_eq!(error.code, AppErrorCode::Conflict);
        assert_eq!(error.message, "duplicate watchlist");
    }
}
