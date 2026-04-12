//! Storage-specific error types

use memory_core::MemoryError;
use thiserror::Error;

/// Storage layer errors
#[derive(Error, Debug)]
pub enum StorageError {
    #[error("Hot storage error: {0}")]
    HotError(String),

    #[error("Cold storage error: {0}")]
    ColdError(String),

    #[error("Zombie storage error: {0}")]
    ZombieError(String),

    #[error("Database error: {0}")]
    DatabaseError(String),

    #[error("Serialization error: {0}")]
    SerializationError(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("SQLite error: {0}")]
    SqliteError(#[from] rusqlite::Error),

    #[error("JSON error: {0}")]
    JsonError(#[from] serde_json::Error),
}

impl From<StorageError> for MemoryError {
    fn from(err: StorageError) -> Self {
        match err {
            StorageError::HotError(msg) => MemoryError::StorageError(format!("Hot: {}", msg)),
            StorageError::ColdError(msg) => MemoryError::StorageError(format!("Cold: {}", msg)),
            StorageError::ZombieError(msg) => MemoryError::StorageError(format!("Zombie: {}", msg)),
            StorageError::DatabaseError(msg) => MemoryError::StorageError(msg),
            StorageError::SerializationError(msg) => MemoryError::SerializationError(msg),
            StorageError::IoError(e) => MemoryError::IoError(e),
            StorageError::SqliteError(e) => MemoryError::StorageError(e.to_string()),
            StorageError::JsonError(e) => MemoryError::SerdeError(e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io;

    // Helper to create a serde_json::Error for testing
    fn create_json_error() -> serde_json::Error {
        serde_json::from_str::<serde_json::Value>("invalid json").unwrap_err()
    }

    #[test]
    fn test_hot_error_conversion() {
        let storage_err = StorageError::HotError("cache miss".to_string());
        let memory_err: MemoryError = storage_err.into();

        match memory_err {
            MemoryError::StorageError(msg) => {
                assert!(msg.contains("Hot:"));
                assert!(msg.contains("cache miss"));
            }
            _ => panic!("Expected StorageError"),
        }
    }

    #[test]
    fn test_cold_error_conversion() {
        let storage_err = StorageError::ColdError("archive not found".to_string());
        let memory_err: MemoryError = storage_err.into();

        match memory_err {
            MemoryError::StorageError(msg) => {
                assert!(msg.contains("Cold:"));
                assert!(msg.contains("archive not found"));
            }
            _ => panic!("Expected StorageError"),
        }
    }

    #[test]
    fn test_zombie_error_conversion() {
        let storage_err = StorageError::ZombieError("gc collection failed".to_string());
        let memory_err: MemoryError = storage_err.into();

        match memory_err {
            MemoryError::StorageError(msg) => {
                assert!(msg.contains("Zombie:"));
                assert!(msg.contains("gc collection failed"));
            }
            _ => panic!("Expected StorageError"),
        }
    }

    #[test]
    fn test_database_error_conversion() {
        let storage_err = StorageError::DatabaseError("connection refused".to_string());
        let memory_err: MemoryError = storage_err.into();

        match memory_err {
            MemoryError::StorageError(msg) => {
                assert_eq!(msg, "connection refused");
            }
            _ => panic!("Expected StorageError"),
        }
    }

    #[test]
    fn test_serialization_error_conversion() {
        let storage_err = StorageError::SerializationError("invalid format".to_string());
        let memory_err: MemoryError = storage_err.into();

        match memory_err {
            MemoryError::SerializationError(msg) => {
                assert_eq!(msg, "invalid format");
            }
            _ => panic!("Expected SerializationError"),
        }
    }

    #[test]
    fn test_io_error_conversion() {
        let io_err = io::Error::new(io::ErrorKind::NotFound, "file not found");
        let storage_err = StorageError::IoError(io_err);
        let memory_err: MemoryError = storage_err.into();

        match memory_err {
            MemoryError::IoError(e) => {
                assert_eq!(e.kind(), io::ErrorKind::NotFound);
            }
            _ => panic!("Expected IoError"),
        }
    }

    #[test]
    fn test_sqlite_error_conversion() {
        // Create a rusqlite::Error for testing - use InvalidQuery which always works
        let sqlite_err = rusqlite::Error::InvalidQuery;
        let storage_err = StorageError::SqliteError(sqlite_err);
        let memory_err: MemoryError = storage_err.into();

        match memory_err {
            MemoryError::StorageError(msg) => {
                assert!(!msg.is_empty(), "Error message should not be empty");
            }
            _ => panic!("Expected StorageError"),
        }
    }

    #[test]
    fn test_json_error_conversion() {
        let json_err = create_json_error();
        let storage_err = StorageError::JsonError(json_err);
        let memory_err: MemoryError = storage_err.into();

        match memory_err {
            MemoryError::SerdeError(e) => {
                assert!(e.to_string().contains("expected"));
            }
            _ => panic!("Expected SerdeError"),
        }
    }

    #[test]
    fn test_error_message_preservation() {
        // Test that error messages are preserved through conversion
        let test_msg = "test storage error message";

        let storage_err = StorageError::DatabaseError(test_msg.to_string());
        let memory_err: MemoryError = storage_err.into();

        let error_string = memory_err.to_string();
        assert!(error_string.contains(test_msg));
    }

    #[test]
    fn test_all_storage_errors_convert_correctly() {
        // Test all variants convert without panic
        let errors = vec![
            StorageError::HotError("test".to_string()),
            StorageError::ColdError("test".to_string()),
            StorageError::ZombieError("test".to_string()),
            StorageError::DatabaseError("test".to_string()),
            StorageError::SerializationError("test".to_string()),
            StorageError::IoError(io::Error::new(io::ErrorKind::Other, "test")),
            StorageError::SqliteError(rusqlite::Error::InvalidQuery),
            StorageError::JsonError(create_json_error()),
        ];

        for err in errors {
            let _memory_err: MemoryError = err.into();
        }
    }

    #[test]
    fn test_io_error_kind_preservation() {
        // Test various IO error kinds are preserved
        let test_cases = vec![
            (io::ErrorKind::NotFound, "NotFound"),
            (io::ErrorKind::PermissionDenied, "PermissionDenied"),
            (io::ErrorKind::AlreadyExists, "AlreadyExists"),
            (io::ErrorKind::ConnectionRefused, "ConnectionRefused"),
        ];

        for (kind, _name) in test_cases {
            let io_err = io::Error::new(kind, "test error");
            let storage_err = StorageError::IoError(io_err);
            let memory_err: MemoryError = storage_err.into();

            match memory_err {
                MemoryError::IoError(e) => {
                    assert_eq!(e.kind(), kind);
                }
                _ => panic!("Expected IoError"),
            }
        }
    }
}
