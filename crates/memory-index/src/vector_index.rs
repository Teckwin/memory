//! HNSW Vector Index Implementation
//!
//! This module provides vector similarity search using the HNSW algorithm.

use crate::error::IndexError;
use memory_core::{MemoryEntry, MemoryId, WorkspaceId};
use space::{MetricPoint, Neighbor};
use std::collections::HashMap;
use tokio::task;
use tracing::{debug, info};

/// Configuration for HNSW index
#[derive(Debug, Clone)]
pub struct HnswConfig {
    /// Maximum number of connections per element
    pub m: usize,
    /// Maximum number of elements to keep in the search
    pub ef_construction: usize,
    /// Number of neighbors to search during retrieval
    pub ef_search: usize,
    /// Number of dimensions in the embedding vectors
    pub dimension: usize,
}

impl Default for HnswConfig {
    fn default() -> Self {
        Self {
            m: 16,
            ef_construction: 200,
            ef_search: 50,
            dimension: 384, // Default embedding dimension
        }
    }
}

/// Custom wrapper for f32 vectors implementing MetricPoint for cosine similarity
#[derive(Clone, Debug)]
pub struct EmbeddingVector(pub Vec<f32>);

impl EmbeddingVector {
    pub fn new(data: Vec<f32>) -> Self {
        Self(data)
    }

    pub fn as_slice(&self) -> &[f32] {
        &self.0
    }
}

impl MetricPoint for EmbeddingVector {
    fn distance(&self, other: &Self) -> u32 {
        // Compute cosine distance (1 - cosine_similarity)
        // Using space::f32_metric to convert f32 to u32
        let dot: f32 = self.0.iter().zip(other.0.iter()).map(|(a, b)| a * b).sum();
        let mag1: f32 = self.0.iter().map(|x| x * x).sum::<f32>().sqrt();
        let mag2: f32 = other.0.iter().map(|x| x * x).sum::<f32>().sqrt();

        if mag1 == 0.0 || mag2 == 0.0 {
            return u32::MAX;
        }

        let similarity = dot / (mag1 * mag2);
        let distance = 1.0 - similarity;

        // Convert to integer representation
        space::f32_metric(distance)
    }
}

/// Vector search result with additional metadata
#[derive(Debug, Clone)]
pub struct VectorSearchResult {
    pub memory_id: MemoryId,
    pub score: f32,
    pub workspace_id: WorkspaceId,
}

/// HNSW Vector Index
///
/// Provides approximate nearest neighbor search for memory embeddings.
pub struct VectorIndex {
    config: HnswConfig,
    /// In-memory HNSW structure
    hnsw: Option<hnsw::HNSW<EmbeddingVector>>,
    /// Mapping from vector index id to memory id
    id_mapping: HashMap<usize, MemoryId>,
    /// Reverse mapping from memory id to vector index id
    memory_to_index: HashMap<MemoryId, usize>,
    /// Workspace isolation
    workspace_ids: HashMap<MemoryId, WorkspaceId>,
    /// Total number of indexed vectors
    count: usize,
}

impl VectorIndex {
    /// Create a new VectorIndex with the given configuration
    pub fn new(config: HnswConfig) -> Self {
        info!(
            "Creating new VectorIndex with dimension: {}",
            config.dimension
        );
        Self {
            config,
            hnsw: None,
            id_mapping: HashMap::new(),
            memory_to_index: HashMap::new(),
            workspace_ids: HashMap::new(),
            count: 0,
        }
    }

    /// Initialize the HNSW structure
    pub fn initialize(&mut self) -> Result<(), IndexError> {
        let hnsw = hnsw::HNSW::new();
        self.hnsw = Some(hnsw);
        info!("VectorIndex initialized");
        Ok(())
    }

    /// Check if index is initialized
    pub fn is_initialized(&self) -> bool {
        self.hnsw.is_some()
    }

    /// Add a memory entry with its embedding to the index
    pub async fn add(&mut self, memory: &MemoryEntry) -> Result<(), IndexError> {
        let embedding = memory
            .embedding
            .as_ref()
            .ok_or_else(|| IndexError::OperationFailed("Memory has no embedding".to_string()))?;

        // Validate dimension
        if embedding.len() != self.config.dimension {
            return Err(IndexError::InvalidDimension {
                expected: self.config.dimension,
                got: embedding.len(),
            });
        }

        let memory_id = memory.id;
        let workspace_id = memory.workspace_id;
        let embedding_vec = embedding.clone();

        // Spawn blocking for HNSW operations
        let hnsw_opt = self.hnsw.take();

        let (hnsw, vector_id) = task::spawn_blocking(move || {
            let mut hnsw = hnsw_opt.expect("HNSW not initialized");

            let emb = EmbeddingVector::new(embedding_vec);
            let id = hnsw.insert(emb, &mut hnsw::Searcher::new());

            (hnsw, id as usize)
        })
        .await
        .map_err(|e| IndexError::OperationFailed(e.to_string()))?;

        self.hnsw = Some(hnsw);

        self.id_mapping.insert(vector_id, memory_id);
        self.memory_to_index.insert(memory_id, vector_id);
        self.workspace_ids.insert(memory_id, workspace_id);
        self.count += 1;

        debug!("Added memory {} to vector index", memory_id);
        Ok(())
    }

