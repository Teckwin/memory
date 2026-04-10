//! Memory client implementation

use async_trait::async_trait;
use memory_core::{
    MemoryApi, SearchApi, WorkspaceApi, 
    MemoryEntry, MemoryId, WorkspaceId, Workspace,
    SearchQuery, SearchResult, BatchResult, MemoryStats,
    MemoryError,
};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, debug, warn};

use crate::error::ApiError;

/// Configuration for MemoryClient
#[derive(Debug, Clone)]
pub struct ClientConfig {
    pub storage_enabled: bool,
    pub index_enabled: bool,
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            storage_enabled: true,
            index_enabled: true,
        }
    }
}

/// Memory client that implements core traits
/// 
/// This client wraps underlying storage and index implementations,
/// providing a unified API for memory operations.
pub struct MemoryClient {
    config: ClientConfig,
    // Storage and index backends would be injected here
    // For now, we use in-memory storage as a placeholder
    memories: Arc<RwLock<std::collections::HashMap<MemoryId, MemoryEntry>>>,
    workspaces: Arc<RwLock<std::collections::HashMap<WorkspaceId, Workspace>>>,
}

impl MemoryClient {
    /// Create a new MemoryClient with default configuration
    pub fn new() -> Self {
        Self::with_config(ClientConfig::default())
    }

    /// Create a new MemoryClient with custom configuration
    pub fn with_config(config: ClientConfig) -> Self {
        info!("Initializing MemoryClient with config: {:?}", config);
        Self {
            config,
            memories: Arc::new(RwLock::new(std::collections::HashMap::new())),
            workspaces: Arc::new(RwLock::new(std::collections::HashMap::new())),
        }
    }

    /// Get client configuration
    pub fn config(&self) -> &ClientConfig {
        &self.config
    }

    /// Check if storage is enabled
    pub fn is_storage_enabled(&self) -> bool {
        self.config.storage_enabled
    }

    /// Check if index is enabled
    pub fn is_index_enabled(&self) -> bool {
        self.config.index_enabled
    }
}

impl Default for MemoryClient {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl MemoryApi for MemoryClient {
    /// Add a new memory
    async fn add(&self, memory: MemoryEntry) -> Result<MemoryId, MemoryError> {
        debug!("Adding memory: {}", memory.id);
        
        if !self.is_storage_enabled() {
            return Err(MemoryError::InvalidOperation("Storage is disabled".to_string()));
        }

        let id = memory.id;
        let mut memories = self.memories.write().await;
        memories.insert(id, memory);
        
        info!("Memory added successfully: {}", id);
        Ok(id)
    }

    /// Get memory by ID
    async fn get(&self, id: MemoryId) -> Result<MemoryEntry, MemoryError> {
        debug!("Getting memory: {}", id);
        
        if !self.is_storage_enabled() {
            return Err(MemoryError::InvalidOperation("Storage is disabled".to_string()));
        }

        let memories = self.memories.read().await;
        memories
            .get(&id)
            .cloned()
            .ok_or_else(|| MemoryError::NotFound(id.to_string()))
    }

    /// Update an existing memory
    async fn update(&self, memory: MemoryEntry) -> Result<(), MemoryError> {
        debug!("Updating memory: {}", memory.id);
        
        if !self.is_storage_enabled() {
            return Err(MemoryError::InvalidOperation("Storage is disabled".to_string()));
        }

        let mut memories = self.memories.write().await;
        if memories.contains_key(&memory.id) {
            memories.insert(memory.id, memory);
            Ok(())
        } else {
            Err(MemoryError::NotFound(memory.id.to_string()))
        }
    }

    /// Delete memory by ID
    async fn delete(&self, id: MemoryId) -> Result<(), MemoryError> {
        debug!("Deleting memory: {}", id);
        
        if !self.is_storage_enabled() {
            return Err(MemoryError::InvalidOperation("Storage is disabled".to_string()));
        }

        let mut memories = self.memories.write().await;
        if memories.remove(&id).is_some() {
            info!("Memory deleted: {}", id);
            Ok(())
        } else {
            Err(MemoryError::NotFound(id.to_string()))
        }
    }

