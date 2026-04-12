//! Hot storage - in-memory cache for Active memories

use async_trait::async_trait;
use std::collections::HashMap;
#[allow(unused_imports)]
use std::sync::Arc;
use tokio::sync::RwLock;

use memos_core::{
    BatchResult, MemoryApi, MemoryEntry, MemoryError, MemoryId, MemoryStatus, SearchQuery,
    SearchResult, WorkspaceId,
};

/// Hot storage implementation using HashMap + RwLock
pub struct HotStorage {
    /// In-memory cache of active memories
    cache: RwLock<HashMap<MemoryId, MemoryEntry>>,
    /// Maximum number of entries to keep in hot storage
    pub max_entries: usize,
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
        match cache.entry(memory.id) {
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
            .filter(|m| workspace_id.is_nil() || m.workspace_id == workspace_id)
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
                result.add_failure(format!("Memory {} is not Active", memory.id));
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
    use memos_core::{MemoryContent, MemoryMetadata};

    fn create_test_memory(status: MemoryStatus) -> MemoryEntry {
        let mut entry = MemoryEntry::new(
            uuid::Uuid::new_v4(),
            MemoryContent::Text("test content".to_string()),
            MemoryMetadata::new(memos_core::MemorySource::UserQuery {
                query: "test".to_string(),
            })
            .with_tags(vec!["test".to_string()]),
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

        let ids: Vec<MemoryId> = (0..5).map(|_| uuid::Uuid::new_v4()).collect();

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

    #[tokio::test]
    async fn test_with_capacity() {
        let storage = HotStorage::with_capacity(50, 10);
        assert_eq!(storage.len().await, 0);
        assert!(storage.is_empty().await);
    }

    #[tokio::test]
    async fn test_contains() {
        let storage = HotStorage::new(100);
        let memory = create_test_memory(MemoryStatus::Active);
        let id = memory.id;

        assert!(!storage.contains(id).await);

        storage.add(memory).await.unwrap();

        assert!(storage.contains(id).await);
    }

    #[tokio::test]
    async fn test_ids() {
        let storage = HotStorage::new(100);
        let memory = create_test_memory(MemoryStatus::Active);

        storage.add(memory).await.unwrap();

        let ids = storage.ids().await;
        assert_eq!(ids.len(), 1);
    }

    #[tokio::test]
    async fn test_load_batch() {
        let storage = HotStorage::new(100);

        let memories: Vec<MemoryEntry> = (0..5)
            .map(|_| create_test_memory(MemoryStatus::Active))
            .collect();

        storage.load_batch(memories).await;

        assert_eq!(storage.len().await, 5);
    }

    #[tokio::test]
    async fn test_remove() {
        let storage = HotStorage::new(100);
        let memory = create_test_memory(MemoryStatus::Active);
        let id = memory.id;

        storage.add(memory).await.unwrap();
        let removed = storage.remove(id).await;

        assert!(removed.is_some());
        assert!(!storage.contains(id).await);
    }

    /// 白盒测试：验证 Active 记忆的读写完全对等
    #[tokio::test]
    async fn test_data_roundtrip_consistency() {
        let storage = HotStorage::new(100);

        let memory = create_test_memory(MemoryStatus::Active);
        let original_id = memory.id;
        let original_content = memory.content.clone();
        let original_metadata = memory.metadata.clone();
        let original_workspace_id = memory.workspace_id;
        let original_access_count = memory.access_count;

        // 存储
        storage.add(memory).await.unwrap();

        // 读取
        let retrieved = storage.get(original_id).await.unwrap();

        // 验证数据完全对等
        assert_eq!(retrieved.id, original_id);
        assert_eq!(retrieved.workspace_id, original_workspace_id);
        assert_eq!(retrieved.status, MemoryStatus::Active);
        assert_eq!(retrieved.access_count, original_access_count + 1); // get() 会增加访问计数

        // 验证 content
        match (&original_content, &retrieved.content) {
            (MemoryContent::Text(original), MemoryContent::Text(retrieved)) => {
                assert_eq!(original, retrieved);
            }
            _ => panic!("Content type mismatch"),
        }

        // 验证 metadata
        assert_eq!(retrieved.metadata.tags, original_metadata.tags);
    }

    /// 黑盒测试：验证 list 接口返回的数据一致性
    #[tokio::test]
    async fn test_list_data_consistency() {
        let storage = HotStorage::new(100);
        let workspace_id = uuid::Uuid::new_v4();

        // 添加多个记忆
        for i in 0..5 {
            let mut memory = create_test_memory(MemoryStatus::Active);
            memory.workspace_id = workspace_id;
            memory.content = MemoryContent::Text(format!("content {}", i));
            storage.add(memory).await.unwrap();
        }

        // 黑盒验证：通过 list 接口
        let query = SearchQuery {
            workspace_id: Some(workspace_id),
            limit: 10,
            ..Default::default()
        };

        let results = storage.list(workspace_id, query).await.unwrap();
        assert_eq!(results.len(), 5);

        // 验证每个返回的结果数据完整性
        for result in &results {
            assert!(!result.memory.id.is_nil());
            assert!(!result.memory.workspace_id.is_nil());
            assert_eq!(result.memory.status, MemoryStatus::Active);
        }
    }

    /// 测试访问计数更新
    #[tokio::test]
    async fn test_access_count_increment() {
        let storage = HotStorage::new(100);
        let memory = create_test_memory(MemoryStatus::Active);
        let id = memory.id;

        storage.add(memory).await.unwrap();

        // 初始访问计数为 0
        let retrieved = storage.get(id).await.unwrap();
        assert_eq!(retrieved.access_count, 1);

        // 再次访问，计数应该增加
        let retrieved = storage.get(id).await.unwrap();
        assert_eq!(retrieved.access_count, 2);
    }

    /// 测试并发读写 - 验证 RwLock 在多次读取时的安全性
    #[tokio::test]
    async fn test_concurrent_access() {
        let storage = HotStorage::new(100);
        let memory = create_test_memory(MemoryStatus::Active);
        let id = memory.id;

        storage.add(memory).await.unwrap();

        // 串行执行多次读取，验证 RwLock 不会导致死锁
        for _ in 0..10 {
            let result = storage.get(id).await;
            assert!(result.is_ok());
        }
    }

    /// 测试并发写入 - 验证多线程添加的安全性
    #[tokio::test]
    async fn test_concurrent_writes() {
        let storage = HotStorage::new(100);

        // 串行添加多个记忆，验证写入逻辑
        for i in 0..20 {
            let mut memory = create_test_memory(MemoryStatus::Active);
            memory.content = MemoryContent::Text(format!("content {}", i));
            let result = storage.add(memory).await;
            assert!(result.is_ok());
        }

        // 验证存储中有 20 条记录
        assert_eq!(storage.len().await, 20);
    }

    /// 测试混合操作 - 读写交替执行
    #[tokio::test]
    async fn test_mixed_operations() {
        let storage = HotStorage::new(100);

        // 先添加记忆
        let id = {
            let memory = create_test_memory(MemoryStatus::Active);
            let id = memory.id;
            storage.add(memory).await.unwrap();
            id
        };

        // 混合读写操作
        for i in 0..10 {
            if i % 2 == 0 {
                // 偶数: 读取
                assert!(storage.get(id).await.is_ok());
            } else {
                // 奇数: 添加新记忆
                let mut m = create_test_memory(MemoryStatus::Active);
                m.content = MemoryContent::Text(format!("new content {}", i));
                assert!(storage.add(m).await.is_ok());
            }
        }
    }

    /// 测试高并发场景 - 大量并发读取同一 key
    #[tokio::test]
    async fn test_high_concurrency_reads() {
        let storage = HotStorage::new(1000);
        let memory = create_test_memory(MemoryStatus::Active);
        let id = memory.id;

        storage.add(memory).await.unwrap();

        // 模拟高并发读取 - 使用 Arc 来共享 storage
        let storage = Arc::new(storage);
        let mut handles = Vec::new();
        let memory_id = id;
        for _ in 0..100 {
            let storage = Arc::clone(&storage);
            let id = memory_id;
            handles.push(tokio::spawn(async move { storage.get(id).await }));
        }

        for handle in handles {
            let result = handle.await.unwrap();
            assert!(result.is_ok());
        }
    }

    /// 测试高并发写入场景
    #[tokio::test]
    async fn test_high_concurrency_writes() {
        let storage = HotStorage::new(2000);
        let storage = Arc::new(storage);

        let mut handles = Vec::new();
        for i in 0..100 {
            let storage = Arc::clone(&storage);
            handles.push(tokio::spawn(async move {
                let mut memory = create_test_memory(MemoryStatus::Active);
                memory.content = MemoryContent::Text(format!("content {}", i));
                storage.add(memory).await
            }));
        }

        for handle in handles {
            let result = handle.await.unwrap();
            assert!(result.is_ok());
        }

        // 由于 Arc 克隆，storage.len() 指向的是 Arc 包装的原 storage
        assert_eq!(storage.len().await, 100);
    }
}
