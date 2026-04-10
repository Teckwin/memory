//! Hot storage - in-memory cache for Active memories

use std::collections::HashMap;
use tokio::sync::RwLock;
use async_trait::async_trait;

use memory_core::{MemoryId, MemoryEntry, MemoryStatus, WorkspaceId,
    SearchQuery, SearchResult, BatchResult,
    MemoryApi, MemoryError,
};

/// Hot storage implementation using HashMap + RwLock
pub struct HotStorage {
    /// In-memory cache of active memories
    cache: RwLock<HashMap<MemoryId, MemoryEntry>>,
    /// Maximum number of entries to keep in hot storage
    max_entries: usize,
}

impl HotStorage {
    pub fn new(max_entries: usize) -> Self {
        Self {
            cache: RwLock::new(HashMap::new()),
            max_entries,
        }
    }

    pub fn with_capacity(capacity: usize, max_entries: usize) -> Self {
        Self {
            cache: RwLock::new(HashMap::with_capacity(capacity)),
            max_entries,
        }
    }

    /// Check if memory is in hot storage
    pub async fn contains(&self, id: MemoryId) -> bool {
        self.cache.read().await.contains_key(&id)
    }

    /// Get memory count in hot storage
    pub async fn len(&self) -> usize {
        self.cache.read().await.len()
    }

    /// Check if hot storage is empty
    pub async fn is_empty(&self) -> bool {
        self.cache.read().await.is_empty()
    }

    /// Get all memory IDs in hot storage
    pub async fn ids(&self) -> Vec<MemoryId> {
        self.cache.read().await.keys().copied().collect()
    }

    /// Evict oldest entries if over capacity
    async fn evict_if_needed(&self) {
        let mut cache = self.cache.write().await;
        while cache.len() >= self.max_entries {
            // Find and remove the oldest entry by access time
            if let Some((oldest_id, _)) = cache
                .iter()
                .min_by_key(|(_, entry)| entry.last_accessed.unwrap_or(entry.created_at))
            {
                let id = *oldest_id;
                cache.remove(&id);
            } else {
                break;
            }
        }
    }

    /// Load multiple entries into hot storage
    pub async fn load_batch(&self, entries: Vec<MemoryEntry>) {
        let mut cache = self.cache.write().await;
        for entry in entries {
            if entry.status == MemoryStatus::Active {
                cache.insert(entry.id, entry);
            }
        }
    }

    /// Remove entry from hot storage (when transitioning to Cold/Zombie)
    pub async fn remove(&self, id: MemoryId) -> Option<MemoryEntry> {
        self.cache.write().await.remove(&id)
    }
}

impl Default for HotStorage {
    fn default() -> Self {
        Self::new(10_000)
    }
}

#[async_trait]
impl MemoryApi for HotStorage {
    async fn add(&self, memory: MemoryEntry) -> Result<MemoryId, MemoryError> {
        if memory.status != MemoryStatus::Active {
            return Err(MemoryError::InvalidOperation(
                "Hot storage only accepts Active memories".to_string(),
            ));
        }

        self.evict_if_needed().await;

        let id = memory.id;
        self.cache.write().await.insert(id, memory);
        Ok(id)
    }

    async fn get(&self, id: MemoryId) -> Result<MemoryEntry, MemoryError> {
        let mut cache = self.cache.write().await;
        if let Some(mut entry) = cache.get(&id).cloned() {
            entry.increment_access();
            cache.insert(id, entry.clone());
            Ok(entry)
        } else {
            Err(MemoryError::NotFound(id.to_string()))
        }
    }

    async fn update(&self, memory: MemoryEntry) -> Result<(), MemoryError> {
        if memory.status != MemoryStatus::Active {
            return Err(MemoryError::InvalidOperation(
                "Hot storage only holds Active memories".to_string(),
            ));
        }

        let mut cache = self.cache.write().await;
        if cache.contains_key(&memory.id) {
            cache.insert(memory.id, memory);
            Ok(())
        } else {
            Err(MemoryError::NotFound(memory.id.to_string()))
        }
    }

