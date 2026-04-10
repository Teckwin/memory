//! Index-specific error types

use thiserror::Error;

/// Index-related errors
#[derive(Error, Debug)]
pub enum IndexError {
    #[error("Index not found: {0}")]
    NotFound(String),

    #[error("Index already exists: {0}")]
    AlreadyExists(String),

    #[error("Index operation failed: {0}")]
    OperationFailed(String),

    #[error("Invalid dimension: expected {expected}, got {got}")]
    InvalidDimension {
        expected: usize,
        got: usize,
    },

    #[error("Invalid query: {0}")]
    InvalidQuery(String),

    #[error("Serialization error: {0}")]
    SerializationError(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Tantivy error: {0}")]
    TantivyError(String),

    #[error("HNSW error: {0}")]
    HnswError(String),
}

impl From<tantivy::TantivyError> for IndexError {
    fn from(e: tantivy::TantivyError) -> Self {
        IndexError::TantivyError(e.to_string())
    }
}

impl From<tantivy::query::QueryParserError> for IndexError {
    fn from(e: tantivy::query::QueryParserError) -> Self {
        IndexError::InvalidQuery(e.to_string())
    }
}

impl From<String> for IndexError {
    fn from(s: String) -> Self {
        IndexError::OperationFailed(s)
    }
}

impl From<&str> for IndexError {
    fn from(s: &str) -> Self {
        IndexError::OperationFailed(s.to_string())
    }
}

impl serde::Serialize for IndexError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_index_error_display() {
        let err = IndexError::NotFound("test-index".to_string());
        assert_eq!(err.to_string(), "Index not found: test-index");
    }

    #[test]
    fn test_invalid_dimension_error() {
        let err = IndexError::InvalidDimension {
            expected: 384,
            got: 128,
        };
        assert_eq!(
            err.to_string(),
            "Invalid dimension: expected 384, got 128"
        );
    }

    #[test]
    fn test_from_tantivy_error() {
        let err = IndexError::TantivyError("test error".to_string());
        assert_eq!(err.to_string(), "Tantivy error: test error");
    }

    #[test]
    fn test_from_string() {
        let err: IndexError = "test error".into();
        assert_eq!(err.to_string(), "Index operation failed: test error");
    }

    #[test]
    fn test_serialization() {
        let err = IndexError::NotFound("test".to_string());
        let serialized = serde_json::to_string(&err).unwrap();
        assert_eq!(serialized, "\"Index not found: test\"");
    }
}