    /// List memories with optional filters
    async fn list(
        &self, 
        workspace_id: WorkspaceId, 
        query: SearchQuery
    ) -> Result<Vec<SearchResult>, MemoryError> {
        debug!("Listing memories for workspace: {}", workspace_id);
        
        if !self.is_storage_enabled() {
            return Err(MemoryError::InvalidOperation("Storage is disabled".to_string()));
        }

        let memories = self.memories.read().await;
        let mut results: Vec<SearchResult> = memories
            .values()
            .filter(|m| m.workspace_id == workspace_id)
            .filter(|m| {
                // Apply status filter if specified
                if let Some(status) = &query.status {
                    m.status == *status
                } else {
                    true
                }
            })
            .filter(|m| {
                // Apply tag filter if specified
                if let Some(tags) = &query.tags {
                    tags.iter().any(|tag| m.metadata.tags.contains(tag))
                } else {
                    true
                }
            })
            .skip(query.offset)
            .take(query.limit)
            .map(|m| SearchResult {
                memory: m.clone(),
                score: 1.0,
                highlights: vec![],
            })
            .collect();

        // Simple text search if query text is provided
        if let Some(text) = &query.text {
            let search_text = text.to_lowercase();
            results.retain(|r| {
                r.memory.content.as_text()
                    .map(|t| t.to_lowercase().contains(&search_text))
                    .unwrap_or(false)
            });
        }

        Ok(results)
    }

    /// Batch add memories
    async fn batch_add(&self, memories: Vec<MemoryEntry>) -> Result<BatchResult, MemoryError> {
        info!("Batch adding {} memories", memories.len());
        
        if !self.is_storage_enabled() {
            return Err(MemoryError::InvalidOperation("Storage is disabled".to_string()));
        }

        let mut result = BatchResult::new();
        let mut storage = self.memories.write().await;

        for memory in memories {
            if storage.insert(memory.id, memory).is_some() {
                result.add_success();
            } else {
                result.add_failure("Failed to insert memory".to_string());
            }
        }

        Ok(result)
    }

    /// Batch delete memories
    async fn batch_delete(&self, ids: Vec<MemoryId>) -> Result<BatchResult, MemoryError> {
        info!("Batch deleting {} memories", ids.len());
        
        if !self.is_storage_enabled() {
            return Err(MemoryError::InvalidOperation("Storage is disabled".to_string()));
        }

        let mut result = BatchResult::new();
        let mut storage = self.memories.write().await;

        for id in ids {
            if storage.remove(&id).is_some() {
                result.add_success();
            } else {
                result.add_failure(format!("Memory not found: {}", id));
            }
        }

        Ok(result)
    }
}

#[async_trait]
impl SearchApi for MemoryClient {
    /// Full-text search
    async fn search(&self, query: SearchQuery) -> Result<Vec<SearchResult>, MemoryError> {
        debug!("Full-text search: {:?}", query.text);
        
        if !self.is_index_enabled() {
            return Err(MemoryError::IndexError("Index is disabled".to_string()));
        }

        // Get all workspaces if not specified
        let workspace_ids: Vec<WorkspaceId> = if let Some(ws_id) = query.workspace_id {
            vec![ws_id]
        } else {
            let workspaces = self.workspaces.read().await;
            workspaces.keys().copied().collect()
        };

        let mut all_results = Vec::new();
        for ws_id in workspace_ids {
            let results = self.list(ws_id, query.clone()).await?;
            all_results.extend(results);
        }

        // Sort by score descending
        all_results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));

        Ok(all_results)
    }

    /// Vector similarity search
    async fn vector_search(
        &self,
        workspace_id: WorkspaceId,
        embedding: &[f32],
        limit: usize,
    ) -> Result<Vec<SearchResult>, MemoryError> {
        debug!("Vector search for workspace: {}, limit: {}", workspace_id, limit);
        
        if !self.is_index_enabled() {
            return Err(MemoryError::IndexError("Index is disabled".to_string()));
        }

        let memories = self.memories.read().await;
        
        // Calculate cosine similarity for memories with embeddings
        let mut results: Vec<SearchResult> = memories
            .values()
            .filter(|m| m.workspace_id == workspace_id)
            .filter_map(|m| {
                m.embedding.as_ref().map(|e| {
                    let score = cosine_similarity(embedding, e);
                    SearchResult {
                        memory: m.clone(),
                        score,
                        highlights: vec![],
                    }
                })
            })
            .collect();

        // Sort by score descending and limit
        results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        results.truncate(limit);

        Ok(results)
    }

    /// Hybrid search (combining text and vector)
    async fn hybrid_search(
        &self,
        workspace_id: WorkspaceId,
        text: &str,
        embedding: &[f32],
        limit: usize,
    ) -> Result<Vec<SearchResult>, MemoryError> {
        debug!("Hybrid search for workspace: {}", workspace_id);
        
        if !self.is_index_enabled() {
            return Err(MemoryError::IndexError("Index is disabled".to_string()));
        }

        // Get text search results
        let text_query = SearchQuery {
            text: Some(text.to_string()),
            workspace_id: Some(workspace_id),
            limit,
            ..Default::default()
        };
        let text_results = self.search(text_query).await?;

        // Get vector search results
        let vector_results = self.vector_search(workspace_id, embedding, limit).await?;

        // Merge and deduplicate results (simple average of scores)
        let mut merged: std::collections::HashMap<MemoryId, (f32, SearchResult)> = std::collections::HashMap::new();
        
        for r in text_results {
            let entry = merged.entry(r.memory.id).or_insert((0.0, r.clone()));
            entry.0 += r.score;
            entry.1 = r;
        }
        
        for r in vector_results {
            let entry = merged.entry(r.memory.id).or_insert((0.0, r.clone()));
            entry.0 += r.score;
            if r.score > entry.1.score {
                entry.1 = r;
            }
        }

        // Normalize scores and sort
        let mut results: Vec<_> = merged.into_values()
            .map(|(sum, mut r)| {
                r.score = sum / 2.0;
                r
            })
            .collect();

        results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        results.truncate(limit);

        Ok(results)
    }
}

