//! Zombie storage - file-based archive for Zombie memories

use async_trait::async_trait;
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};
use tokio::sync::RwLock;

use crate::error::StorageError;
use memos_core::{
    BatchResult, MemoryApi, MemoryEntry, MemoryError, MemoryId, MemoryStatus, SearchQuery,
    SearchResult, WorkspaceId,
};

/// Zombie storage implementation using JSON line files
pub struct ZombieStorage {
    /// Base directory for archived memories
    base_path: PathBuf,
    /// In-memory index for quick lookups
    index: RwLock<ZombieIndex>,
}

/// In-memory index for zombie storage
#[derive(Default)]
struct ZombieIndex {
    /// Map from MemoryId to file path
    id_to_path: std::collections::HashMap<MemoryId, PathBuf>,
    /// Map from WorkspaceId to list of MemoryIds
    workspace_to_ids: std::collections::HashMap<WorkspaceId, Vec<MemoryId>>,
}

impl ZombieStorage {
    pub fn new(base_path: PathBuf) -> Self {
        Self {
            base_path,
            index: RwLock::new(ZombieIndex::default()),
        }
    }

    /// Initialize the storage (create directories, load index)
    pub async fn initialize(&self) -> Result<(), StorageError> {
        // Create base directory if it doesn't exist
        fs::create_dir_all(&self.base_path)?;

        // Load existing index
        self.rebuild_index().await?;

        Ok(())
    }

    /// Get the path for a memory's archive file
    fn memory_path(&self, workspace_id: WorkspaceId, memory_id: MemoryId) -> PathBuf {
        // Organize by workspace_id subdirectories
        let ws_dir = self.base_path.join(workspace_id.to_string());
        ws_dir.join(format!("{}.jsonl", memory_id))
    }

    /// Write a memory to its archive file
    fn write_memory(&self, memory: &MemoryEntry) -> Result<(), StorageError> {
        let path = self.memory_path(memory.workspace_id, memory.id);

        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&path)?;

        let mut writer = BufWriter::new(file);
        let json = serde_json::to_string(memory)?;
        writer.write_all(json.as_bytes())?;
        writer.write_all(b"\n")?;
        writer.flush()?;

