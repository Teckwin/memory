//! Model Manager - Manages trained model files

use crate::error::TrainError;
use memory_core::{TrainModel, LoadedModel, ModelType};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;
use chrono::{DateTime, Utc};

/// Manages trained model files (save, load, list)
pub struct ModelManager {
    /// Base directory for storing models
    models_dir: PathBuf,
    /// In-memory cache of model metadata
    models_cache: Arc<RwLock<Vec<TrainModel>>>,
}

impl ModelManager {
    /// Create a new ModelManager with the given models directory
    pub fn new(models_dir: PathBuf) -> Self {
        Self {
            models_dir,
            models_cache: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Initialize the model manager
    pub async fn initialize(&self) -> Result<(), TrainError> {
        // Create models directory if it doesn't exist
        std::fs::create_dir_all(&self.models_dir)?;
        
        // Load existing models into cache
        self.refresh_cache().await?;
        
        tracing::info!("ModelManager initialized with {} models", 
            self.models_cache.read().await.len());
        
        Ok(())
    }

    /// Refresh the models cache by scanning the directory
    async fn refresh_cache(&self) -> Result<(), TrainError> {
        let mut models = Vec::new();
        
        if !self.models_dir.exists() {
            return Ok(());
        }

        for entry in std::fs::read_dir(&self.models_dir)? {
            let entry = entry?;
            let path = entry.path();
            
            if path.extension().and_then(|s| s.to_str()) == Some("model") {
                if let Some(model) = self.load_model_metadata(&path).await? {
                    models.push(model);
                }
            }
        }

        let mut cache = self.models_cache.write().await;
        *cache = models;
        
        Ok(())
    }

    /// Load model metadata from a file
    async fn load_model_metadata(&self, path: &Path) -> Result<Option<TrainModel>, TrainError> {
        let content = std::fs::read_to_string(path)?;
        
        // Try to parse as JSON model file
        if let Ok(info) = serde_json::from_str::<serde_json::Value>(&content) {
            let model_id = info.get("model_id")
                .and_then(|v: &serde_json::Value| v.as_str())
                .and_then(|s| Uuid::parse_str(s).ok())
                .unwrap_or_else(Uuid::new_v4);
            
            let name = info.get("model_type")
                .and_then(|v: &serde_json::Value| v.as_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| "Unknown".to_string());
            
            let model_type = match info.get("model_type")
                .and_then(|v: &serde_json::Value| v.as_str())
                .map(|s| s.to_lowercase())
                .as_deref() {
                    Some("classifier") => ModelType::Classifier,
                    Some("reranker") => ModelType::Reranker,
                    _ => ModelType::Embedding,
                };
            
            let created_at = info.get("created_at")
                .and_then(|v: &serde_json::Value| v.as_str())
                .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(Utc::now);

            return Ok(Some(TrainModel {
                id: model_id,
                name,
                model_type,
                created_at,
                file_path: path.to_path_buf(),
            }));
        }

        // If not JSON, create basic metadata
        let model_id = Uuid::new_v4();
        let filename = path.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("model");
        
        Ok(Some(TrainModel {
            id: model_id,
            name: filename.to_string(),
            model_type: ModelType::Embedding,
            created_at: Utc::now(),
            file_path: path.to_path_buf(),
        }))
    }

    /// List all available models
    pub async fn list_models(&self) -> Result<Vec<TrainModel>, TrainError> {
        // Ensure cache is up to date
        self.refresh_cache().await?;
        
        let cache = self.models_cache.read().await;
        Ok(cache.clone())
    }

    /// Load a model by ID
    pub async fn load_model(&self, model_id: Uuid) -> Result<LoadedModel, TrainError> {
        let cache = self.models_cache.read().await;
        
        let model = cache.iter()
            .find(|m| m.id == model_id)
            .ok_or_else(|| TrainError::ModelNotFound(model_id.to_string()))?;

        // Load the model file
        let path = &model.file_path;
        if !path.exists() {
            return Err(TrainError::ModelLoadError(
                format!("Model file not found: {}", path.display())
            ));
        }

        // For now, return a placeholder model
        tracing::info!("Loading model: {} from {}", model_id, path.display());

        Ok(LoadedModel {
            id: model_id,
            model: Box::new(()), // Placeholder
        })
    }

    /// Get a specific model by ID (without loading the full model)
    pub async fn get_model(&self, model_id: Uuid) -> Result<TrainModel, TrainError> {
        let cache = self.models_cache.read().await;
        
        cache.iter()
            .find(|m| m.id == model_id)
            .cloned()
            .ok_or_else(|| TrainError::ModelNotFound(model_id.to_string()))
    }

    /// Delete a model by ID
    pub async fn delete_model(&self, model_id: Uuid) -> Result<(), TrainError> {
        let mut cache = self.models_cache.write().await;
        
        let position = cache.iter()
            .position(|m| m.id == model_id)
            .ok_or_else(|| TrainError::ModelNotFound(model_id.to_string()))?;

        let model = cache.remove(position);
        
        // Delete the file
        if model.file_path.exists() {
            std::fs::remove_file(&model.file_path)?;
        }

        tracing::info!("Deleted model: {}", model_id);
        
        Ok(())
    }

    /// Get the models directory path
    pub fn models_dir(&self) -> &Path {
        &self.models_dir
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_model_manager(temp_dir: &TempDir) -> ModelManager {
        let models_dir = temp_dir.path().to_path_buf();
        ModelManager::new(models_dir)
    }

    #[tokio::test]
    async fn test_model_manager_initialize() {
        let temp_dir = TempDir::new().unwrap();
        let manager = create_model_manager(&temp_dir);
        
        let result = manager.initialize().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_list_models_empty() {
        let temp_dir = TempDir::new().unwrap();
        let manager = create_model_manager(&temp_dir);
        manager.initialize().await.unwrap();
        
        let models = manager.list_models().await.unwrap();
        assert!(models.is_empty());
    }

    #[tokio::test]
    async fn test_load_nonexistent_model() {
        let temp_dir = TempDir::new().unwrap();
        let manager = create_model_manager(&temp_dir);
        manager.initialize().await.unwrap();
        
        let result = manager.load_model(Uuid::new_v4()).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_get_model_not_found() {
        let temp_dir = TempDir::new().unwrap();
        let manager = create_model_manager(&temp_dir);
        manager.initialize().await.unwrap();
        
        let result = manager.get_model(Uuid::new_v4()).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_models_dir() {
        let temp_dir = TempDir::new().unwrap();
        let manager = create_model_manager(&temp_dir);
        
        assert_eq!(manager.models_dir(), temp_dir.path());
    }
}
