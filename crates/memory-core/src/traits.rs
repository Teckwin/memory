//! Core traits for the memory system

use crate::error::MemoryError;
use crate::types::*;
use async_trait::async_trait;
use uuid::Uuid;

#[async_trait]
pub trait MemoryApi: Send + Sync {
    async fn add(&self, memory: MemoryEntry) -> Result<MemoryId, MemoryError>;
    async fn get(&self, id: MemoryId) -> Result<MemoryEntry, MemoryError>;
    async fn update(&self, memory: MemoryEntry) -> Result<(), MemoryError>;
    async fn delete(&self, id: MemoryId) -> Result<(), MemoryError>;
    async fn list(
        &self,
        workspace_id: WorkspaceId,
        query: SearchQuery,
    ) -> Result<Vec<SearchResult>, MemoryError>;
    async fn batch_add(&self, memories: Vec<MemoryEntry>) -> Result<BatchResult, MemoryError>;
    async fn batch_delete(&self, ids: Vec<MemoryId>) -> Result<BatchResult, MemoryError>;
}

#[async_trait]
pub trait SearchApi: Send + Sync {
    async fn search(&self, query: SearchQuery) -> Result<Vec<SearchResult>, MemoryError>;
    async fn vector_search(
        &self,
        workspace_id: WorkspaceId,
        embedding: &[f32],
        limit: usize,
    ) -> Result<Vec<SearchResult>, MemoryError>;
    async fn hybrid_search(
        &self,
        workspace_id: WorkspaceId,
        text: &str,
        embedding: &[f32],
        limit: usize,
    ) -> Result<Vec<SearchResult>, MemoryError>;
}

#[async_trait]
pub trait WorkspaceApi: Send + Sync {
    async fn create(&self, workspace: Workspace) -> Result<WorkspaceId, MemoryError>;
    async fn get(&self, id: WorkspaceId) -> Result<Workspace, MemoryError>;
    async fn update(&self, workspace: Workspace) -> Result<(), MemoryError>;
    async fn delete(&self, id: WorkspaceId) -> Result<(), MemoryError>;
    async fn list(&self) -> Result<Vec<Workspace>, MemoryError>;
    async fn stats(&self, id: WorkspaceId) -> Result<MemoryStats, MemoryError>;
}

#[async_trait]
pub trait LifecycleApi: Send + Sync {
    async fn transition(&self, id: MemoryId, new_status: MemoryStatus) -> Result<(), MemoryError>;
    async fn get_transition_candidates(
        &self,
        status: MemoryStatus,
    ) -> Result<Vec<MemoryId>, MemoryError>;
    async fn run_transitions(&self) -> Result<BatchResult, MemoryError>;
    async fn archive(&self, workspace_id: WorkspaceId) -> Result<BatchResult, MemoryError>;
}