    /// Add multiple memory entries in batch
    pub async fn batch_add(&mut self, memories: &[MemoryEntry]) -> Result<(), IndexError> {
        for memory in memories {
            self.add(memory).await?;
        }
        Ok(())
    }

    /// Remove a memory from the index
    pub async fn remove(&mut self, memory_id: MemoryId) -> Result<(), IndexError> {
        let vector_id = self
            .memory_to_index
            .remove(&memory_id)
            .ok_or_else(|| IndexError::NotFound(format!("Memory {} not in index", memory_id)))?;

        self.id_mapping.remove(&vector_id);
        self.workspace_ids.remove(&memory_id);
        self.count = self.count.saturating_sub(1);

        debug!("Removed memory {} from vector index", memory_id);
        Ok(())
    }

    /// Search for similar vectors
    pub async fn search(
        &self,
        query_vector: &[f32],
        workspace_id: Option<WorkspaceId>,
        limit: usize,
    ) -> Result<Vec<VectorSearchResult>, IndexError> {
        // Validate query dimension
        if query_vector.len() != self.config.dimension {
            return Err(IndexError::InvalidDimension {
                expected: self.config.dimension,
                got: query_vector.len(),
            });
        }

        let hnsw_ref = self
            .hnsw
            .as_ref()
            .ok_or_else(|| IndexError::OperationFailed("HNSW index not initialized".to_string()))?;

        let query_vec = query_vector.to_vec();
        let limit = limit.min(1000); // Cap at 1000 results

        // Clone the HNSW for the blocking task
        let hnsw_clone = hnsw_ref.clone();
        let id_mapping = self.id_mapping.clone();
        let workspace_ids = self.workspace_ids.clone();

        let results: Vec<(usize, u32)> = task::spawn_blocking(move || {
            let mut searcher = hnsw::Searcher::new();
            let query = EmbeddingVector::new(query_vec);
            let mut neighbors = [Neighbor::invalid(); 1000];

            hnsw_clone.nearest(&query, limit, &mut searcher, &mut neighbors);

            neighbors
                .iter()
                .take(limit)
                .filter(|n| n.index != usize::MAX)
                .map(|n| (n.index, n.distance))
                .collect()
        })
        .await
        .map_err(|e| IndexError::OperationFailed(e.to_string()))?;

        let search_results: Vec<VectorSearchResult> = results
            .into_iter()
            .filter_map(|(id, distance)| {
                let memory_id = id_mapping.get(&id)?;
                let ws_id = workspace_ids.get(memory_id)?;

                // Filter by workspace if specified
                if let Some(ref filter_ws) = workspace_id {
                    if ws_id != filter_ws {
                        return None;
                    }
                }

                // Convert distance to similarity score
                // Distance is u32, lower is better
                let similarity = 1.0 - (distance as f32 / u32::MAX as f32);

                Some(VectorSearchResult {
                    memory_id: *memory_id,
                    score: similarity,
                    workspace_id: *ws_id,
                })
            })
            .collect();

        debug!("Vector search returned {} results", search_results.len());
        Ok(search_results)
    }

    /// Get the number of indexed vectors
    pub fn len(&self) -> usize {
        self.count
    }

    /// Check if the index is empty
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// Get the configuration
    pub fn config(&self) -> &HnswConfig {
        &self.config
    }
}

/// Builder for VectorIndex with fluent configuration
pub struct VectorIndexBuilder {
    config: HnswConfig,
}

impl VectorIndexBuilder {
    pub fn new() -> Self {
        Self {
            config: HnswConfig::default(),
        }
    }

    pub fn with_dimension(mut self, dimension: usize) -> Self {
        self.config.dimension = dimension;
        self
    }

    pub fn with_m(mut self, m: usize) -> Self {
        self.config.m = m;
        self
    }

    pub fn with_ef_construction(mut self, ef: usize) -> Self {
        self.config.ef_construction = ef;
        self
    }

    pub fn with_ef_search(mut self, ef: usize) -> Self {
        self.config.ef_search = ef;
        self
    }

    pub fn build(self) -> VectorIndex {
        VectorIndex::new(self.config)
    }
}

