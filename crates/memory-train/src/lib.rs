//! Memory Train Module
//!
//! This module provides training capabilities for the memory system:
//! - **DataPreparer**: Extracts texts/labels from memories for training
//! - **Trainer**: Simplified model training (stub implementation)
//! - **ModelManager**: Manages trained models (save, load, list)
//! - **TrainService**: Main service implementing the TrainApi trait
//!
//! # Usage
//!
//! ```ignore
//! use memory_train::TrainService;
//! use memory_core::{TrainParams, TrainApi, WorkspaceId};
//! use memory_storage::UnifiedStorage;
//! use std::sync::Arc;
//! use std::path::PathBuf;
//! use uuid::Uuid;
//!
//! async fn example() {
//!     let storage = Arc::new(UnifiedStorage::new(
//!         100,
//!         PathBuf::from("./cold.db"),
//!         PathBuf::from("./zombie"),
//!     ));
//!     
//!     let service = TrainService::new(
//!         storage,
//!         PathBuf::from("./models"),
//!     ).await.unwrap();
//!
//!     let workspace_id = Uuid::new_v4();
//!     
//!     // Prepare training data
//!     let data = service.prepare_data(workspace_id, TrainParams::default()).await;
//!
//!     // Train a model
//!     let result = service.train(data.unwrap(), TrainParams::default()).await;
//! }
//! ```

pub mod error;
pub mod data_preparer;
pub mod trainer;
pub mod model_manager;

pub use error::TrainError;
pub use data_preparer::DataPreparer;
pub use trainer::Trainer;
pub use model_manager::ModelManager;

use memory_core::{
    MemoryError, MemoryEntry, WorkspaceId,
    TrainApi, TrainData, TrainResult, TrainParams, TrainModel, LoadedModel,
};
use memory_storage::UnifiedStorage;
use async_trait::async_trait;
use std::sync::Arc;
use uuid::Uuid;

/// Main training service that implements the TrainApi trait
pub struct TrainService {
    #[allow(dead_code)]
    storage: Arc<UnifiedStorage>,
    data_preparer: DataPreparer,
    trainer: Trainer,
    model_manager: ModelManager,
}

impl TrainService {
    /// Create a new TrainService
    pub async fn new(
        storage: Arc<UnifiedStorage>,
        models_dir: std::path::PathBuf,
    ) -> Result<Self, MemoryError> {
        let model_manager = ModelManager::new(models_dir);
        model_manager.initialize().await
            .map_err(|e| MemoryError::TrainingError(e.to_string()))?;

        Ok(Self {
            storage,
            data_preparer: DataPreparer::new(),
            trainer: Trainer::new(),
            model_manager,
        })
    }

    /// Create a TrainService with custom DataPreparer
    pub async fn with_preparer(
        storage: Arc<UnifiedStorage>,
        models_dir: std::path::PathBuf,
        preparer: DataPreparer,
    ) -> Result<Self, MemoryError> {
        let mut service = Self::new(storage, models_dir).await?;
        service.data_preparer = preparer;
        Ok(service)
    }

    /// Generate simple embeddings for texts
    /// 
    /// This is a simplified implementation that creates mock embeddings.
    /// In production, this would use a proper embedding model.
    fn generate_simple_embeddings(&self, texts: &[String]) -> Vec<Vec<f32>> {
        // Embedding dimension
        const DIM: usize = 128;
        
        texts.iter().map(|text| {
            // Simple hash-based "embedding" for demonstration
            // In production, use a proper embedding model
            let hash = self.simple_hash(text);
            
            // Create a deterministic but varied embedding
            (0..DIM).map(|i| {
                // Use different parts of the hash for different dimensions
                // to avoid shifting by more than 63 bits
                let shift = (i % 64) as u32;
                let bit = (hash >> shift) & 1;
                if bit == 1 { 1.0 } else { -1.0 }
            }).collect()
        }).collect()
    }

