//! Metadata Index for tags and filters
//!
//! This module provides efficient filtering and querying by metadata fields
//! such as tags, status, importance, and date ranges.

use crate::error::IndexError;
use memory_core::{DateRange, MemoryEntry, MemoryId, WorkspaceId};
use std::collections::{HashMap, HashSet};
use tracing::{debug, info};

/// Configuration for the metadata index
#[derive(Debug, Clone)]
pub struct MetadataConfig {
    /// Maximum number of tags to track per workspace
    pub max_tags: usize,
    /// Enable importance-based indexing
    pub index_importance: bool,
    /// Enable status-based indexing
    pub index_status: bool,
}

impl Default for MetadataConfig {
    fn default() -> Self {
        Self {
            max_tags: 10_000,
            index_importance: true,
            index_status: true,
        }
    }
}

/// Metadata Index for efficient filtering
pub struct MetadataIndex {
    config: MetadataConfig,
    /// Workspace -> memory IDs
    workspace_memories: HashMap<WorkspaceId, HashSet<MemoryId>>,
    /// Memory ID -> metadata mapping
    memory_metadata: HashMap<MemoryId, MemoryMetadataEntry>,
    /// Tag -> memory IDs (inverted index)
    tag_index: HashMap<String, HashSet<MemoryId>>,
    /// Status -> memory IDs (using String to avoid Hash requirement)
    status_index: HashMap<String, HashSet<MemoryId>>,
    /// Workspace -> tags
    workspace_tags: HashMap<WorkspaceId, HashSet<String>>,
    count: usize,
}

/// In-memory metadata entry for quick filtering
#[derive(Debug, Clone)]
struct MemoryMetadataEntry {
    memory_id: MemoryId,
    workspace_id: WorkspaceId,
    tags: Vec<String>,
    importance: f32,
    status: String,
    created_at: chrono::DateTime<chrono::Utc>,
    updated_at: chrono::DateTime<chrono::Utc>,
}

impl MetadataIndex {
    /// Create a new MetadataIndex
    pub fn new(config: MetadataConfig) -> Self {
        info!("Creating new MetadataIndex");
        Self {
            config,
            workspace_memories: HashMap::new(),
            memory_metadata: HashMap::new(),
            tag_index: HashMap::new(),
            status_index: HashMap::new(),
            workspace_tags: HashMap::new(),
            count: 0,
        }
    }

    /// Add a memory entry to the index
    pub async fn add(&mut self, memory: &MemoryEntry) -> Result<(), IndexError> {
        let memory_id = memory.id;
        let workspace_id = memory.workspace_id;

        // Add to workspace index
        self.workspace_memories
            .entry(workspace_id)
            .or_default()
            .insert(memory_id);

        // Add to workspace tags
        for tag in &memory.metadata.tags {
            self.workspace_tags
                .entry(workspace_id)
                .or_default()
                .insert(tag.clone());
            
            // Add to tag inverted index
            self.tag_index
                .entry(tag.clone())
                .or_default()
                .insert(memory_id);
        }

        // Add to status index (use String key)
        let status_str = memory.status.to_string();
        self.status_index
            .entry(status_str.clone())
            .or_default()
            .insert(memory_id);

        // Store metadata
        let entry = MemoryMetadataEntry {
            memory_id,
            workspace_id,
            tags: memory.metadata.tags.clone(),
            importance: memory.metadata.importance,
            status: status_str,
            created_at: memory.created_at,
            updated_at: memory.updated_at,
        };
        
        self.memory_metadata.insert(memory_id, entry);
        self.count += 1;

        debug!("Added memory {} to metadata index", memory_id);
        Ok(())
    }

    /// Add multiple memories in batch
    pub async fn batch_add(&mut self, memories: &[MemoryEntry]) -> Result<(), IndexError> {
        for memory in memories {
            self.add(memory).await?;
        }
        Ok(())
    }

    /// Remove a memory from the index
    pub async fn remove(&mut self, memory_id: MemoryId) -> Result<(), IndexError> {
        let entry = self
            .memory_metadata
            .remove(&memory_id)
            .ok_or_else(|| IndexError::NotFound(format!("Memory {} not in index", memory_id)))?;

        // Remove from workspace index
        if let Some(set) = self.workspace_memories.get_mut(&entry.workspace_id) {
            set.remove(&memory_id);
        }

        // Remove from tag index
        for tag in &entry.tags {
            if let Some(set) = self.tag_index.get_mut(tag) {
                set.remove(&memory_id);
            }
        }

        // Remove from status index
        if let Some(set) = self.status_index.get_mut(&entry.status) {
            set.remove(&memory_id);
        }

        self.count = self.count.saturating_sub(1);
        debug!("Removed memory {} from metadata index", memory_id);
        Ok(())
    }

    /// Get all memory IDs for a workspace
    pub async fn get_by_workspace(&self, workspace_id: WorkspaceId) -> Vec<MemoryId> {
        self.workspace_memories
            .get(&workspace_id)
            .map(|s| s.iter().copied().collect())
            .unwrap_or_default()
    }

