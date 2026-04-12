//! Lifecycle-specific error types

use memory_core::MemoryError;
use thiserror::Error;

/// Lifecycle-specific errors
#[derive(Error, Debug)]
pub enum LifecycleError {
    #[error("Transition error: {0}")]
    TransitionError(String),

    #[error("Policy error: {0}")]
    PolicyError(String),

    #[error("Scheduler error: {0}")]
    SchedulerError(String),

    #[error("Storage error: {0}")]
    StorageError(String),

    #[error("Invalid status transition: {0} -> {1}")]
    InvalidTransition(String, String),

    #[error("Memory not found: {0}")]
    NotFound(String),
}

impl From<LifecycleError> for MemoryError {
    fn from(err: LifecycleError) -> Self {
        match err {
            LifecycleError::TransitionError(msg) => {
                MemoryError::InvalidOperation(format!("Transition: {}", msg))
            }
            LifecycleError::PolicyError(msg) => {
                MemoryError::InvalidOperation(format!("Policy: {}", msg))
            }
            LifecycleError::SchedulerError(msg) => {
                MemoryError::InvalidOperation(format!("Scheduler: {}", msg))
            }
            LifecycleError::StorageError(msg) => MemoryError::StorageError(msg),
            LifecycleError::InvalidTransition(from, to) => {
                MemoryError::InvalidOperation(format!("Invalid transition: {} -> {}", from, to))
            }
            LifecycleError::NotFound(id) => MemoryError::NotFound(id),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transition_error_conversion() {
        let lifecycle_err = LifecycleError::TransitionError("invalid state".to_string());
        let memory_err: MemoryError = lifecycle_err.into();

        match memory_err {
            MemoryError::InvalidOperation(msg) => {
                assert!(msg.contains("Transition:"));
                assert!(msg.contains("invalid state"));
            }
            _ => panic!("Expected InvalidOperation error"),
        }
    }

    #[test]
    fn test_policy_error_conversion() {
        let lifecycle_err = LifecycleError::PolicyError("policy violation".to_string());
        let memory_err: MemoryError = lifecycle_err.into();

        match memory_err {
            MemoryError::InvalidOperation(msg) => {
                assert!(msg.contains("Policy:"));
                assert!(msg.contains("policy violation"));
            }
            _ => panic!("Expected InvalidOperation error"),
        }
    }

    #[test]
    fn test_scheduler_error_conversion() {
        let lifecycle_err = LifecycleError::SchedulerError("scheduler timeout".to_string());
        let memory_err: MemoryError = lifecycle_err.into();

        match memory_err {
            MemoryError::InvalidOperation(msg) => {
                assert!(msg.contains("Scheduler:"));
                assert!(msg.contains("scheduler timeout"));
            }
            _ => panic!("Expected InvalidOperation error"),
        }
    }

    #[test]
    fn test_storage_error_conversion() {
        let lifecycle_err = LifecycleError::StorageError("failed to persist".to_string());
        let memory_err: MemoryError = lifecycle_err.into();

        match memory_err {
            MemoryError::StorageError(msg) => {
                assert_eq!(msg, "failed to persist");
            }
            _ => panic!("Expected StorageError"),
        }
    }

    #[test]
    fn test_invalid_transition_error_conversion() {
        let lifecycle_err =
            LifecycleError::InvalidTransition("active".to_string(), "frozen".to_string());
        let memory_err: MemoryError = lifecycle_err.into();

        match memory_err {
            MemoryError::InvalidOperation(msg) => {
                assert!(msg.contains("Invalid transition:"));
                assert!(msg.contains("active"));
                assert!(msg.contains("frozen"));
            }
            _ => panic!("Expected InvalidOperation error"),
        }
    }

    #[test]
    fn test_not_found_error_conversion() {
        let lifecycle_err = LifecycleError::NotFound("memory-123".to_string());
        let memory_err: MemoryError = lifecycle_err.into();

        match memory_err {
            MemoryError::NotFound(id) => {
                assert_eq!(id, "memory-123");
            }
            _ => panic!("Expected NotFound error"),
        }
    }

    #[test]
    fn test_error_message_preservation() {
        // Test that error messages are preserved through conversion
        let test_msg = "test error message 123";

        let transition_err = LifecycleError::TransitionError(test_msg.to_string());
        let memory_err: MemoryError = transition_err.into();

        let error_string = memory_err.to_string();
        assert!(error_string.contains(test_msg));
    }

    #[test]
    fn test_all_lifecycle_errors_convert_correctly() {
        // Test all variants convert without panic
        let errors = vec![
            LifecycleError::TransitionError("test".to_string()),
            LifecycleError::PolicyError("test".to_string()),
            LifecycleError::SchedulerError("test".to_string()),
            LifecycleError::StorageError("test".to_string()),
            LifecycleError::InvalidTransition("a".to_string(), "b".to_string()),
            LifecycleError::NotFound("test".to_string()),
        ];

        for err in errors {
            let _memory_err: MemoryError = err.into();
        }
    }
}