    async fn delete(&self, id: MemoryId) -> Result<(), MemoryError> {
        if self.cache.write().await.remove(&id).is_some() {
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
        let cache = self.cache.read().await;
        
        let mut results: Vec<SearchResult> = cache
            .values()
            .filter(|m| m.workspace_id == workspace_id)
            .filter(|m| {
                // Apply status filter if specified
                if let Some(status) = query.status {
                    m.status == status
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
            .filter(|m| {
                // Apply text search if specified
                if let Some(text) = &query.text {
                    let content_text = m.content.as_text().unwrap_or("");
                    content_text.to_lowercase().contains(&text.to_lowercase())
                } else {
                    true
                }
            })
            .map(|m| SearchResult {
                memory: m.clone(),
                score: 1.0,
                highlights: vec![],
            })
            .collect();

        // Sort by created_at descending
        results.sort_by(|a, b| b.memory.created_at.cmp(&a.memory.created_at));

        // Apply pagination
        let start = query.offset;
        let end = (query.offset + query.limit).min(results.len());
        if start < results.len() {
            results[start..end].to_vec()
        } else {
            vec![]
        };
        
        Ok(results)
    }

    async fn batch_add(&self, memories: Vec<MemoryEntry>) -> Result<BatchResult, MemoryError> {
        let mut result = BatchResult::new();
        
        
        for memory in memories {
            if memory.status != MemoryStatus::Active {
                result.add_failure(format!(
                    "Memory {} is not Active",
                    memory.id
                ));
                continue;
            }
            
            self.evict_if_needed().await;
            
            let id = memory.id;
            let mut cache = self.cache.write().await;
            let is_new = cache.insert(id, memory).is_none();
            drop(cache);
            
            if is_new {
                result.add_success();
            } else {
                result.add_failure(format!("Failed to add memory {}", id));
            }
        }
        
        Ok(result)
    }

    async fn batch_delete(&self, ids: Vec<MemoryId>) -> Result<BatchResult, MemoryError> {
        let mut result = BatchResult::new();
        
        for id in ids {
            if self.cache.write().await.remove(&id).is_some() {
                result.add_success();
            } else {
                result.add_failure(format!("Memory {} not found", id));
            }
        }
        
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use memory_core::{MemoryContent, MemoryMetadata};

    fn create_test_memory(status: MemoryStatus) -> MemoryEntry {
        let mut entry = MemoryEntry::new(
            uuid::Uuid::new_v4(),
            MemoryContent::Text("test content".to_string()),
            MemoryMetadata::new(memory_core::MemorySource::UserQuery { 
                query: "test".to_string() 
            }).with_tags(vec!["test".to_string()]),
        );
        entry.status = status;
        entry
    }

    #[tokio::test]
    async fn test_add_active_memory() {
        let storage = HotStorage::new(100);
        let memory = create_test_memory(MemoryStatus::Active);
        let id = memory.id;
        
        let result = storage.add(memory).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), id);
    }

    #[tokio::test]
    async fn test_add_non_active_memory_fails() {
        let storage = HotStorage::new(100);
        let memory = create_test_memory(MemoryStatus::Cold);
        
        let result = storage.add(memory).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_get_memory() {
        let storage = HotStorage::new(100);
        let memory = create_test_memory(MemoryStatus::Active);
        let id = memory.id;
        
        storage.add(memory).await.unwrap();
        let retrieved = storage.get(id).await;
        
        assert!(retrieved.is_ok());
        assert_eq!(retrieved.unwrap().id, id);
    }

    #[tokio::test]
    async fn test_get_nonexistent_memory() {
        let storage = HotStorage::new(100);
        let id = uuid::Uuid::new_v4();
        
        let result = storage.get(id).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_delete_memory() {
        let storage = HotStorage::new(100);
        let memory = create_test_memory(MemoryStatus::Active);
        let id = memory.id;
        
        storage.add(memory).await.unwrap();
        let result = storage.delete(id).await;
        assert!(result.is_ok());
        
        let get_result = storage.get(id).await;
        assert!(get_result.is_err());
    }

    #[tokio::test]
    async fn test_list_memories() {
        let storage = HotStorage::new(100);
        let workspace_id = uuid::Uuid::new_v4();
        
        for i in 0..5 {
            let mut memory = create_test_memory(MemoryStatus::Active);
            memory.workspace_id = workspace_id;
            memory.content = MemoryContent::Text(format!("test content {}", i));
            storage.add(memory).await.unwrap();
        }
        
        let query = SearchQuery {
            workspace_id: Some(workspace_id),
            limit: 10,
            ..Default::default()
        };
        
        let results = storage.list(workspace_id, query).await.unwrap();
        assert_eq!(results.len(), 5);
    }

    #[tokio::test]
    async fn test_batch_operations() {
        let storage = HotStorage::new(100);
        
        let memories: Vec<MemoryEntry> = (0..10)
            .map(|_| create_test_memory(MemoryStatus::Active))
            .collect();
        
        let add_result = storage.batch_add(memories).await.unwrap();
        assert_eq!(add_result.success_count, 10);
        
        let ids: Vec<MemoryId> = (0..5)
            .map(|_| uuid::Uuid::new_v4())
            .collect();
        
        let delete_result = storage.batch_delete(ids).await.unwrap();
        assert_eq!(delete_result.success_count, 0); // None exist
    }

    #[tokio::test]
    async fn test_eviction() {
        let storage = HotStorage::new(3);
        
        for _i in 0..5 {
            let memory = create_test_memory(MemoryStatus::Active);
            storage.add(memory).await.unwrap();
        }
        
        assert_eq!(storage.len().await, 3);
    }
}