#[async_trait]
pub trait TrainApi: Send + Sync {
    async fn prepare_data(
        &self,
        workspace_id: WorkspaceId,
        params: TrainParams,
    ) -> Result<TrainData, MemoryError>;
    async fn train(&self, data: TrainData, params: TrainParams)
        -> Result<TrainResult, MemoryError>;
    async fn list_models(&self) -> Result<Vec<TrainModel>, MemoryError>;
    async fn load_model(&self, model_id: Uuid) -> Result<LoadedModel, MemoryError>;
    async fn generate_embeddings(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, MemoryError>;
}

#[derive(Debug, Clone)]
pub struct TrainParams {
    pub model_type: ModelType,
    pub epochs: u32,
    pub batch_size: u32,
    pub learning_rate: f32,
    pub output_dir: std::path::PathBuf,
}

impl Default for TrainParams {
    fn default() -> Self {
        Self {
            model_type: ModelType::Embedding,
            epochs: 10,
            batch_size: 32,
            learning_rate: 0.001,
            output_dir: std::path::PathBuf::from("models"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelType {
    Embedding,
    Classifier,
    Reranker,
}

#[derive(Debug, Clone)]
pub struct TrainData {
    pub texts: Vec<String>,
    pub labels: Option<Vec<String>>,
    pub embeddings: Option<Vec<Vec<f32>>>,
}

#[derive(Debug, Clone)]
pub struct TrainResult {
    pub model_id: Uuid,
    pub metrics: TrainMetrics,
    pub output_path: std::path::PathBuf,
}

#[derive(Debug, Clone, Default)]
pub struct TrainMetrics {
    pub loss: f32,
    pub accuracy: Option<f32>,
    pub f1_score: Option<f32>,
}

#[derive(Debug, Clone)]
pub struct TrainModel {
    pub id: Uuid,
    pub name: String,
    pub model_type: ModelType,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub file_path: std::path::PathBuf,
}

#[derive(Debug)]
pub struct LoadedModel {
    pub id: Uuid,
    pub model: Box<dyn std::any::Any + Send + Sync>,
}

#[cfg(test)]
mod tests {
    use super::*;

    // ==================== TrainParams Tests ====================

    #[test]
    fn test_train_params_default() {
        let params = TrainParams::default();

        assert_eq!(params.model_type, ModelType::Embedding);
        assert_eq!(params.epochs, 10);
        assert_eq!(params.batch_size, 32);
        assert_eq!(params.learning_rate, 0.001);
        assert_eq!(params.output_dir, std::path::PathBuf::from("models"));
    }

    #[test]
    fn test_train_params_custom() {
        let params = TrainParams {
            model_type: ModelType::Classifier,
            epochs: 50,
            batch_size: 64,
            learning_rate: 0.0001,
            output_dir: std::path::PathBuf::from("custom_models"),
        };

        assert_eq!(params.model_type, ModelType::Classifier);
        assert_eq!(params.epochs, 50);
        assert_eq!(params.batch_size, 64);
        assert_eq!(params.learning_rate, 0.0001);
        assert_eq!(params.output_dir, std::path::PathBuf::from("custom_models"));
    }

    // ==================== TrainMetrics Tests ====================

    #[test]
    fn test_train_metrics_default() {
        let metrics = TrainMetrics::default();

        assert_eq!(metrics.loss, 0.0);
        assert!(metrics.accuracy.is_none());
        assert!(metrics.f1_score.is_none());
    }

    #[test]
    fn test_train_metrics_with_values() {
        let metrics = TrainMetrics {
            loss: 0.25,
            accuracy: Some(0.92),
            f1_score: Some(0.90),
        };

        assert_eq!(metrics.loss, 0.25);
        assert_eq!(metrics.accuracy, Some(0.92));
        assert_eq!(metrics.f1_score, Some(0.90));
    }

    // ==================== ModelType Tests ====================

    #[test]
    fn test_model_type_variants() {
        // Test Embedding variant
        let embedding = ModelType::Embedding;
        assert_eq!(format!("{:?}", embedding), "Embedding");

        // Test Classifier variant
        let classifier = ModelType::Classifier;
        assert_eq!(format!("{:?}", classifier), "Classifier");

        // Test Reranker variant
        let reranker = ModelType::Reranker;
        assert_eq!(format!("{:?}", reranker), "Reranker");
    }

    #[test]
    fn test_model_type_equality() {
        let m1 = ModelType::Embedding;
        let m2 = ModelType::Embedding;
        let m3 = ModelType::Classifier;

        assert_eq!(m1, m2);
        assert_ne!(m1, m3);
    }

    #[test]
    fn test_model_type_clone() {
        let original = ModelType::Reranker;
        let cloned = original.clone();

        assert_eq!(original, cloned);
    }

    #[test]
    fn test_model_type_copy() {
        // ModelType is Copy because it's a simple enum with no heap data
        let original = ModelType::Embedding;
        let copied = original;

        assert_eq!(original, copied);
    }
}
