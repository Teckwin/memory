//! Memory API error types

use thiserror::Error;
use memory_core::MemoryError;

/// API-specific errors
#[derive(Error, Debug)]
pub enum ApiError {
    #[error("Client not initialized: {0}")]
    ClientNotInitialized(String),

    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("Invalid parameter: {0}")]
    InvalidParameter(String),

    #[error("Connection error: {0}")]
    ConnectionError(String),

    #[error("Timeout: {0}")]
    Timeout(String),

    #[error("Backend error: {0}")]
    BackendError(String),

    #[error("Core error: {0}")]
    CoreError(#[from] MemoryError),
}

impl From<ApiError> for MemoryError {
    fn from(err: ApiError) -> Self {
        match err {
            ApiError::CoreError(e) => e,
            ApiError::ClientNotInitialized(msg) => MemoryError::InvalidOperation(msg),
            ApiError::ConfigError(msg) => MemoryError::InvalidOperation(msg),
            ApiError::InvalidParameter(msg) => MemoryError::InvalidOperation(msg),
            ApiError::ConnectionError(msg) => MemoryError::StorageError(msg),
            ApiError::Timeout(msg) => MemoryError::StorageError(msg),
            ApiError::BackendError(msg) => MemoryError::StorageError(msg),
        }
    }
}
