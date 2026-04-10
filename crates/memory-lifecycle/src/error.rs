//! Lifecycle-specific error types

use thiserror::Error;
use memory_core::MemoryError;

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
            LifecycleError::TransitionError(msg) => MemoryError::InvalidOperation(format!("Transition: {}", msg)),
            LifecycleError::PolicyError(msg) => MemoryError::InvalidOperation(format!("Policy: {}", msg)),
            LifecycleError::SchedulerError(msg) => MemoryError::InvalidOperation(format!("Scheduler: {}", msg)),
            LifecycleError::StorageError(msg) => MemoryError::StorageError(msg),
            LifecycleError::InvalidTransition(from, to) => MemoryError::InvalidOperation(format!("Invalid transition: {} -> {}", from, to)),
            LifecycleError::NotFound(id) => MemoryError::NotFound(id),
        }
    }
}
