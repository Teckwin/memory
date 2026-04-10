use uuid::Uuid;
//! Core traits for the memory system

use crate::error::MemoryError;
use crate::types::*;
use async_trait::async_trait;

/// Core memory operations trait
#[async_trait]
pub trait MemoryApi: Send + Sync {
    /// Add a new memory
    async fn add(&self, memory: MemoryEntry) -> Result<MemoryId, MemoryError>;

    /// Get memory by ID
    async fn get(&self, id: MemoryId) -> Result<MemoryEntry, MemoryError>;

    /// Update an existing memory
    async fn update(&self, memory: MemoryEntry) -> Result<(), MemoryError>;

    /// Delete memory by ID
    async fn delete(&self, id: MemoryId) -> Result<(), MemoryError>;

    /// List memories with optional filters
    async fn list(&self, workspace_id: WorkspaceId, query: SearchQuery) -> Result<Vec<SearchResult>, MemoryError>;

    /// Batch add memories
    async fn batch_add(&self, memories: Vec<MemoryEntry>) -> Result<BatchResult, MemoryError>;

    /// Batch delete memories
    async fn batch_delete(&self, ids: Vec<MemoryId>) -> Result<BatchResult, MemoryError>;
}

/// Search operations trait
#[async_trait]
pub trait SearchApi: Send + Sync {
    /// Full-text search
    async fn search(&self, query: SearchQuery) -> Result<Vec<SearchResult>, MemoryError>;

    /// Vector similarity search
    async fn vector_search(
        &self,
        workspace_id: WorkspaceId,
        embedding: &[f32],
        limit: usize,
    ) -> Result<Vec<SearchResult>, MemoryError>;

    /// Hybrid search (combining text and vector)
    async fn hybrid_search(
        &self,
        workspace_id: WorkspaceId,
        text: &str,
        embedding: &[f32],
        limit: usize,
    ) -> Result<Vec<SearchResult>, MemoryError>;
}

/// Workspace operations trait
#[async_trait]
pub trait WorkspaceApi: Send + Sync {
    /// Create a new workspace
    async fn create(&self, workspace: Workspace) -> Result<WorkspaceId, MemoryError>;

    /// Get workspace by ID
    async fn get(&self, id: WorkspaceId) -> Result<Workspace, MemoryError>;

    /// Update workspace
    async fn update(&self, workspace: Workspace) -> Result<(), MemoryError>;

    /// Delete workspace (and all its memories)
    async fn delete(&self, id: WorkspaceId) -> Result<(), MemoryError>;

    /// List all workspaces
    async fn list(&self) -> Result<Vec<Workspace>, MemoryError>;

    /// Get workspace statistics
    async fn stats(&self, id: WorkspaceId) -> Result<MemoryStats, MemoryError>;
}

/// Lifecycle management trait
#[async_trait]
pub trait LifecycleApi: Send + Sync {
    /// Transition memory to new status
    async fn transition(&self, id: MemoryId, new_status: MemoryStatus) -> Result<(), MemoryError>;

    /// Get memories that need status transition
    async fn get_transition_candidates(&self, status: MemoryStatus) -> Result<Vec<MemoryId>, MemoryError>;

    /// Run lifecycle transition for all eligible memories
    async fn run_transitions(&self) -> Result<BatchResult, MemoryError>;

    /// Archive old memories
    async fn archive(&self, workspace_id: WorkspaceId) -> Result<BatchResult, MemoryError>;
}

/// Training operations trait
#[async_trait]
pub trait TrainApi: Send + Sync {
    /// Prepare training data
    async fn prepare_data(
        &self,
        workspace_id: WorkspaceId,
        params: TrainParams,
    ) -> Result<TrainData, MemoryError>;

    /// Run training
    async fn train(&self, data: TrainData, params: TrainParams) -> Result<TrainResult, MemoryError>;

    /// Get available models
    async fn list_models(&self) -> Result<Vec<TrainModel>, MemoryError>;

    /// Load a model
    async fn load_model(&self, model_id: Uuid) -> Result<LoadedModel, MemoryError>;

    /// Generate embeddings
    async fn generate_embeddings(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, MemoryError>;
}

/// Training parameters
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

/// Model type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelType {
    Embedding,
    Classifier,
    Reranker,
}

/// Training data
#[derive(Debug, Clone)]
pub struct TrainData {
    pub texts: Vec<String>,
    pub labels: Option<Vec<String>>,
    pub embeddings: Option<Vec<Vec<f32>>>,
}

/// Training result
#[derive(Debug, Clone)]
pub struct TrainResult {
    pub model_id: Uuid,
    pub metrics: TrainMetrics,
    pub output_path: std::path::PathBuf,
}

/// Training metrics
#[derive(Debug, Clone, Default)]
pub struct TrainMetrics {
    pub loss: f32,
    pub accuracy: Option<f32>,
    pub f1_score: Option<f32>,
}

/// Available training model
#[derive(Debug, Clone)]
pub struct TrainModel {
    pub id: Uuid,
    pub name: String,
    pub model_type: ModelType,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub file_path: std::path::PathBuf,
}

/// Loaded model handle
#[derive(Debug)]
pub struct LoadedModel {
    pub id: Uuid,
    pub model: Box<dyn std::any::Any + Send + Sync>,
}