#[async_trait]
impl WorkspaceApi for MemoryClient {
    /// Create a new workspace
    async fn create(&self, workspace: Workspace) -> Result<WorkspaceId, MemoryError> {
        info!("Creating workspace: {}", workspace.name);
        
        let id = workspace.id;
        let mut workspaces = self.workspaces.write().await;
        workspaces.insert(id, workspace);
        
        Ok(id)
    }

    /// Get workspace by ID
    async fn get(&self, id: WorkspaceId) -> Result<Workspace, MemoryError> {
        debug!("Getting workspace: {}", id);
        
        let workspaces = self.workspaces.read().await;
        workspaces
            .get(&id)
            .cloned()
            .ok_or_else(|| MemoryError::WorkspaceError(format!("Workspace not found: {}", id)))
    }

    /// Update workspace
    async fn update(&self, workspace: Workspace) -> Result<(), MemoryError> {
        debug!("Updating workspace: {}", workspace.id);
        
        let mut workspaces = self.workspaces.write().await;
        if workspaces.contains_key(&workspace.id) {
            workspaces.insert(workspace.id, workspace);
            Ok(())
        } else {
            Err(MemoryError::WorkspaceError(format!("Workspace not found: {}", workspace.id)))
        }
    }

    /// Delete workspace (and all its memories)
    async fn delete(&self, id: WorkspaceId) -> Result<(), MemoryError> {
        info!("Deleting workspace: {}", id);
        
        // Delete all memories in the workspace
        let mut memories = self.memories.write().await;
        memories.retain(|_, m| m.workspace_id != id);
        
        // Delete the workspace
        let mut workspaces = self.workspaces.write().await;
        if workspaces.remove(&id).is_some() {
            Ok(())
        } else {
            Err(MemoryError::WorkspaceError(format!("Workspace not found: {}", id)))
        }
    }

    /// List all workspaces
    async fn list(&self) -> Result<Vec<Workspace>, MemoryError> {
        debug!("Listing all workspaces");
        
        let workspaces = self.workspaces.read().await;
        Ok(workspaces.values().cloned().collect())
    }

    /// Get workspace statistics
    async fn stats(&self, id: WorkspaceId) -> Result<MemoryStats, MemoryError> {
        debug!("Getting stats for workspace: {}", id);
        
        // Verify workspace exists
        let workspaces = self.workspaces.read().await;
        if !workspaces.contains_key(&id) {
            return Err(MemoryError::WorkspaceError(format!("Workspace not found: {}", id)));
        }
        drop(workspaces);

        let memories = self.memories.read().await;
        let workspace_memories: Vec<_> = memories
            .values()
            .filter(|m| m.workspace_id == id)
            .collect();

        let mut stats = MemoryStats::default();
        stats.total_memories = workspace_memories.len() as u64;

        for m in &workspace_memories {
            match m.status {
                memory_core::MemoryStatus::Active => stats.active_count += 1,
                memory_core::MemoryStatus::Cooling => stats.cooling_count += 1,
                memory_core::MemoryStatus::Cold => stats.cold_count += 1,
                memory_core::MemoryStatus::Zombie => stats.zombie_count += 1,
            }
            stats.average_importance += m.metadata.importance;
        }

        if stats.total_memories > 0 {
            stats.average_importance /= stats.total_memories as f32;
        }

        Ok(stats)
    }
}

/// Calculate cosine similarity between two vectors
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }

    let dot_product: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let magnitude_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let magnitude_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

    if magnitude_a == 0.0 || magnitude_b == 0.0 {
        return 0.0;
    }

    dot_product / (magnitude_a * magnitude_b)
}
