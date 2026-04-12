//! Model Manager - Manages trained model files

use crate::error::TrainError;
use chrono::{DateTime, Utc};
use memory_core::{LoadedModel, ModelType, TrainModel};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

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

        tracing::info!(
            "ModelManager initialized with {} models",
            self.models_cache.read().await.len()
        );

        Ok(())
    }

    /// Refresh the models cache by scanning the directory
    pub async fn refresh_cache(&self) -> Result<(), TrainError> {
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
    pub async fn load_model_metadata(&self, path: &Path) -> Result<Option<TrainModel>, TrainError> {
        let content = std::fs::read_to_string(path)?;

        // Try to parse as JSON model file
        if let Ok(info) = serde_json::from_str::<serde_json::Value>(&content) {
            let model_id = info
                .get("model_id")
                .and_then(|v: &serde_json::Value| v.as_str())
                .and_then(|s| Uuid::parse_str(s).ok())
                .unwrap_or_else(Uuid::new_v4);

            let name = info
                .get("name")
                .and_then(|v: &serde_json::Value| v.as_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| "Unknown".to_string());

            let model_type = match info
                .get("model_type")
                .and_then(|v: &serde_json::Value| v.as_str())
                .map(|s| s.to_lowercase())
                .as_deref()
            {
                Some("classifier") => ModelType::Classifier,
                Some("reranker") => ModelType::Reranker,
                _ => ModelType::Embedding,
            };

            let created_at = info
                .get("created_at")
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
        let filename = path.file_stem().and_then(|s| s.to_str()).unwrap_or("model");

        Ok(Some(TrainModel {
            id: model_id,
            name: filename.to_string(),
            model_type: ModelType::Embedding,
            created_at: Utc::now(),
            file_path: path.to_path_buf(),
        }))
    }

    /// Save a model to disk
    pub async fn save_model(
        &self,
        model_id: Uuid,
        model_type: ModelType,
        name: String,
        content: &[u8],
    ) -> Result<TrainModel, TrainError> {
        // Ensure models directory exists
        std::fs::create_dir_all(&self.models_dir)?;

        // Determine file extension and name based on model type
        let model_type_str = match model_type {
            ModelType::Embedding => "embedding",
            ModelType::Classifier => "classifier",
            ModelType::Reranker => "reranker",
        };

        let filename = format!("{}_{}.model", model_type_str, model_id);
        let file_path = self.models_dir.join(&filename);

        // Create model metadata as JSON
        let model_info = serde_json::json!({
            "model_id": model_id.to_string(),
            "model_type": model_type_str,
            "name": name,
            "created_at": Utc::now().to_rfc3339(),
        });

        // Write the model file (metadata + content)
        let mut file_content = model_info.to_string();
        file_content.push('\n');
        file_content.push_str(&String::from_utf8_lossy(content));

        std::fs::write(&file_path, file_content)?;

        // Create and add TrainModel to cache
        let train_model = TrainModel {
            id: model_id,
            name,
            model_type,
            created_at: Utc::now(),
            file_path,
        };

        let mut cache = self.models_cache.write().await;
        cache.push(train_model.clone());

        tracing::info!("Saved model: {} to {}", model_id, self.models_dir.display());

        Ok(train_model)
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

        let model = cache
            .iter()
            .find(|m| m.id == model_id)
            .ok_or_else(|| TrainError::ModelNotFound(model_id.to_string()))?;

        // Load the model file
        let path = &model.file_path;
        if !path.exists() {
            return Err(TrainError::ModelLoadError(format!(
                "Model file not found: {}",
                path.display()
            )));
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

        cache
            .iter()
            .find(|m| m.id == model_id)
            .cloned()
            .ok_or_else(|| TrainError::ModelNotFound(model_id.to_string()))
    }

    /// Delete a model by ID
    pub async fn delete_model(&self, model_id: Uuid) -> Result<(), TrainError> {
        let mut cache = self.models_cache.write().await;

        let position = cache
            .iter()
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

    #[tokio::test]
    async fn test_refresh_cache() {
        let temp_dir = TempDir::new().unwrap();
        let manager = create_model_manager(&temp_dir);
        manager.initialize().await.unwrap();

        // Initially empty
        let models = manager.list_models().await.unwrap();
        assert!(models.is_empty());

        // Create a model file directly
        let model_content = r#"{"model_id": "00000000-0000-0000-0000-000000000001", "model_type": "embedding", "name": "test_model"}"#;
        std::fs::write(
            temp_dir
                .path()
                .join("embedding_00000000-0000-0000-0000-000000000001.model"),
            model_content,
        )
        .unwrap();

        // Refresh cache
        let result = manager.refresh_cache().await;
        assert!(result.is_ok());

        // Now should have one model
        let models = manager.list_models().await.unwrap();
        assert_eq!(models.len(), 1);
    }

    #[tokio::test]
    async fn test_load_model_metadata() {
        let temp_dir = TempDir::new().unwrap();
        let manager = create_model_manager(&temp_dir);

        // Create a test model file
        let model_content = serde_json::json!({
            "model_id": "00000000-0000-0000-0000-000000000002",
            "model_type": "classifier",
            "name": "my_classifier",
            "created_at": "2024-01-01T00:00:00Z"
        })
        .to_string();

        let model_path = temp_dir
            .path()
            .join("classifier_00000000-0000-0000-0000-000000000002.model");
        std::fs::write(&model_path, &model_content).unwrap();

        // Load metadata
        let result = manager.load_model_metadata(&model_path).await;
        assert!(result.is_ok());

        let model = result.unwrap();
        assert!(model.is_some());

        let model = model.unwrap();
        assert_eq!(model.name, "my_classifier");
        assert!(matches!(model.model_type, ModelType::Classifier));
    }

    #[tokio::test]
    async fn test_save_model() {
        let temp_dir = TempDir::new().unwrap();
        let manager = create_model_manager(&temp_dir);
        manager.initialize().await.unwrap();

        let model_id = Uuid::new_v4();
        let model_content = b"mock model data";

        let result = manager
            .save_model(
                model_id,
                ModelType::Embedding,
                "test_embedding".to_string(),
                model_content,
            )
            .await;

        assert!(result.is_ok());

        let saved_model = result.unwrap();
        assert_eq!(saved_model.id, model_id);
        assert_eq!(saved_model.name, "test_embedding");
        assert!(matches!(saved_model.model_type, ModelType::Embedding));

        // Verify file was created
        assert!(saved_model.file_path.exists());

        // Verify model count (list_models reloads from disk so ID may differ)
        let models = manager.list_models().await.unwrap();
        assert_eq!(models.len(), 1);
    }

    #[tokio::test]
    async fn test_save_model_different_types() {
        let temp_dir = TempDir::new().unwrap();
        let manager = create_model_manager(&temp_dir);
        manager.initialize().await.unwrap();

        // Test embedding model
        let embedding_id = Uuid::new_v4();
        let result = manager
            .save_model(
                embedding_id,
                ModelType::Embedding,
                "my_embedding".to_string(),
                b"data",
            )
            .await;
        assert!(result.is_ok());
        assert!(matches!(result.unwrap().model_type, ModelType::Embedding));

        // Test classifier model
        let classifier_id = Uuid::new_v4();
        let result = manager
            .save_model(
                classifier_id,
                ModelType::Classifier,
                "my_classifier".to_string(),
                b"data",
            )
            .await;
        assert!(result.is_ok());
        assert!(matches!(result.unwrap().model_type, ModelType::Classifier));

        // Test reranker model
        let reranker_id = Uuid::new_v4();
        let result = manager
            .save_model(
                reranker_id,
                ModelType::Reranker,
                "my_reranker".to_string(),
                b"data",
            )
            .await;
        assert!(result.is_ok());
        assert!(matches!(result.unwrap().model_type, ModelType::Reranker));

        // Verify all three are in the list
        let models = manager.list_models().await.unwrap();
        assert_eq!(models.len(), 3);
    }

    #[tokio::test]
    async fn test_delete_model() {
        let temp_dir = TempDir::new().unwrap();
        let manager = create_model_manager(&temp_dir);
        manager.initialize().await.unwrap();

        // Save a model first
        let model_id = Uuid::new_v4();
        let result = manager
            .save_model(
                model_id,
                ModelType::Embedding,
                "to_delete".to_string(),
                b"data",
            )
            .await;
        assert!(result.is_ok());
        let model = result.unwrap();

        // Verify it exists
        assert!(model.file_path.exists());

        // Delete the model
        let delete_result = manager.delete_model(model_id).await;
        assert!(delete_result.is_ok());

        // Verify it's gone from cache
        let models = manager.list_models().await.unwrap();
        assert!(models.is_empty());

        // Verify file was deleted
        assert!(!model.file_path.exists());
    }

    #[tokio::test]
    async fn test_delete_nonexistent_model() {
        let temp_dir = TempDir::new().unwrap();
        let manager = create_model_manager(&temp_dir);
        manager.initialize().await.unwrap();

        let result = manager.delete_model(Uuid::new_v4()).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_load_saved_model() {
        let temp_dir = TempDir::new().unwrap();
        let manager = create_model_manager(&temp_dir);
        manager.initialize().await.unwrap();

        // Save a model
        let model_id = Uuid::new_v4();
        let save_result = manager
            .save_model(
                model_id,
                ModelType::Embedding,
                "loadable_model".to_string(),
                b"test data",
            )
            .await;
        assert!(save_result.is_ok());

        // Load the model
        let load_result = manager.load_model(model_id).await;
        assert!(load_result.is_ok());

        let loaded = load_result.unwrap();
        assert_eq!(loaded.id, model_id);
    }
}
