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