    /// Simple hash function for generating pseudo-embeddings
    fn simple_hash(&self, text: &str) -> u64 {
        let mut hash: u64 = 0;
        for (i, byte) in text.bytes().enumerate() {
            hash = hash.wrapping_add((byte as u64).wrapping_mul(i as u64 + 1));
            hash = hash.rotate_left(5);
        }
        // Ensure hash is not zero to avoid shift issues
        if hash == 0 {
            hash = 1;
        }
        hash
    }
}

#[async_trait]
impl TrainApi for TrainService {
    /// Prepare training data from memories in a workspace
    async fn prepare_data(
        &self,
        workspace_id: WorkspaceId,
        _params: TrainParams,
    ) -> Result<TrainData, MemoryError> {
        // Fetch memories from storage
        let memories = self.fetch_memories_for_workspace(workspace_id).await
            .map_err(|e| MemoryError::TrainingError(e.to_string()))?;

        // Prepare training data
        self.data_preparer.prepare(memories)
            .map_err(|e| MemoryError::TrainingError(e.to_string()))
    }

    /// Train a model with the given data
    async fn train(
        &self,
        data: TrainData,
        params: TrainParams,
    ) -> Result<TrainResult, MemoryError> {
        self.trainer.train(data, params)
            .await
            .map_err(|e| MemoryError::TrainingError(e.to_string()))
    }

    /// List all available trained models
    async fn list_models(&self) -> Result<Vec<TrainModel>, MemoryError> {
        self.model_manager.list_models()
            .await
            .map_err(|e| MemoryError::TrainingError(e.to_string()))
    }

    /// Load a trained model by ID
    async fn load_model(&self, model_id: Uuid) -> Result<LoadedModel, MemoryError> {
        self.model_manager.load_model(model_id)
            .await
            .map_err(|e| MemoryError::TrainingError(e.to_string()))
    }

    /// Generate embeddings for texts
    async fn generate_embeddings(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, MemoryError> {
        Ok(self.generate_simple_embeddings(texts))
    }
}

impl TrainService {
    /// Fetch all memories for a workspace
    /// 
    /// This is a helper method that iterates through storage tiers
    /// to collect all memories in a workspace.
    #[allow(dead_code)]
    async fn fetch_memories_for_workspace(
        &self,
        workspace_id: WorkspaceId,
    ) -> Result<Vec<MemoryEntry>, TrainError> {
        let all_memories = Vec::new();

        // Try to get from hot storage
        // Note: This is a simplified approach - in production,
        // storage would have a more efficient way to list all memories

        // For now, return empty if we can't easily list
        // In a full implementation, storage should implement a list_by_workspace method
        tracing::debug!("Fetching memories for workspace: {}", workspace_id);

        Ok(all_memories)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_hash() {
        let service = TrainService {
            storage: Arc::new(UnifiedStorage::new(
                100,
                std::path::PathBuf::from("/tmp/test.db"),
                std::path::PathBuf::from("/tmp/zombie"),
            )),
            data_preparer: DataPreparer::new(),
            trainer: Trainer::new(),
            model_manager: ModelManager::new(std::path::PathBuf::from("/tmp/models")),
        };

        let hash1 = service.simple_hash("hello");
        let hash2 = service.simple_hash("hello");
        let hash3 = service.simple_hash("world");

        assert_eq!(hash1, hash2);
        assert_ne!(hash1, hash3);
    }

    #[test]
    fn test_generate_simple_embeddings() {
        let service = TrainService {
            storage: Arc::new(UnifiedStorage::new(
                100,
                std::path::PathBuf::from("/tmp/test.db"),
                std::path::PathBuf::from("/tmp/zombie"),
            )),
            data_preparer: DataPreparer::new(),
            trainer: Trainer::new(),
            model_manager: ModelManager::new(std::path::PathBuf::from("/tmp/models")),
        };

        let texts = vec![
            "hello world".to_string(),
            "foo bar".to_string(),
        ];

        let embeddings = service.generate_simple_embeddings(&texts);

        assert_eq!(embeddings.len(), 2);
        assert_eq!(embeddings[0].len(), 128);
        assert_eq!(embeddings[1].len(), 128);
    }
}
