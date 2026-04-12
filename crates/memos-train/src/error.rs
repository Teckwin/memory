//! Training-specific error types

use thiserror::Error;

/// Errors that can occur during training operations
#[derive(Error, Debug)]
pub enum TrainError {
    #[error("Data preparation error: {0}")]
    DataPreparationError(String),

    #[error("Training error: {0}")]
    TrainingError(String),

    #[error("Model not found: {0}")]
    ModelNotFound(String),

    #[error("Model loading error: {0}")]
    ModelLoadError(String),

    #[error("Model saving error: {0}")]
    ModelSaveError(String),

    #[error("Insufficient data for training: {0}")]
    InsufficientData(String),

    #[error("Invalid parameters: {0}")]
    InvalidParameters(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),
}