    /// Get memories by tags (OR logic - any matching tag)
    pub async fn get_by_tags(&self, tags: &[String], workspace_id: Option<WorkspaceId>) -> Vec<MemoryId> {
        let mut result: HashSet<MemoryId> = HashSet::new();

        for tag in tags {
            if let Some(memory_ids) = self.tag_index.get(tag) {
                result.extend(memory_ids.iter().copied());
            }
        }

        // Filter by workspace if specified
        if let Some(ws_id) = workspace_id {
            result.retain(|id| {
                self.memory_metadata
                    .get(id)
                    .map(|m| m.workspace_id == ws_id)
                    .unwrap_or(false)
            });
        }

        result.into_iter().collect()
    }

    /// Get memories by status
    pub async fn get_by_status(&self, status: memory_core::MemoryStatus, workspace_id: Option<WorkspaceId>) -> Vec<MemoryId> {
        let status_str = status.to_string();
        
        let mut result = self
            .status_index
            .get(&status_str)
            .map(|s| s.iter().copied().collect::<Vec<_>>())
            .unwrap_or_default();

        // Filter by workspace if specified
        if let Some(ws_id) = workspace_id {
            result.retain(|id| {
                self.memory_metadata
                    .get(id)
                    .map(|m| m.workspace_id == ws_id)
                    .unwrap_or(false)
            });
        }

        result
    }

    /// Get memories by date range
    pub async fn get_by_date_range(
        &self,
        range: &DateRange,
        workspace_id: Option<WorkspaceId>,
    ) -> Vec<MemoryId> {
        let mut result = Vec::new();

        for (_, entry) in &self.memory_metadata {
            // Check workspace filter
            if let Some(ws_id) = workspace_id {
                if entry.workspace_id != ws_id {
                    continue;
                }
            }

            if entry.created_at >= range.start && entry.created_at <= range.end {
                result.push(entry.memory_id);
            }
        }

        result
    }

    /// Get memories by importance range
    pub async fn get_by_importance(
        &self,
        min: f32,
        max: f32,
        workspace_id: Option<WorkspaceId>,
    ) -> Vec<MemoryId> {
        let mut result = Vec::new();

        for (_, entry) in &self.memory_metadata {
            // Check workspace filter
            if let Some(ws_id) = workspace_id {
                if entry.workspace_id != ws_id {
                    continue;
                }
            }

            if entry.importance >= min && entry.importance <= max {
                result.push(entry.memory_id);
            }
        }

        result
    }

