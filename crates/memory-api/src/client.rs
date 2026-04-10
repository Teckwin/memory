//! Memory client implementation

use async_trait::async_trait;
use memory_core::{
    BatchResult, MemoryApi, MemoryEntry, MemoryError, MemoryId, MemoryStats, SearchApi,
    SearchQuery, SearchResult, Workspace, WorkspaceApi, WorkspaceId,
};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info};

/// Configuration for MemoryClient
#[derive(Debug, Clone, PartialEq)]
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
    async fn add(&self, memory: MemoryEntry) -> Result<MemoryId, MemoryError> {
        debug!("Adding memory: {}", memory.id);

        if !self.is_storage_enabled() {
            return Err(MemoryError::InvalidOperation(
                "Storage is disabled".to_string(),
            ));
        }

        let id = memory.id;
        let mut memories = self.memories.write().await;
        memories.insert(id, memory);

        info!("Memory added successfully: {}", id);
        Ok(id)
    }

    async fn get(&self, id: MemoryId) -> Result<MemoryEntry, MemoryError> {
        debug!("Getting memory: {}", id);

        if !self.is_storage_enabled() {
            return Err(MemoryError::InvalidOperation(
                "Storage is disabled".to_string(),
            ));
        }

        let memories = self.memories.read().await;
        memories
            .get(&id)
            .cloned()
            .ok_or_else(|| MemoryError::NotFound(id.to_string()))
    }

    async fn update(&self, memory: MemoryEntry) -> Result<(), MemoryError> {
        debug!("Updating memory: {}", memory.id);

        if !self.is_storage_enabled() {
            return Err(MemoryError::InvalidOperation(
                "Storage is disabled".to_string(),
            ));
        }

        let mut memories = self.memories.write().await;
        match memories.entry(memory.id) {
            std::collections::hash_map::Entry::Occupied(mut e) => {
                e.insert(memory);
                Ok(())
            }
            std::collections::hash_map::Entry::Vacant(_) => {
                Err(MemoryError::NotFound(memory.id.to_string()))
            }
        }
    }

    async fn delete(&self, id: MemoryId) -> Result<(), MemoryError> {
        debug!("Deleting memory: {}", id);

        if !self.is_storage_enabled() {
            return Err(MemoryError::InvalidOperation(
                "Storage is disabled".to_string(),
            ));
        }

        let mut memories = self.memories.write().await;
        if memories.remove(&id).is_some() {
            info!("Memory deleted: {}", id);
            Ok(())
        } else {
            Err(MemoryError::NotFound(id.to_string()))
        }
    }

    async fn list(
        &self,
        workspace_id: WorkspaceId,
        query: SearchQuery,
    ) -> Result<Vec<SearchResult>, MemoryError> {
        debug!("Listing memories for workspace: {}", workspace_id);

        if !self.is_storage_enabled() {
            return Err(MemoryError::InvalidOperation(
                "Storage is disabled".to_string(),
            ));
        }

        let memories = self.memories.read().await;
        let mut results: Vec<SearchResult> = memories
            .values()
            .filter(|m| m.workspace_id == workspace_id)
            .filter(|m| {
                if let Some(status) = &query.status {
                    m.status == *status
                } else {
                    true
                }
            })
            .filter(|m| {
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

        if let Some(text) = &query.text {
            let search_text = text.to_lowercase();
            results.retain(|r| {
                r.memory
                    .content
                    .as_text()
                    .map(|t| t.to_lowercase().contains(&search_text))
                    .unwrap_or(false)
            });
        }

        Ok(results)
    }

    async fn batch_add(&self, memories: Vec<MemoryEntry>) -> Result<BatchResult, MemoryError> {
        info!("Batch adding {} memories", memories.len());

        if !self.is_storage_enabled() {
            return Err(MemoryError::InvalidOperation(
                "Storage is disabled".to_string(),
            ));
        }

        let mut result = BatchResult::new();
        let mut storage = self.memories.write().await;

        for memory in memories {
            if storage.insert(memory.id, memory).is_none() {
                result.add_success();
            } else {
                result.add_failure("Memory ID already exists".to_string());
            }
        }

        Ok(result)
    }

    async fn batch_delete(&self, ids: Vec<MemoryId>) -> Result<BatchResult, MemoryError> {
        info!("Batch deleting {} memories", ids.len());

        if !self.is_storage_enabled() {
            return Err(MemoryError::InvalidOperation(
                "Storage is disabled".to_string(),
            ));
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
    async fn search(&self, query: SearchQuery) -> Result<Vec<SearchResult>, MemoryError> {
        debug!("Full-text search: {:?}", query.text);

        if !self.is_index_enabled() {
            return Err(MemoryError::IndexError("Index is disabled".to_string()));
        }

        let workspace_ids: Vec<WorkspaceId> = if let Some(ws_id) = query.workspace_id {
            vec![ws_id]
        } else {
            let workspaces = self.workspaces.read().await;
            workspaces.keys().copied().collect()
        };

        let mut all_results = Vec::new();
        for ws_id in workspace_ids {
            let results = MemoryApi::list(self, ws_id, query.clone()).await?;
            all_results.extend(results);
        }

        all_results.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        Ok(all_results)
    }

    async fn vector_search(
        &self,
        workspace_id: WorkspaceId,
        embedding: &[f32],
        limit: usize,
    ) -> Result<Vec<SearchResult>, MemoryError> {
        debug!(
            "Vector search for workspace: {}, limit: {}",
            workspace_id, limit
        );

        if !self.is_index_enabled() {
            return Err(MemoryError::IndexError("Index is disabled".to_string()));
        }

        let memories = self.memories.read().await;

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

        results.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        results.truncate(limit);

        Ok(results)
    }

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

        let text_query = SearchQuery {
            text: Some(text.to_string()),
            workspace_id: Some(workspace_id),
            limit,
            ..Default::default()
        };
        let text_results = self.search(text_query).await?;

        let vector_results = self.vector_search(workspace_id, embedding, limit).await?;

        let mut merged: std::collections::HashMap<MemoryId, (f32, SearchResult)> =
            std::collections::HashMap::new();

        for r in text_results {
            let entry = merged
                .entry(r.memory.id)
                .or_insert_with(|| (0.0, r.clone()));
            entry.0 += r.score;
        }

        for r in vector_results {
            let entry = merged
                .entry(r.memory.id)
                .or_insert_with(|| (0.0, r.clone()));
            entry.0 += r.score;
            if r.score > entry.1.score {
                entry.1 = r;
            }
        }

        let mut results: Vec<_> = merged
            .into_values()
            .map(|(sum, mut r)| {
                r.score = sum / 2.0;
                r
            })
            .collect();

        results.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        results.truncate(limit);

        Ok(results)
    }
}

#[async_trait]
impl WorkspaceApi for MemoryClient {
    async fn create(&self, workspace: Workspace) -> Result<WorkspaceId, MemoryError> {
        info!("Creating workspace: {}", workspace.name);

        let id = workspace.id;
        let mut workspaces = self.workspaces.write().await;
        workspaces.insert(id, workspace);

        Ok(id)
    }

    async fn get(&self, id: WorkspaceId) -> Result<Workspace, MemoryError> {
        debug!("Getting workspace: {}", id);

        let workspaces = self.workspaces.read().await;
        workspaces
            .get(&id)
            .cloned()
            .ok_or_else(|| MemoryError::WorkspaceError(format!("Workspace not found: {}", id)))
    }

    async fn update(&self, workspace: Workspace) -> Result<(), MemoryError> {
        debug!("Updating workspace: {}", workspace.id);

        let mut workspaces = self.workspaces.write().await;
        match workspaces.entry(workspace.id) {
            std::collections::hash_map::Entry::Occupied(mut e) => {
                e.insert(workspace);
                Ok(())
            }
            std::collections::hash_map::Entry::Vacant(_) => Err(MemoryError::WorkspaceError(
                format!("Workspace not found: {}", workspace.id),
            )),
        }
    }

    async fn delete(&self, id: WorkspaceId) -> Result<(), MemoryError> {
        info!("Deleting workspace: {}", id);

        let mut memories = self.memories.write().await;
        memories.retain(|_, m| m.workspace_id != id);

        let mut workspaces = self.workspaces.write().await;
        if workspaces.remove(&id).is_some() {
            Ok(())
        } else {
            Err(MemoryError::WorkspaceError(format!(
                "Workspace not found: {}",
                id
            )))
        }
    }

    async fn list(&self) -> Result<Vec<Workspace>, MemoryError> {
        debug!("Listing all workspaces");

        let workspaces = self.workspaces.read().await;
        Ok(workspaces.values().cloned().collect())
    }

    async fn stats(&self, id: WorkspaceId) -> Result<MemoryStats, MemoryError> {
        debug!("Getting stats for workspace: {}", id);

        let workspaces = self.workspaces.read().await;
        if !workspaces.contains_key(&id) {
            return Err(MemoryError::WorkspaceError(format!(
                "Workspace not found: {}",
                id
            )));
        }
        drop(workspaces);

        let memories = self.memories.read().await;
        let workspace_memories: Vec<_> =
            memories.values().filter(|m| m.workspace_id == id).collect();

        let mut stats = MemoryStats {
            total_memories: workspace_memories.len() as u64,
            active_count: 0,
            cooling_count: 0,
            cold_count: 0,
            zombie_count: 0,
            total_size_bytes: 0,
            average_importance: 0.0,
        };

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

#[cfg(test)]
mod tests {
    use super::*;
    use memory_core::{
        MemoryContent, MemoryMetadata, MemorySource, MemoryStatus, SearchQuery, Workspace,
    };
    use uuid::Uuid;

    // Helper function to create a test memory entry
    fn create_test_memory(workspace_id: WorkspaceId, importance: f32) -> MemoryEntry {
        let content = MemoryContent::Text("Test memory content".to_string());
        let metadata = MemoryMetadata::new(MemorySource::UserQuery {
            query: "test".to_string(),
        })
        .with_importance(importance);
        MemoryEntry::new(workspace_id, content, metadata)
    }

    // Helper function to create a test workspace
    fn create_test_workspace(name: &str) -> Workspace {
        Workspace::new(name.to_string())
    }

    // ==================== ClientConfig Tests ====================

    #[test]
    fn test_client_config_default() {
        let config = ClientConfig::default();
        assert!(config.storage_enabled);
        assert!(config.index_enabled);
    }

    #[test]
    fn test_client_config_custom() {
        let config = ClientConfig {
            storage_enabled: false,
            index_enabled: false,
        };
        assert!(!config.storage_enabled);
        assert!(!config.index_enabled);
    }

    // ==================== MemoryClient Tests ====================

    #[test]
    fn test_memory_client_new() {
        let client = MemoryClient::new();
        assert!(client.is_storage_enabled());
        assert!(client.is_index_enabled());
    }

    #[test]
    fn test_memory_client_with_config() {
        let config = ClientConfig {
            storage_enabled: false,
            index_enabled: true,
        };
        let client = MemoryClient::with_config(config);
        assert!(!client.is_storage_enabled());
        assert!(client.is_index_enabled());
    }

    #[tokio::test]
    async fn test_memory_client_default_trait() {
        let client = MemoryClient::default();
        assert!(client.is_storage_enabled());
    }

    // ==================== MemoryApi - add Tests ====================

    #[tokio::test]
    async fn test_add_memory_success() {
        let client = MemoryClient::new();
        let workspace_id = Uuid::new_v4();
        let memory = create_test_memory(workspace_id, 0.8);
        let id = memory.id;

        let result = MemoryApi::add(&client, memory).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), id);

        // Verify it was stored
        let retrieved = MemoryApi::get(&client, id).await;
        assert!(retrieved.is_ok());
    }

    #[tokio::test]
    async fn test_add_memory_storage_disabled() {
        let client = MemoryClient::with_config(ClientConfig {
            storage_enabled: false,
            index_enabled: true,
        });
        let workspace_id = Uuid::new_v4();
        let memory = create_test_memory(workspace_id, 0.5);

        let result = MemoryApi::add(&client, memory).await;
        assert!(result.is_err());
    }

    // ==================== MemoryApi - get Tests ====================

    #[tokio::test]
    async fn test_get_memory_success() {
        let client = MemoryClient::new();
        let workspace_id = Uuid::new_v4();
        let memory = create_test_memory(workspace_id, 0.8);

        let id = memory.id;
        MemoryApi::add(&client, memory).await.unwrap();

        let result = MemoryApi::get(&client, id).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().id, id);
    }

    #[tokio::test]
    async fn test_get_memory_not_found() {
        let client = MemoryClient::new();
        let id = Uuid::new_v4();

        let result = MemoryApi::get(&client, id).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_get_memory_storage_disabled() {
        let client = MemoryClient::with_config(ClientConfig {
            storage_enabled: false,
            index_enabled: true,
        });
        let id = Uuid::new_v4();

        let result = MemoryApi::get(&client, id).await;
        assert!(result.is_err());
    }

    // ==================== MemoryApi - update Tests ====================

    #[tokio::test]
    async fn test_update_memory_success() {
        let client = MemoryClient::new();
        let workspace_id = Uuid::new_v4();
        let memory = create_test_memory(workspace_id, 0.5);

        let id = memory.id;
        MemoryApi::add(&client, memory).await.unwrap();

        // Update the memory
        let mut updated_memory = create_test_memory(workspace_id, 0.9);
        updated_memory.id = id;

        let result = MemoryApi::update(&client, updated_memory).await;
        assert!(result.is_ok());

        // Verify the update
        let retrieved = MemoryApi::get(&client, id).await.unwrap();
        assert_eq!(retrieved.metadata.importance, 0.9);
    }

    #[tokio::test]
    async fn test_update_memory_not_found() {
        let client = MemoryClient::new();
        let workspace_id = Uuid::new_v4();
        let memory = create_test_memory(workspace_id, 0.5);

        let result = MemoryApi::update(&client, memory).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_update_memory_storage_disabled() {
        let client = MemoryClient::with_config(ClientConfig {
            storage_enabled: false,
            index_enabled: true,
        });
        let workspace_id = Uuid::new_v4();
        let memory = create_test_memory(workspace_id, 0.5);

        let result = MemoryApi::update(&client, memory).await;
        assert!(result.is_err());
    }

    // ==================== MemoryApi - delete Tests ====================

    #[tokio::test]
    async fn test_delete_memory_success() {
        let client = MemoryClient::new();
        let workspace_id = Uuid::new_v4();
        let memory = create_test_memory(workspace_id, 0.5);

        let id = memory.id;
        MemoryApi::add(&client, memory).await.unwrap();

        let result = MemoryApi::delete(&client, id).await;
        assert!(result.is_ok());

        // Verify it was deleted
        let retrieved = MemoryApi::get(&client, id).await;
        assert!(retrieved.is_err());
    }

    #[tokio::test]
    async fn test_delete_memory_not_found() {
        let client = MemoryClient::new();
        let id = Uuid::new_v4();

        let result = MemoryApi::delete(&client, id).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_delete_memory_storage_disabled() {
        let client = MemoryClient::with_config(ClientConfig {
            storage_enabled: false,
            index_enabled: true,
        });
        let id = Uuid::new_v4();

        let result = MemoryApi::delete(&client, id).await;
        assert!(result.is_err());
    }

    // ==================== MemoryApi - list Tests ====================

    #[tokio::test]
    async fn test_list_memories_empty() {
        let client = MemoryClient::new();
        let workspace_id = Uuid::new_v4();
        let query = SearchQuery::default();

        let result = MemoryApi::list(&client, workspace_id, query).await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_list_memories_with_results() {
        let client = MemoryClient::new();
        let workspace_id = Uuid::new_v4();

        // Add some memories
        for i in 0..3 {
            let content = MemoryContent::Text(format!("Memory {}", i));
            let metadata = MemoryMetadata::new(MemorySource::UserQuery {
                query: "test".to_string(),
            });
            let memory = MemoryEntry::new(workspace_id, content, metadata);
            MemoryApi::add(&client, memory).await.unwrap();
        }

        let query = SearchQuery::default();
        let result = MemoryApi::list(&client, workspace_id, query).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 3);
    }

    #[tokio::test]
    async fn test_list_memories_different_workspace() {
        let client = MemoryClient::new();
        let workspace_id1 = Uuid::new_v4();
        let workspace_id2 = Uuid::new_v4();

        // Add memories to workspace1
        let content = MemoryContent::Text("Test".to_string());
        let metadata = MemoryMetadata::new(MemorySource::UserQuery {
            query: "test".to_string(),
        });
        let memory1 = MemoryEntry::new(workspace_id1, content.clone(), metadata.clone());
        MemoryApi::add(&client, memory1).await.unwrap();

        // Add memories to workspace2
        let memory2 = MemoryEntry::new(workspace_id2, content, metadata);
        MemoryApi::add(&client, memory2).await.unwrap();

        // List workspace1
        let query = SearchQuery::default();
        let result = MemoryApi::list(&client, workspace_id1, query).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn test_list_memories_storage_disabled() {
        let client = MemoryClient::with_config(ClientConfig {
            storage_enabled: false,
            index_enabled: true,
        });
        let workspace_id = Uuid::new_v4();
        let query = SearchQuery::default();

        let result = MemoryApi::list(&client, workspace_id, query).await;
        assert!(result.is_err());
    }

    // ==================== MemoryApi - batch_add Tests ====================

    #[tokio::test]
    async fn test_batch_add_success() {
        let client = MemoryClient::new();
        let workspace_id = Uuid::new_v4();

        let mut memories: Vec<MemoryEntry> = Vec::new();
        for i in 0..5 {
            let content = MemoryContent::Text(format!("Memory {}", i));
            let metadata = MemoryMetadata::new(MemorySource::UserQuery {
                query: "test".to_string(),
            });
            let memory = MemoryEntry::new(workspace_id, content, metadata);
            memories.push(memory);
        }

        let result = MemoryApi::batch_add(&client, memories).await;
        assert!(result.is_ok());
        let batch_result = result.unwrap();
        assert_eq!(batch_result.success_count, 5);
        assert_eq!(batch_result.failure_count, 0);
    }

    #[tokio::test]
    async fn test_batch_add_storage_disabled() {
        let client = MemoryClient::with_config(ClientConfig {
            storage_enabled: false,
            index_enabled: true,
        });
        let workspace_id = Uuid::new_v4();

        let memories = vec![create_test_memory(workspace_id, 0.5)];

        let result = MemoryApi::batch_add(&client, memories).await;
        assert!(result.is_err());
    }

    // ==================== MemoryApi - batch_delete Tests ====================

    #[tokio::test]
    async fn test_batch_delete_success() {
        let client = MemoryClient::new();
        let workspace_id = Uuid::new_v4();

        // Add memories first
        let mut ids: Vec<MemoryId> = Vec::new();
        for _ in 0..5 {
            let content = MemoryContent::Text("Test".to_string());
            let metadata = MemoryMetadata::new(MemorySource::UserQuery {
                query: "test".to_string(),
            });
            let memory = MemoryEntry::new(workspace_id, content, metadata);
            let id = memory.id;
            MemoryApi::add(&client, memory).await.unwrap();
            ids.push(id);
        }

        let result = MemoryApi::batch_delete(&client, ids).await;
        assert!(result.is_ok());
        let batch_result = result.unwrap();
        assert_eq!(batch_result.success_count, 5);
        assert_eq!(batch_result.failure_count, 0);
    }

    #[tokio::test]
    async fn test_batch_delete_partial_failure() {
        let client = MemoryClient::new();
        let workspace_id = Uuid::new_v4();

        // Add one memory
        let content = MemoryContent::Text("Test".to_string());
        let metadata = MemoryMetadata::new(MemorySource::UserQuery {
            query: "test".to_string(),
        });
        let memory = MemoryEntry::new(workspace_id, content, metadata);
        let existing_id = memory.id;
        MemoryApi::add(&client, memory).await.unwrap();

        // Try to delete existing and non-existing
        let ids = vec![existing_id, Uuid::new_v4()];

        let result = MemoryApi::batch_delete(&client, ids).await;
        assert!(result.is_ok());
        let batch_result = result.unwrap();
        assert_eq!(batch_result.success_count, 1);
        assert_eq!(batch_result.failure_count, 1);
    }

    #[tokio::test]
    async fn test_batch_delete_storage_disabled() {
        let client = MemoryClient::with_config(ClientConfig {
            storage_enabled: false,
            index_enabled: true,
        });
        let ids = vec![Uuid::new_v4()];

        let result = MemoryApi::batch_delete(&client, ids).await;
        assert!(result.is_err());
    }

    // ==================== SearchApi - search Tests ====================

    #[tokio::test]
    async fn test_search_empty_results() {
        let client = MemoryClient::new();
        let query = SearchQuery::default();

        let result = SearchApi::search(&client, query).await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_search_with_memories() {
        let client = MemoryClient::new();

        // Create a workspace first (search needs workspaces to exist)
        let workspace = Workspace::new("test".to_string());
        let workspace_id = workspace.id;
        WorkspaceApi::create(&client, workspace).await.unwrap();

        // Add memories with different importance
        for i in 0..3 {
            let content = MemoryContent::Text(format!("Searchable memory {}", i));
            let metadata = MemoryMetadata::new(MemorySource::UserQuery {
                query: "test".to_string(),
            })
            .with_importance(0.5 + i as f32 * 0.2);
            let memory = MemoryEntry::new(workspace_id, content, metadata);
            MemoryApi::add(&client, memory).await.unwrap();
        }

        let query = SearchQuery::default();
        let result = SearchApi::search(&client, query).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 3);
    }

    // ==================== SearchApi - vector_search Tests ====================

    #[tokio::test]
    async fn test_vector_search_empty_results() {
        let client = MemoryClient::new();
        let workspace_id = Uuid::new_v4();
        let embedding = vec![0.1; 128];

        let result = SearchApi::vector_search(&client, workspace_id, &embedding, 10).await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_vector_search_with_embeddings() {
        let client = MemoryClient::new();
        let workspace_id = Uuid::new_v4();

        // Add memory with embedding
        let content = MemoryContent::Text("Test content".to_string());
        let metadata = MemoryMetadata::new(MemorySource::UserQuery {
            query: "test".to_string(),
        });
        let mut memory = MemoryEntry::new(workspace_id, content, metadata);
        memory.embedding = Some(vec![1.0; 128]);
        MemoryApi::add(&client, memory).await.unwrap();

        let embedding = vec![1.0; 128];
        let result = SearchApi::vector_search(&client, workspace_id, &embedding, 10).await;
        assert!(result.is_ok());
        assert!(!result.unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_vector_search_no_matching_embedding() {
        let client = MemoryClient::new();
        let workspace_id = Uuid::new_v4();

        // Add memory without embedding
        let content = MemoryContent::Text("Test content".to_string());
        let metadata = MemoryMetadata::new(MemorySource::UserQuery {
            query: "test".to_string(),
        });
        let memory = MemoryEntry::new(workspace_id, content, metadata);
        MemoryApi::add(&client, memory).await.unwrap();

        let embedding = vec![1.0; 128];
        let result = SearchApi::vector_search(&client, workspace_id, &embedding, 10).await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    // ==================== SearchApi - hybrid_search Tests ====================

    #[tokio::test]
    async fn test_hybrid_search_empty_results() {
        let client = MemoryClient::new();
        let workspace_id = Uuid::new_v4();
        let embedding = vec![0.1; 128];

        let result =
            SearchApi::hybrid_search(&client, workspace_id, "nonexistent", &embedding, 10).await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_hybrid_search_with_results() {
        let client = MemoryClient::new();
        let workspace_id = Uuid::new_v4();

        // Add memory with embedding and text content
        let content = MemoryContent::Text("hybrid search test".to_string());
        let metadata = MemoryMetadata::new(MemorySource::UserQuery {
            query: "test".to_string(),
        });
        let mut memory = MemoryEntry::new(workspace_id, content, metadata);
        memory.embedding = Some(vec![1.0; 128]);
        MemoryApi::add(&client, memory).await.unwrap();

        let embedding = vec![1.0; 128];
        let result =
            SearchApi::hybrid_search(&client, workspace_id, "hybrid", &embedding, 10).await;
        assert!(result.is_ok());
        assert!(!result.unwrap().is_empty());
    }

    // ==================== WorkspaceApi - create Tests ====================

    #[tokio::test]
    async fn test_create_workspace_success() {
        let client = MemoryClient::new();
        let workspace = create_test_workspace("Test Workspace");

        let id = workspace.id;
        let result = WorkspaceApi::create(&client, workspace).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), id);

        // Verify it was stored
        let retrieved = WorkspaceApi::get(&client, id).await;
        assert!(retrieved.is_ok());
    }

    // ==================== WorkspaceApi - get Tests ====================

    #[tokio::test]
    async fn test_get_workspace_success() {
        let client = MemoryClient::new();
        let workspace = create_test_workspace("Test Workspace");

        let id = workspace.id;
        WorkspaceApi::create(&client, workspace).await.unwrap();

        let result = WorkspaceApi::get(&client, id).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().id, id);
    }

    #[tokio::test]
    async fn test_get_workspace_not_found() {
        let client = MemoryClient::new();
        let id = Uuid::new_v4();

        let result = WorkspaceApi::get(&client, id).await;
        assert!(result.is_err());
    }

    // ==================== WorkspaceApi - update Tests ====================

    #[tokio::test]
    async fn test_update_workspace_success() {
        let client = MemoryClient::new();
        let workspace = create_test_workspace("Original Name");

        let id = workspace.id;
        WorkspaceApi::create(&client, workspace).await.unwrap();

        // Update workspace
        let mut updated_workspace = create_test_workspace("Updated Name");
        updated_workspace.id = id;

        let result = WorkspaceApi::update(&client, updated_workspace).await;
        assert!(result.is_ok());

        // Verify the update
        let retrieved = WorkspaceApi::get(&client, id).await.unwrap();
        assert_eq!(retrieved.name, "Updated Name");
    }

    #[tokio::test]
    async fn test_update_workspace_not_found() {
        let client = MemoryClient::new();
        let workspace = create_test_workspace("Test");

        let result = WorkspaceApi::update(&client, workspace).await;
        assert!(result.is_err());
    }

    // ==================== WorkspaceApi - delete Tests ====================

    #[tokio::test]
    async fn test_delete_workspace_success() {
        let client = MemoryClient::new();
        let workspace = create_test_workspace("Test Workspace");

        let id = workspace.id;
        WorkspaceApi::create(&client, workspace).await.unwrap();

        let result = WorkspaceApi::delete(&client, id).await;
        assert!(result.is_ok());

        // Verify it was deleted
        let retrieved = WorkspaceApi::get(&client, id).await;
        assert!(retrieved.is_err());
    }

    #[tokio::test]
    async fn test_delete_workspace_not_found() {
        let client = MemoryClient::new();
        let id = Uuid::new_v4();

        let result = WorkspaceApi::delete(&client, id).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_delete_workspace_cascades_memories() {
        let client = MemoryClient::new();
        let workspace = create_test_workspace("Test Workspace");

        let workspace_id = workspace.id;
        WorkspaceApi::create(&client, workspace).await.unwrap();

        // Add memories to workspace
        let content = MemoryContent::Text("Test".to_string());
        let metadata = MemoryMetadata::new(MemorySource::UserQuery {
            query: "test".to_string(),
        });
        let memory = MemoryEntry::new(workspace_id, content, metadata);
        MemoryApi::add(&client, memory).await.unwrap();

        // Delete workspace
        WorkspaceApi::delete(&client, workspace_id).await.unwrap();

        // Verify memories were deleted
        let query = SearchQuery::default();
        let result = MemoryApi::list(&client, workspace_id, query).await.unwrap();
        assert!(result.is_empty());
    }

    // ==================== WorkspaceApi - list Tests ====================

    #[tokio::test]
    async fn test_list_workspaces_empty() {
        let client = MemoryClient::new();

        let result = WorkspaceApi::list(&client).await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_list_workspaces_with_results() {
        let client = MemoryClient::new();

        // Create workspaces
        for i in 0..3 {
            let workspace = create_test_workspace(&format!("Workspace {}", i));
            WorkspaceApi::create(&client, workspace).await.unwrap();
        }

        let result = WorkspaceApi::list(&client).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 3);
    }

    // ==================== WorkspaceApi - stats Tests ====================

    #[tokio::test]
    async fn test_workspace_stats_empty() {
        let client = MemoryClient::new();
        let workspace = create_test_workspace("Test Workspace");

        let workspace_id = workspace.id;
        WorkspaceApi::create(&client, workspace).await.unwrap();

        let result = WorkspaceApi::stats(&client, workspace_id).await;
        assert!(result.is_ok());
        let stats = result.unwrap();
        assert_eq!(stats.total_memories, 0);
    }

    #[tokio::test]
    async fn test_workspace_stats_with_memories() {
        let client = MemoryClient::new();
        let workspace = create_test_workspace("Test Workspace");

        let workspace_id = workspace.id;
        WorkspaceApi::create(&client, workspace).await.unwrap();

        // Add memories with different statuses
        let statuses = vec![
            MemoryStatus::Active,
            MemoryStatus::Active,
            MemoryStatus::Cooling,
            MemoryStatus::Cold,
        ];

        for (i, status) in statuses.iter().enumerate() {
            let content = MemoryContent::Text(format!("Memory {}", i));
            let metadata = MemoryMetadata::new(MemorySource::UserQuery {
                query: "test".to_string(),
            })
            .with_importance(0.5);
            let mut memory = MemoryEntry::new(workspace_id, content, metadata);
            memory.status = *status;
            MemoryApi::add(&client, memory).await.unwrap();
        }

        let result = WorkspaceApi::stats(&client, workspace_id).await;
        assert!(result.is_ok());
        let stats = result.unwrap();
        assert_eq!(stats.total_memories, 4);
        assert_eq!(stats.active_count, 2);
        assert_eq!(stats.cooling_count, 1);
        assert_eq!(stats.cold_count, 1);
    }

    #[tokio::test]
    async fn test_workspace_stats_not_found() {
        let client = MemoryClient::new();
        let id = Uuid::new_v4();

        let result = WorkspaceApi::stats(&client, id).await;
        assert!(result.is_err());
    }

    // ==================== Cosine Similarity Tests ====================

    #[test]
    fn test_cosine_similarity_identical_vectors() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];

        let result = cosine_similarity(&a, &b);
        assert_eq!(result, 1.0);
    }

    #[test]
    fn test_cosine_similarity_orthogonal_vectors() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![0.0, 1.0, 0.0];

        let result = cosine_similarity(&a, &b);
        assert_eq!(result, 0.0);
    }

    #[test]
    fn test_cosine_similarity_opposite_vectors() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![-1.0, 0.0, 0.0];

        let result = cosine_similarity(&a, &b);
        assert_eq!(result, -1.0);
    }

    #[test]
    fn test_cosine_similarity_different_lengths() {
        let a = vec![1.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];

        let result = cosine_similarity(&a, &b);
        assert_eq!(result, 0.0);
    }

    #[test]
    fn test_cosine_similarity_empty_vectors() {
        let a: Vec<f32> = vec![];
        let b: Vec<f32> = vec![];

        let result = cosine_similarity(&a, &b);
        assert_eq!(result, 0.0);
    }

    #[test]
    fn test_cosine_similarity_zero_vector() {
        let a = vec![0.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];

        let result = cosine_similarity(&a, &b);
        assert_eq!(result, 0.0);
    }
}
