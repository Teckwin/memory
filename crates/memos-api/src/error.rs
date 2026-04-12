//! Memory API error types

use memos_core::MemoryError;
use thiserror::Error;

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

#[cfg(test)]
mod tests {
    use super::*;
    use memos_core::MemoryError;

    // ==================== ApiError Display Tests ====================

    #[test]
    fn test_api_error_client_not_initialized_display() {
        let err = ApiError::ClientNotInitialized("test error".to_string());
        assert_eq!(format!("{}", err), "Client not initialized: test error");
    }

    #[test]
    fn test_api_error_config_error_display() {
        let err = ApiError::ConfigError("invalid config".to_string());
        assert_eq!(format!("{}", err), "Configuration error: invalid config");
    }

    #[test]
    fn test_api_error_invalid_parameter_display() {
        let err = ApiError::InvalidParameter("invalid param".to_string());
        assert_eq!(format!("{}", err), "Invalid parameter: invalid param");
    }

    #[test]
    fn test_api_error_connection_error_display() {
        let err = ApiError::ConnectionError("connection failed".to_string());
        assert_eq!(format!("{}", err), "Connection error: connection failed");
    }

    #[test]
    fn test_api_error_timeout_display() {
        let err = ApiError::Timeout("operation timed out".to_string());
        assert_eq!(format!("{}", err), "Timeout: operation timed out");
    }

    #[test]
    fn test_api_error_backend_error_display() {
        let err = ApiError::BackendError("backend failed".to_string());
        assert_eq!(format!("{}", err), "Backend error: backend failed");
    }

    #[test]
    fn test_api_error_core_error_display() {
        let core_err = MemoryError::NotFound("memory not found".to_string());
        let err = ApiError::CoreError(core_err);
        assert_eq!(
            format!("{}", err),
            "Core error: Memory not found: memory not found"
        );
    }

    // ==================== From<ApiError> for MemoryError Tests ====================

    #[test]
    fn test_from_api_error_core_error() {
        let api_err = ApiError::CoreError(MemoryError::NotFound("test".to_string()));
        let memory_err: MemoryError = api_err.into();

        match memory_err {
            MemoryError::NotFound(msg) => {
                assert_eq!(msg, "test");
            }
            _ => panic!("Expected NotFound"),
        }
    }

    #[test]
    fn test_from_api_error_client_not_initialized() {
        let api_err = ApiError::ClientNotInitialized("not initialized".to_string());
        let memory_err: MemoryError = api_err.into();

        match memory_err {
            MemoryError::InvalidOperation(msg) => {
                assert_eq!(msg, "not initialized");
            }
            _ => panic!("Expected InvalidOperation"),
        }
    }

    #[test]
    fn test_from_api_error_config_error() {
        let api_err = ApiError::ConfigError("config error".to_string());
        let memory_err: MemoryError = api_err.into();

        match memory_err {
            MemoryError::InvalidOperation(msg) => {
                assert_eq!(msg, "config error");
            }
            _ => panic!("Expected InvalidOperation"),
        }
    }

    #[test]
    fn test_from_api_error_invalid_parameter() {
        let api_err = ApiError::InvalidParameter("invalid param".to_string());
        let memory_err: MemoryError = api_err.into();

        match memory_err {
            MemoryError::InvalidOperation(msg) => {
                assert_eq!(msg, "invalid param");
            }
            _ => panic!("Expected InvalidOperation"),
        }
    }

    #[test]
    fn test_from_api_error_connection_error() {
        let api_err = ApiError::ConnectionError("connection failed".to_string());
        let memory_err: MemoryError = api_err.into();

        match memory_err {
            MemoryError::StorageError(msg) => {
                assert_eq!(msg, "connection failed");
            }
            _ => panic!("Expected StorageError"),
        }
    }

    #[test]
    fn test_from_api_error_timeout() {
        let api_err = ApiError::Timeout("timeout".to_string());
        let memory_err: MemoryError = api_err.into();

        match memory_err {
            MemoryError::StorageError(msg) => {
                assert_eq!(msg, "timeout");
            }
            _ => panic!("Expected StorageError"),
        }
    }

    #[test]
    fn test_from_api_error_backend_error() {
        let api_err = ApiError::BackendError("backend failed".to_string());
        let memory_err: MemoryError = api_err.into();

        match memory_err {
            MemoryError::StorageError(msg) => {
                assert_eq!(msg, "backend failed");
            }
            _ => panic!("Expected StorageError"),
        }
    }
}