    /// Get all tags for a workspace
    pub async fn get_tags(&self, workspace_id: WorkspaceId) -> Vec<String> {
        self.workspace_tags
            .get(&workspace_id)
            .map(|s| s.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// Get count by status for a workspace
    pub async fn get_status_counts(&self, workspace_id: WorkspaceId) -> HashMap<String, usize> {
        let mut counts = HashMap::new();

        for (status, memory_ids) in &self.status_index {
            let count = memory_ids
                .iter()
                .filter(|id| {
                    self.memory_metadata
                        .get(id)
                        .map(|m| m.workspace_id == workspace_id)
                        .unwrap_or(false)
                })
                .count();
            
            if count > 0 {
                counts.insert(status.clone(), count);
            }
        }

        counts
    }

    /// Get the number of indexed memories
    pub fn len(&self) -> usize {
        self.count
    }

    /// Check if the index is empty
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// Get the configuration
    pub fn config(&self) -> &MetadataConfig {
        &self.config
    }
}

/// Builder for MetadataIndex
pub struct MetadataIndexBuilder {
    config: MetadataConfig,
}

impl MetadataIndexBuilder {
    pub fn new() -> Self {
        Self {
            config: MetadataConfig::default(),
        }
    }

    pub fn with_max_tags(mut self, max: usize) -> Self {
        self.config.max_tags = max;
        self
    }

    pub fn with_index_importance(mut self, enabled: bool) -> Self {
        self.config.index_importance = enabled;
        self
    }

    pub fn with_index_status(mut self, enabled: bool) -> Self {
        self.config.index_status = enabled;
        self
    }

    pub fn build(self) -> MetadataIndex {
        MetadataIndex::new(self.config)
    }
}

impl Default for MetadataIndexBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use memory_core::{MemoryContent, MemoryMetadata, MemoryStatus};
    use uuid::Uuid;

    fn create_test_memory(
        workspace_id: WorkspaceId,
        tags: Vec<String>,
        status: MemoryStatus,
    ) -> MemoryEntry {
        MemoryEntry {
            id: Uuid::new_v4(),
            workspace_id,
            content: MemoryContent::Text("Test content".to_string()),
            embedding: None,
            metadata: MemoryMetadata {
                source: memory_core::MemorySource::System { source_type: "test".to_string() },
                custom_fields: std::collections::HashMap::new(),
                tags,
                importance: 0.5,
            },
            status,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            access_count: 0,
            last_accessed: None,
        }
    }

    #[tokio::test]
    async fn test_metadata_index_add() {
        let mut index = MetadataIndex::new(MetadataConfig::default());
        
        let workspace_id = Uuid::new_v4();
        let memory = create_test_memory(
            workspace_id,
            vec!["tag1".to_string(), "tag2".to_string()],
            MemoryStatus::Active,
        );

        index.add(&memory).await.unwrap();
        assert_eq!(index.len(), 1);
    }

    #[tokio::test]
    async fn test_get_by_workspace() {
        let mut index = MetadataIndex::new(MetadataConfig::default());
        
        let ws1 = Uuid::new_v4();
        let ws2 = Uuid::new_v4();

        let mem1 = create_test_memory(ws1, vec![], MemoryStatus::Active);
        let mem2 = create_test_memory(ws2, vec![], MemoryStatus::Active);

        index.add(&mem1).await.unwrap();
        index.add(&mem2).await.unwrap();

        let results = index.get_by_workspace(ws1).await;
        assert_eq!(results.len(), 1);
    }

    #[tokio::test]
    async fn test_get_by_tags() {
        let mut index = MetadataIndex::new(MetadataConfig::default());
        
        let workspace_id = Uuid::new_v4();

        let mem1 = create_test_memory(
            workspace_id,
            vec!["rust".to_string(), "programming".to_string()],
            MemoryStatus::Active,
        );
        let mem2 = create_test_memory(
            workspace_id,
            vec!["python".to_string(), "programming".to_string()],
            MemoryStatus::Active,
        );
        let mem3 = create_test_memory(
            workspace_id,
            vec!["cooking".to_string()],
            MemoryStatus::Active,
        );

        index.add(&mem1).await.unwrap();
        index.add(&mem2).await.unwrap();
        index.add(&mem3).await.unwrap();

        // Search for "programming" tag
        let results = index.get_by_tags(&["programming".to_string()], Some(workspace_id)).await;
        assert_eq!(results.len(), 2);
    }

    #[tokio::test]
    async fn test_get_by_status() {
        let mut index = MetadataIndex::new(MetadataConfig::default());
        
        let workspace_id = Uuid::new_v4();

        let mem1 = create_test_memory(workspace_id, vec![], MemoryStatus::Active);
        let mem2 = create_test_memory(workspace_id, vec![], MemoryStatus::Cooling);
        let mem3 = create_test_memory(workspace_id, vec![], MemoryStatus::Cold);

        index.add(&mem1).await.unwrap();
        index.add(&mem2).await.unwrap();
        index.add(&mem3).await.unwrap();

        let active_results = index.get_by_status(MemoryStatus::Active, Some(workspace_id)).await;
        assert_eq!(active_results.len(), 1);

        let cooling_results = index.get_by_status(MemoryStatus::Cooling, Some(workspace_id)).await;
        assert_eq!(cooling_results.len(), 1);
    }

    #[tokio::test]
    async fn test_get_tags() {
        let mut index = MetadataIndex::new(MetadataConfig::default());
        
        let workspace_id = Uuid::new_v4();

        let mem1 = create_test_memory(
            workspace_id,
            vec!["rust".to_string(), "programming".to_string()],
            MemoryStatus::Active,
        );
        let mem2 = create_test_memory(
            workspace_id,
            vec!["python".to_string(), "programming".to_string()],
            MemoryStatus::Active,
        );

        index.add(&mem1).await.unwrap();
        index.add(&mem2).await.unwrap();

        let tags = index.get_tags(workspace_id).await;
        assert_eq!(tags.len(), 3); // rust, python, programming
    }

    #[tokio::test]
    async fn test_remove_memory() {
        let mut index = MetadataIndex::new(MetadataConfig::default());
        
        let workspace_id = Uuid::new_v4();
        let memory = create_test_memory(workspace_id, vec!["tag1".to_string()], MemoryStatus::Active);
        let memory_id = memory.id;

        index.add(&memory).await.unwrap();
        assert_eq!(index.len(), 1);

        index.remove(memory_id).await.unwrap();
        assert_eq!(index.len(), 0);
    }

    #[tokio::test]
    async fn test_batch_add() {
        let mut index = MetadataIndex::new(MetadataConfig::default());
        
        let workspace_id = Uuid::new_v4();
        let memories: Vec<_> = (0..5)
            .map(|_| create_test_memory(workspace_id, vec![], MemoryStatus::Active))
            .collect();

        index.batch_add(&memories).await.unwrap();
        assert_eq!(index.len(), 5);
    }

    #[test]
    fn test_metadata_index_builder() {
        let index = MetadataIndexBuilder::new()
            .with_max_tags(5000)
            .with_index_importance(false)
            .with_index_status(true)
            .build();

        assert_eq!(index.config().max_tags, 5000);
    }
}