        Ok(())
    }

    /// Read a memory from its archive file
    fn read_memory(&self, path: &Path) -> Result<MemoryEntry, StorageError> {
        let file = File::open(path)?;
        let mut reader = BufReader::new(file);

        let mut line = String::new();
        reader.read_line(&mut line)?;

        let memory: MemoryEntry = serde_json::from_str(&line)?;
        Ok(memory)
    }

    /// Delete a memory's archive file
    fn delete_memory_file(&self, path: &Path) -> Result<(), StorageError> {
        if path.exists() {
            fs::remove_file(path)?;
        }
        Ok(())
    }

    /// Rebuild the in-memory index from disk
    async fn rebuild_index(&self) -> Result<(), StorageError> {
        let mut index = self.index.write().await;
        index.id_to_path.clear();
        index.workspace_to_ids.clear();

        if !self.base_path.exists() {
            return Ok(());
        }

        // Iterate through workspace directories
        for entry in fs::read_dir(&self.base_path)? {
            let entry = entry?;
            let path = entry.path();

            if !path.is_dir() {
                continue;
            }

            // Parse workspace_id from directory name
            let ws_name = path.file_name().and_then(|n| n.to_str()).ok_or_else(|| {
                StorageError::ZombieError("Invalid workspace directory name".to_string())
            })?;

            let workspace_id = uuid::Uuid::parse_str(ws_name)
                .map_err(|e| StorageError::ZombieError(format!("Invalid workspace UUID: {}", e)))?;

            // Iterate through memory files
            if let Ok(entries) = fs::read_dir(&path) {
                for file_entry in entries.flatten() {
                    let file_path = file_entry.path();
                    if file_path.extension().map(|e| e == "jsonl").unwrap_or(false) {
                        // Try to read to get the memory ID for indexing
                        if let Ok(memory) = self.read_memory(&file_path) {
                            index.id_to_path.insert(memory.id, file_path.clone());
                            index
                                .workspace_to_ids
                                .entry(workspace_id)
                                .or_default()
                                .push(memory.id);
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Update index after adding a memory
    async fn index_memory(&self, memory: &MemoryEntry) {
        let path = self.memory_path(memory.workspace_id, memory.id);
        let mut index = self.index.write().await;
        index.id_to_path.insert(memory.id, path);
        index
            .workspace_to_ids
            .entry(memory.workspace_id)
            .or_default()
            .push(memory.id);
    }

    /// Remove from index after deleting a memory
    async fn unindex_memory(&self, id: MemoryId, workspace_id: WorkspaceId) {
        let mut index = self.index.write().await;

        if let Some(path) = index.id_to_path.remove(&id) {
            let _ = self.delete_memory_file(&path);
        }

        if let Some(ids) = index.workspace_to_ids.get_mut(&workspace_id) {
            ids.retain(|&x| x != id);
        }
    }
}

#[async_trait]
impl MemoryApi for ZombieStorage {
    async fn add(&self, memory: MemoryEntry) -> Result<MemoryId, MemoryError> {
        if memory.status != MemoryStatus::Zombie {
            return Err(MemoryError::InvalidOperation(
                "Zombie storage only accepts Zombie memories".to_string(),
            ));
        }

        // Write to file
        self.write_memory(&memory)
            .map_err(|e| MemoryError::StorageError(e.to_string()))?;

        // Update index
        self.index_memory(&memory).await;

        Ok(memory.id)
    }

    async fn get(&self, id: MemoryId) -> Result<MemoryEntry, MemoryError> {
        let index = self.index.read().await;

        let path = index
            .id_to_path
            .get(&id)
            .cloned()
            .ok_or_else(|| MemoryError::NotFound(id.to_string()))?;

        drop(index); // Release read lock

        // Read from file
        self.read_memory(&path)
            .map_err(|e| MemoryError::StorageError(e.to_string()))
    }

    async fn update(&self, memory: MemoryEntry) -> Result<(), MemoryError> {
        if memory.status != MemoryStatus::Zombie {
            return Err(MemoryError::InvalidOperation(
                "Zombie storage only holds Zombie memories".to_string(),
            ));
        }

        // Check if exists
        {
            let index = self.index.read().await;
            if !index.id_to_path.contains_key(&memory.id) {
                return Err(MemoryError::NotFound(memory.id.to_string()));
            }
        }

        // Write updated version
        self.write_memory(&memory)
            .map_err(|e| MemoryError::StorageError(e.to_string()))?;

        Ok(())
    }

    async fn delete(&self, id: MemoryId) -> Result<(), MemoryError> {
        // Get workspace_id before deleting
        let (workspace_id, path) = {
            let index = self.index.read().await;
            let path = index
                .id_to_path
                .get(&id)
                .cloned()
                .ok_or_else(|| MemoryError::NotFound(id.to_string()))?;

            // We need the workspace_id - read it from the file
            let memory = self
                .read_memory(&path)
                .map_err(|e| MemoryError::StorageError(e.to_string()))?;
            (memory.workspace_id, path)
        };

        // Delete file
        self.delete_memory_file(&path)
            .map_err(|e| MemoryError::StorageError(e.to_string()))?;

        // Update index
        self.unindex_memory(id, workspace_id).await;

        Ok(())
    }

    async fn list(
        &self,
        workspace_id: WorkspaceId,
        query: SearchQuery,
    ) -> Result<Vec<SearchResult>, MemoryError> {
        let index = self.index.read().await;

        let memory_ids = index
            .workspace_to_ids
            .get(&workspace_id)
            .cloned()
            .unwrap_or_default();

        let mut results = Vec::new();

        for id in memory_ids {
            if let Some(path) = index.id_to_path.get(&id) {
                if let Ok(memory) = self.read_memory(path) {
                    // Apply filters
                    if let Some(status) = query.status {
                        if memory.status != status {
                            continue;
                        }
                    }

                    if let Some(tags) = &query.tags {
                        if !tags.iter().any(|tag| memory.metadata.tags.contains(tag)) {
                            continue;
                        }
                    }

                    if let Some(text) = &query.text {
                        let content_text = memory.content.as_text().unwrap_or("");
                        if !content_text.to_lowercase().contains(&text.to_lowercase()) {
                            continue;
                        }
                    }

                    results.push(SearchResult {
                        memory,
                        score: 1.0,
                        highlights: vec![],
                    });
                }
            }
        }

        // Sort by created_at descending
        results.sort_by(|a, b| b.memory.created_at.cmp(&a.memory.created_at));

        // Apply pagination
        let start = query.offset;
        let end = (query.offset + query.limit).min(results.len());
        if start < results.len() {
            Ok(results[start..end].to_vec())
        } else {
            Ok(vec![])
        }
    }

    async fn batch_add(&self, memories: Vec<MemoryEntry>) -> Result<BatchResult, MemoryError> {
        let mut result = BatchResult::new();

        for memory in memories {
            if memory.status != MemoryStatus::Zombie {
                result.add_failure(format!("Memory {} is not Zombie", memory.id));
                continue;
            }

            match self.write_memory(&memory) {
                Ok(_) => {
                    self.index_memory(&memory).await;
                    result.add_success();
                }
                Err(e) => result.add_failure(format!("{}: {}", memory.id, e)),
            }
        }

        Ok(result)
    }

    async fn batch_delete(&self, ids: Vec<MemoryId>) -> Result<BatchResult, MemoryError> {
        let mut result = BatchResult::new();

        for id in ids {
            // Get workspace_id for indexing
            let workspace_id = {
                let index = self.index.read().await;
                if let Some(path) = index.id_to_path.get(&id) {
                    if let Ok(memory) = self.read_memory(path) {
                        Some(memory.workspace_id)
                    } else {
                        None
                    }
                } else {
                    None
                }
            };

            if let Some(ws_id) = workspace_id {
                let path = self.memory_path(ws_id, id);
                match self.delete_memory_file(&path) {
                    Ok(_) => {
                        self.unindex_memory(id, ws_id).await;
                        result.add_success();
                    }
                    Err(e) => result.add_failure(format!("{}: {}", id, e)),
                }
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
    use tempfile::tempdir;

    fn create_test_memory(status: MemoryStatus) -> MemoryEntry {
        MemoryEntry {
            id: uuid::Uuid::new_v4(),
            workspace_id: uuid::Uuid::new_v4(),
            content: MemoryContent::Text("test content".to_string()),
            embedding: None,
            metadata: MemoryMetadata::new(memos_core::MemorySource::UserQuery {
                query: "test".to_string(),
            })
            .with_tags(vec!["test".to_string()]),
            status,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            access_count: 0,
            last_accessed: None,
        }
    }

    #[tokio::test]
    async fn test_zombie_storage_initialize() {
        let temp_dir = tempdir().unwrap();
        let base_path = temp_dir.path().join("zombie");
        let storage = ZombieStorage::new(base_path);

        let result = storage.initialize().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_add_and_get_memory() {
        let temp_dir = tempdir().unwrap();
        let base_path = temp_dir.path().join("zombie");
        let storage = ZombieStorage::new(base_path);
        storage.initialize().await.unwrap();

        let memory = create_test_memory(MemoryStatus::Zombie);
        let id = memory.id;

        let add_result = storage.add(memory).await;
        assert!(add_result.is_ok());
        assert_eq!(add_result.unwrap(), id);

        let get_result = storage.get(id).await;
        assert!(get_result.is_ok());
        assert_eq!(get_result.unwrap().id, id);
    }

    #[tokio::test]
    async fn test_add_non_zombie_memory_fails() {
        let temp_dir = tempdir().unwrap();
        let base_path = temp_dir.path().join("zombie");
        let storage = ZombieStorage::new(base_path);
        storage.initialize().await.unwrap();

        let memory = create_test_memory(MemoryStatus::Active);

        let result = storage.add(memory).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_update_memory() {
        let temp_dir = tempdir().unwrap();
        let base_path = temp_dir.path().join("zombie");
        let storage = ZombieStorage::new(base_path);
        storage.initialize().await.unwrap();

        let mut memory = create_test_memory(MemoryStatus::Zombie);
        let id = memory.id;
        let workspace_id = memory.workspace_id;

        storage.add(memory.clone()).await.unwrap();

        memory.content = MemoryContent::Text("updated content".to_string());

        let update_result = storage.update(memory).await;
        assert!(update_result.is_ok());

        let get_result = storage.get(id).await.unwrap();
        assert_eq!(get_result.content.as_text(), Some("updated content"));

        // Cleanup
        let path = storage.memory_path(workspace_id, id);
        let _ = storage.delete_memory_file(&path);
    }

    #[tokio::test]
    async fn test_delete_memory() {
        let temp_dir = tempdir().unwrap();
        let base_path = temp_dir.path().join("zombie");
        let storage = ZombieStorage::new(base_path);
        storage.initialize().await.unwrap();

        let memory = create_test_memory(MemoryStatus::Zombie);
        let id = memory.id;

        storage.add(memory).await.unwrap();

        let delete_result = storage.delete(id).await;
        assert!(delete_result.is_ok());

        let get_result = storage.get(id).await;
        assert!(get_result.is_err());
    }

    #[tokio::test]
    async fn test_list_memories() {
        let temp_dir = tempdir().unwrap();
        let base_path = temp_dir.path().join("zombie");
        let storage = ZombieStorage::new(base_path);
        storage.initialize().await.unwrap();

        let workspace_id = uuid::Uuid::new_v4();

        for i in 0..5 {
            let mut memory = create_test_memory(MemoryStatus::Zombie);
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
        let temp_dir = tempdir().unwrap();
        let base_path = temp_dir.path().join("zombie");
        let storage = ZombieStorage::new(base_path);
        storage.initialize().await.unwrap();

        let memories: Vec<MemoryEntry> = (0..10)
            .map(|_| create_test_memory(MemoryStatus::Zombie))
            .collect();

        let add_result = storage.batch_add(memories).await.unwrap();
        assert_eq!(add_result.success_count, 10);

        let ids: Vec<MemoryId> = (0..5).map(|_| uuid::Uuid::new_v4()).collect();

        let delete_result = storage.batch_delete(ids).await.unwrap();
        assert_eq!(delete_result.success_count, 0); // None exist
    }

    #[tokio::test]
    async fn test_get_nonexistent_memory() {
        let temp_dir = tempdir().unwrap();
        let base_path = temp_dir.path().join("zombie");
        let storage = ZombieStorage::new(base_path);
        storage.initialize().await.unwrap();

        let id = uuid::Uuid::new_v4();
        let result = storage.get(id).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_update_nonexistent_memory() {
        let temp_dir = tempdir().unwrap();
        let base_path = temp_dir.path().join("zombie");
        let storage = ZombieStorage::new(base_path);
        storage.initialize().await.unwrap();

        let memory = create_test_memory(MemoryStatus::Zombie);
        let result = storage.update(memory).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_delete_nonexistent_memory() {
        let temp_dir = tempdir().unwrap();
        let base_path = temp_dir.path().join("zombie");
        let storage = ZombieStorage::new(base_path);
        storage.initialize().await.unwrap();

        let id = uuid::Uuid::new_v4();
        let result = storage.delete(id).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_list_with_filters() {
        let temp_dir = tempdir().unwrap();
        let base_path = temp_dir.path().join("zombie");
        let storage = ZombieStorage::new(base_path);
        storage.initialize().await.unwrap();

        let workspace_id = uuid::Uuid::new_v4();

        // Add memories with different tags
        for i in 0..3 {
            let mut memory = create_test_memory(MemoryStatus::Zombie);
            memory.workspace_id = workspace_id;
            if i == 0 {
                memory.metadata.tags = vec!["tag1".to_string()];
            } else {
                memory.metadata.tags = vec!["tag2".to_string()];
            }
            storage.add(memory).await.unwrap();
        }

        // Filter by tag
        let query = SearchQuery {
            workspace_id: Some(workspace_id),
            tags: Some(vec!["tag1".to_string()]),
            limit: 10,
            ..Default::default()
        };

        let results = storage.list(workspace_id, query).await.unwrap();
        assert_eq!(results.len(), 1);
    }

    #[tokio::test]
    async fn test_list_with_text_search() {
        let temp_dir = tempdir().unwrap();
        let base_path = temp_dir.path().join("zombie");
        let storage = ZombieStorage::new(base_path);
        storage.initialize().await.unwrap();

        let workspace_id = uuid::Uuid::new_v4();

        let mut memory = create_test_memory(MemoryStatus::Zombie);
        memory.workspace_id = workspace_id;
        memory.content = MemoryContent::Text("unique search term here".to_string());
        storage.add(memory).await.unwrap();

        let query = SearchQuery {
            workspace_id: Some(workspace_id),
            text: Some("unique search".to_string()),
            limit: 10,
            ..Default::default()
        };

        let results = storage.list(workspace_id, query).await.unwrap();
        assert_eq!(results.len(), 1);
    }

    #[tokio::test]
    async fn test_batch_add_mixed_status() {
        let temp_dir = tempdir().unwrap();
        let base_path = temp_dir.path().join("zombie");
        let storage = ZombieStorage::new(base_path);
        storage.initialize().await.unwrap();

        let zombie = create_test_memory(MemoryStatus::Zombie);
        let active = create_test_memory(MemoryStatus::Active);

        let result = storage.batch_add(vec![zombie, active]).await.unwrap();
        assert_eq!(result.success_count, 1);
        assert!(result.failure_count >= 1);
    }

    #[tokio::test]
    async fn test_batch_delete_partial() {
        let temp_dir = tempdir().unwrap();
        let base_path = temp_dir.path().join("zombie");
        let storage = ZombieStorage::new(base_path);
        storage.initialize().await.unwrap();

        let memory = create_test_memory(MemoryStatus::Zombie);
        let id = memory.id;
        storage.add(memory).await.unwrap();

        // One exists, one doesn't
        let ids = vec![id, uuid::Uuid::new_v4()];

        let result = storage.batch_delete(ids).await.unwrap();
        assert_eq!(result.success_count, 1);
        assert_eq!(result.failure_count, 1);
    }

    /// 白盒测试：验证 Zombie 记忆的读写完全对等
    #[tokio::test]
    async fn test_data_roundtrip_consistency() {
        let temp_dir = tempdir().unwrap();
        let storage = ZombieStorage::new(temp_dir.path().to_path_buf());
        storage.initialize().await.unwrap();

        let memory = create_test_memory(MemoryStatus::Zombie);
        let original_id = memory.id;
        let original_content = memory.content.clone();
        let original_metadata = memory.metadata.clone();
        let original_workspace_id = memory.workspace_id;
        let original_created_at = memory.created_at;
        let original_updated_at = memory.updated_at;
        let original_access_count = memory.access_count;

        // 存储
        storage.add(memory).await.unwrap();

        // 读取
        let retrieved = storage.get(original_id).await.unwrap();

        // 验证数据完全对等
        assert_eq!(retrieved.id, original_id);
        assert_eq!(retrieved.workspace_id, original_workspace_id);
        assert_eq!(retrieved.status, MemoryStatus::Zombie);
        assert_eq!(retrieved.access_count, original_access_count);
        assert_eq!(retrieved.created_at, original_created_at);
        assert_eq!(retrieved.updated_at, original_updated_at);

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

    /// 黑盒测试：通过 list 接口验证数据完整性
    #[tokio::test]
    async fn test_list_data_consistency() {
        let temp_dir = tempdir().unwrap();
        let storage = ZombieStorage::new(temp_dir.path().to_path_buf());
        storage.initialize().await.unwrap();

        let workspace_id = uuid::Uuid::new_v4();

        // 添加多个记忆
        for i in 0..5 {
            let mut memory = create_test_memory(MemoryStatus::Zombie);
            memory.workspace_id = workspace_id;
            memory.content = MemoryContent::Text(format!("archived content {}", i));
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
            assert_eq!(result.memory.status, MemoryStatus::Zombie);
            assert!(result.memory.content.as_text().is_some());
        }
    }

    /// Test rebuild_index with existing files
    #[tokio::test]
    async fn test_rebuild_index_with_files() {
        use std::fs;

        let temp_dir = tempdir().unwrap();
        let base_path = temp_dir.path().join("zombie");

        // Create a workspace directory with valid UUID name
        let workspace_id = uuid::Uuid::new_v4();
        let ws_dir = base_path.join(workspace_id.to_string());
        fs::create_dir_all(&ws_dir).unwrap();

        // Create a valid memory file
        let memory = create_test_memory(MemoryStatus::Zombie);
        let memory_id = memory.id;
        let memory_path = ws_dir.join(format!("{}.jsonl", memory_id));
        let json = serde_json::to_string(&memory).unwrap();
        fs::write(&memory_path, json).unwrap();

        // Create storage and initialize (this triggers rebuild_index)
        let storage = ZombieStorage::new(base_path);
        storage.initialize().await.unwrap();

        // Verify the memory can be found
        let result = storage.get(memory_id).await;
        assert!(result.is_ok());
    }

    /// Test rebuild_index with invalid workspace directory name
    #[tokio::test]
    async fn test_rebuild_index_invalid_workspace_name() {
        use std::fs;

        let temp_dir = tempdir().unwrap();
        let base_path = temp_dir.path().join("zombie");

        // Create a workspace directory with INVALID name (not a UUID)
        let invalid_dir = base_path.join("invalid-dir-name");
        fs::create_dir_all(&invalid_dir).unwrap();

        // Create storage and initialize - should handle invalid dir gracefully
        let storage = ZombieStorage::new(base_path);
        let result = storage.initialize().await;
        // Should fail because the invalid dir name can't be parsed as UUID
        assert!(result.is_err());
    }

    /// Test rebuild_index skips non-directory entries
    #[tokio::test]
    async fn test_rebuild_index_skips_files() {
        use std::fs;

        let temp_dir = tempdir().unwrap();
        let base_path = temp_dir.path().join("zombie");

        // Create the base directory first
        fs::create_dir_all(&base_path).unwrap();

        // Create a regular file in base_path (not a directory)
        fs::write(base_path.join("not_a_dir.txt"), "content").unwrap();

        // Create storage and initialize - should skip the file
        let storage = ZombieStorage::new(base_path);
        let result = storage.initialize().await;
        // Should not fail - file should be skipped
        assert!(result.is_ok());
    }
}