impl Default for VectorIndexBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use memory_core::{MemoryContent, MemoryMetadata, MemoryStatus};
    use uuid::Uuid;

    fn create_test_memory(workspace_id: WorkspaceId, dimension: usize) -> MemoryEntry {
        let embedding: Vec<f32> = (0..dimension)
            .map(|i| (i as f32) / dimension as f32)
            .collect();
        MemoryEntry {
            id: Uuid::new_v4(),
            workspace_id,
            content: MemoryContent::Text("Test memory content".to_string()),
            embedding: Some(embedding),
            metadata: MemoryMetadata {
                source: memory_core::MemorySource::System {
                    source_type: "test".to_string(),
                },
                custom_fields: std::collections::HashMap::new(),
                tags: vec!["test".to_string()],
                importance: 0.5,
            },
            status: MemoryStatus::Active,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            access_count: 0,
            last_accessed: None,
        }
    }

    #[tokio::test]
    async fn test_vector_index_creation() {
        let config = HnswConfig {
            dimension: 128,
            ..Default::default()
        };
        let mut index = VectorIndex::new(config);
        index.initialize().unwrap();
        assert!(index.is_initialized());
    }

    #[tokio::test]
    async fn test_add_memory() {
        let config = HnswConfig {
            dimension: 128,
            ..Default::default()
        };
        let mut index = VectorIndex::new(config);
        index.initialize().unwrap();

        let workspace_id = Uuid::new_v4();
        let memory = create_test_memory(workspace_id, 128);

        index.add(&memory).await.unwrap();
        assert_eq!(index.len(), 1);
    }

    #[tokio::test]
    async fn test_add_memory_wrong_dimension() {
        let config = HnswConfig {
            dimension: 128,
            ..Default::default()
        };
        let mut index = VectorIndex::new(config);
        index.initialize().unwrap();

        let workspace_id = Uuid::new_v4();
        let memory = create_test_memory(workspace_id, 256); // Wrong dimension

        let result = index.add(&memory).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_search() {
        let config = HnswConfig {
            dimension: 128,
            ..Default::default()
        };
        let mut index = VectorIndex::new(config);
        index.initialize().unwrap();

        let workspace_id = Uuid::new_v4();

        // Add 3 memories with similar embeddings
        for i in 0..3 {
            let mut memory = create_test_memory(workspace_id, 128);
            // Make embeddings slightly different
            if let Some(emb) = memory.embedding.as_mut() {
                emb[0] = i as f32;
            }
            index.add(&memory).await.unwrap();
        }

        // Search with a query vector similar to the indexed ones
        let query: Vec<f32> = vec![0.5; 128];
        let results = index.search(&query, Some(workspace_id), 10).await.unwrap();

        assert!(!results.is_empty());
    }

    #[tokio::test]
    async fn test_workspace_filter() {
        let config = HnswConfig {
            dimension: 128,
            ..Default::default()
        };
        let mut index = VectorIndex::new(config);
        index.initialize().unwrap();

        let ws1 = Uuid::new_v4();
        let ws2 = Uuid::new_v4();

        let mem1 = create_test_memory(ws1, 128);
        let mem2 = create_test_memory(ws2, 128);

        index.add(&mem1).await.unwrap();
        index.add(&mem2).await.unwrap();

        // Search only in ws1
        let query: Vec<f32> = vec![0.5; 128];
        let results = index.search(&query, Some(ws1), 10).await.unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].workspace_id, ws1);
    }

    #[tokio::test]
    async fn test_remove_memory() {
        let config = HnswConfig {
            dimension: 128,
            ..Default::default()
        };
        let mut index = VectorIndex::new(config);
        index.initialize().unwrap();

        let workspace_id = Uuid::new_v4();
        let memory = create_test_memory(workspace_id, 128);
        let memory_id = memory.id;

        index.add(&memory).await.unwrap();
        assert_eq!(index.len(), 1);

        index.remove(memory_id).await.unwrap();
        assert_eq!(index.len(), 0);
    }

    #[tokio::test]
    async fn test_batch_add() {
        let config = HnswConfig {
            dimension: 128,
            ..Default::default()
        };
        let mut index = VectorIndex::new(config);
        index.initialize().unwrap();

        let workspace_id = Uuid::new_v4();
        let memories: Vec<_> = (0..5)
            .map(|_| create_test_memory(workspace_id, 128))
            .collect();

        index.batch_add(&memories).await.unwrap();
        assert_eq!(index.len(), 5);
    }

    #[test]
    fn test_vector_index_builder() {
        let index = VectorIndexBuilder::new()
            .with_dimension(256)
            .with_m(32)
            .with_ef_construction(300)
            .with_ef_search(100)
            .build();

        assert_eq!(index.config().dimension, 256);
        assert_eq!(index.config().m, 32);
    }
}
