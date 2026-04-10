//! Memory Storage - Multi-tier storage for memories
//!
//! This module provides three storage tiers:
//! - **Hot Storage**: In-memory cache for Active memories (fast access)
//! - **Cold Storage**: SQLite database for Cold/Cooling memories
//! - **Zombie Storage**: File-based archive for Zombie memories
//!
//! # Architecture
//!
//! The storage module implements a tiered caching strategy:
//! - Active memories are kept in hot storage for fast access
//! - Cooling/Cold memories are persisted to SQLite
//! - Zombie memories are archived to files

pub mod cold;
pub mod error;
pub mod hot;
pub mod zombie;

pub use cold::ColdStorage;
pub use error::StorageError;
pub use hot::HotStorage;
pub use zombie::ZombieStorage;

use async_trait::async_trait;
use std::sync::Arc;

use memory_core::{
    BatchResult, LifecycleApi, MemoryApi, MemoryEntry, MemoryError, MemoryId, MemoryStatus,
    SearchQuery, SearchResult, WorkspaceId,
};

/// Unified storage that delegates to appropriate tier based on memory status
pub struct UnifiedStorage {
    hot: Arc<HotStorage>,
    cold: Arc<ColdStorage>,
    zombie: Arc<ZombieStorage>,
}

impl UnifiedStorage {
    pub fn new(
        hot_max_entries: usize,
        cold_db_path: std::path::PathBuf,
        zombie_base_path: std::path::PathBuf,
    ) -> Self {
        Self {
            hot: Arc::new(HotStorage::new(hot_max_entries)),
            cold: Arc::new(ColdStorage::new(cold_db_path)),
            zombie: Arc::new(ZombieStorage::new(zombie_base_path)),
        }
    }

    /// Initialize all storage tiers
    pub async fn initialize(&self) -> Result<(), StorageError> {
        self.cold.initialize().await?;
        self.zombie.initialize().await?;
        Ok(())
    }

    /// Get reference to hot storage
    pub fn hot(&self) -> &Arc<HotStorage> {
        &self.hot
    }

    /// Get reference to cold storage
    pub fn cold(&self) -> &Arc<ColdStorage> {
        &self.cold
    }

    /// Get reference to zombie storage
    pub fn zombie(&self) -> &Arc<ZombieStorage> {
        &self.zombie
    }

    /// Get storage tier based on memory status
    fn get_storage_for_status(&self, status: MemoryStatus) -> &dyn MemoryApi {
        match status {
            MemoryStatus::Active => self.hot.as_ref() as &dyn MemoryApi,
            MemoryStatus::Cooling | MemoryStatus::Cold => self.cold.as_ref() as &dyn MemoryApi,
            MemoryStatus::Zombie => self.zombie.as_ref() as &dyn MemoryApi,
        }
    }

    /// Move memory between tiers
    pub async fn migrate(
        &self,
        id: MemoryId,
        from_status: MemoryStatus,
        to_status: MemoryStatus,
    ) -> Result<(), MemoryError> {
        // Get from source
        let memory = match from_status {
            MemoryStatus::Active => self.hot.get(id).await,
            MemoryStatus::Cooling | MemoryStatus::Cold => self.cold.get(id).await,
            MemoryStatus::Zombie => self.zombie.get(id).await,
        }?;

        // Remove from source
        match from_status {
            MemoryStatus::Active => {
                self.hot.remove(id).await;
            }
            MemoryStatus::Cooling | MemoryStatus::Cold => {
                self.cold.delete(id).await?;
            }
            MemoryStatus::Zombie => {
                self.zombie.delete(id).await?;
            }
        }

        // Insert to destination with new status
        let mut migrated_memory = memory;
        migrated_memory.status = to_status;
        migrated_memory.updated_at = chrono::Utc::now();

        match to_status {
            MemoryStatus::Active => self.hot.add(migrated_memory).await?,
            MemoryStatus::Cooling | MemoryStatus::Cold => self.cold.add(migrated_memory).await?,
            MemoryStatus::Zombie => self.zombie.add(migrated_memory).await?,
        };

        Ok(())
    }
}

#[async_trait]
impl MemoryApi for UnifiedStorage {
    async fn add(&self, memory: MemoryEntry) -> Result<MemoryId, MemoryError> {
        let storage = self.get_storage_for_status(memory.status);
        storage.add(memory).await
    }

    async fn get(&self, id: MemoryId) -> Result<MemoryEntry, MemoryError> {
        // Try all tiers in order of likelihood
        // First check hot storage (most common for active memories)
        if self.hot.contains(id).await {
            return self.hot.get(id).await;
        }

        // Check cold storage
        if let Ok(memory) = self.cold.get(id).await {
            return Ok(memory);
        }

        // Check zombie storage
        self.zombie.get(id).await
    }

    async fn update(&self, memory: MemoryEntry) -> Result<(), MemoryError> {
        let storage = self.get_storage_for_status(memory.status);
        storage.update(memory).await
    }

    async fn delete(&self, id: MemoryId) -> Result<(), MemoryError> {
        // Try all tiers
        if self.hot.remove(id).await.is_some() {
            return Ok(());
        }

        if self.cold.delete(id).await.is_ok() {
            return Ok(());
        }

        self.zombie.delete(id).await
    }

    async fn list(
        &self,
        workspace_id: WorkspaceId,
        query: SearchQuery,
    ) -> Result<Vec<SearchResult>, MemoryError> {
        // If status is specified, query only that tier
        if let Some(status) = query.status {
            let storage = self.get_storage_for_status(status);
            return storage.list(workspace_id, query).await;
        }

        // Otherwise, query all tiers and combine
        let mut results = Vec::new();

        // Get from hot
        let hot_query = SearchQuery {
            workspace_id: Some(workspace_id),
            status: Some(MemoryStatus::Active),
            limit: query.limit,
            offset: 0,
            ..Default::default()
        };
        if let Ok(mut hot_results) = self.hot.list(workspace_id, hot_query).await {
            results.append(&mut hot_results);
        }

        // Get from cold
        let cold_query = SearchQuery {
            workspace_id: Some(workspace_id),
            status: Some(MemoryStatus::Cold),
            limit: query.limit,
            offset: 0,
            ..Default::default()
        };
        if let Ok(mut cold_results) = self.cold.list(workspace_id, cold_query).await {
            results.append(&mut cold_results);
        }

        // Get from zombie
        let zombie_query = SearchQuery {
            workspace_id: Some(workspace_id),
            status: Some(MemoryStatus::Zombie),
            limit: query.limit,
            offset: 0,
            ..Default::default()
        };
        if let Ok(mut zombie_results) = self.zombie.list(workspace_id, zombie_query).await {
            results.append(&mut zombie_results);
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
        // Group by status
        let mut active = Vec::new();
        let mut cold = Vec::new();
        let mut zombie = Vec::new();

        for memory in memories {
            match memory.status {
                MemoryStatus::Active => active.push(memory),
                MemoryStatus::Cooling | MemoryStatus::Cold => cold.push(memory),
                MemoryStatus::Zombie => zombie.push(memory),
            }
        }

        let mut result = BatchResult::new();

        if !active.is_empty() {
            let r = self.hot.batch_add(active).await?;
            result.success_count += r.success_count;
            result.failure_count += r.failure_count;
            result.errors.extend(r.errors);
        }

        if !cold.is_empty() {
            let r = self.cold.batch_add(cold).await?;
            result.success_count += r.success_count;
            result.failure_count += r.failure_count;
            result.errors.extend(r.errors);
        }

        if !zombie.is_empty() {
            let r = self.zombie.batch_add(zombie).await?;
            result.success_count += r.success_count;
            result.failure_count += r.failure_count;
            result.errors.extend(r.errors);
        }

        Ok(result)
    }

    async fn batch_delete(&self, ids: Vec<MemoryId>) -> Result<BatchResult, MemoryError> {
        // Try to delete from all tiers (ids that don't exist in a tier are ignored)
        let mut result = BatchResult::new();

        for id in ids {
            let mut deleted = false;

            if self.hot.remove(id).await.is_some() {
                result.add_success();
                deleted = true;
            }

            if self.cold.delete(id).await.is_ok() {
                result.add_success();
                deleted = true;
            }

            if self.zombie.delete(id).await.is_ok() {
                result.add_success();
                deleted = true;
            }

            if !deleted {
                result.add_failure(format!("Memory {} not found in any tier", id));
            }
        }

        Ok(result)
    }
}

#[async_trait]
impl LifecycleApi for UnifiedStorage {
    async fn transition(&self, id: MemoryId, new_status: MemoryStatus) -> Result<(), MemoryError> {
        // Determine current status by checking each tier
        let current_status = if self.hot.contains(id).await {
            MemoryStatus::Active
        } else if self.cold.get(id).await.is_ok() {
            MemoryStatus::Cold
        } else if self.zombie.get(id).await.is_ok() {
            MemoryStatus::Zombie
        } else {
            return Err(MemoryError::NotFound(id.to_string()));
        };

        if current_status == new_status {
            return Ok(());
        }

        self.migrate(id, current_status, new_status).await
    }

    async fn get_transition_candidates(
        &self,
        _status: MemoryStatus,
    ) -> Result<Vec<MemoryId>, MemoryError> {
        // For now, return empty - this would be implemented based on lifecycle policy
        // In a full implementation, this would query based on access patterns,
        // age, importance, etc.
        Ok(vec![])
    }

    async fn run_transitions(&self) -> Result<BatchResult, MemoryError> {
        // This would be called by a background job to perform automatic transitions
        // For now, return empty result
        Ok(BatchResult::new())
    }

    async fn archive(&self, workspace_id: WorkspaceId) -> Result<BatchResult, MemoryError> {
        // Move all Cold memories to Zombie in this workspace
        let query = SearchQuery {
            workspace_id: Some(workspace_id),
            status: Some(MemoryStatus::Cold),
            limit: 1000,
            offset: 0,
            ..Default::default()
        };

        let cold_memories = self.cold.list(workspace_id, query).await?;
        let mut result = BatchResult::new();

        for search_result in cold_memories {
            let memory = search_result.memory;
            if let Err(e) = self
                .migrate(memory.id, MemoryStatus::Cold, MemoryStatus::Zombie)
                .await
            {
                result.add_failure(format!("{}: {}", memory.id, e));
            } else {
                result.add_success();
            }
        }

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use memory_core::{MemoryContent, MemoryMetadata};
    use tempfile::tempdir;

    fn create_test_memory(status: MemoryStatus) -> MemoryEntry {
        MemoryEntry {
            id: uuid::Uuid::new_v4(),
            workspace_id: uuid::Uuid::new_v4(),
            content: MemoryContent::Text("test content".to_string()),
            embedding: None,
            metadata: MemoryMetadata::new(memory_core::MemorySource::UserQuery {
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
    async fn test_unified_storage_initialize() {
        let temp_dir = tempdir().unwrap();
        let hot_max = 100;
        let cold_db = temp_dir.path().join("cold.db");
        let zombie_base = temp_dir.path().join("zombie");

        let storage = UnifiedStorage::new(hot_max, cold_db, zombie_base);
        let result = storage.initialize().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_unified_add_active() {
        let temp_dir = tempdir().unwrap();
        let storage = UnifiedStorage::new(
            100,
            temp_dir.path().join("cold.db"),
            temp_dir.path().join("zombie"),
        );
        storage.initialize().await.unwrap();

        let memory = create_test_memory(MemoryStatus::Active);
        let id = memory.id;

        let result = storage.add(memory).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), id);
    }

    #[tokio::test]
    async fn test_unified_add_cold() {
        let temp_dir = tempdir().unwrap();
        let storage = UnifiedStorage::new(
            100,
            temp_dir.path().join("cold.db"),
            temp_dir.path().join("zombie"),
        );
        storage.initialize().await.unwrap();

        let memory = create_test_memory(MemoryStatus::Cold);
        let id = memory.id;

        let result = storage.add(memory).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), id);
    }

    #[tokio::test]
    async fn test_unified_get() {
        let temp_dir = tempdir().unwrap();
        let storage = UnifiedStorage::new(
            100,
            temp_dir.path().join("cold.db"),
            temp_dir.path().join("zombie"),
        );
        storage.initialize().await.unwrap();

        // Add to hot
        let memory = create_test_memory(MemoryStatus::Active);
        let id = memory.id;
        storage.add(memory).await.unwrap();

        // Get should find it
        let result = storage.get(id).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().id, id);
    }

    #[tokio::test]
    async fn test_unified_transition() {
        let temp_dir = tempdir().unwrap();
        let storage = UnifiedStorage::new(
            100,
            temp_dir.path().join("cold.db"),
            temp_dir.path().join("zombie"),
        );
        storage.initialize().await.unwrap();

        // Add as Active
        let memory = create_test_memory(MemoryStatus::Active);
        let id = memory.id;
        storage.add(memory).await.unwrap();

        // Transition to Cold
        let result = storage.transition(id, MemoryStatus::Cold).await;
        assert!(result.is_ok());

        // Should now be in cold storage
        let get_result = storage.get(id).await;
        assert!(get_result.is_ok());
        assert_eq!(get_result.unwrap().status, MemoryStatus::Cold);
    }

    #[tokio::test]
    async fn test_unified_delete() {
        let temp_dir = tempdir().unwrap();
        let storage = UnifiedStorage::new(
            100,
            temp_dir.path().join("cold.db"),
            temp_dir.path().join("zombie"),
        );
        storage.initialize().await.unwrap();

        let memory = create_test_memory(MemoryStatus::Active);
        let id = memory.id;
        storage.add(memory).await.unwrap();

        let result = storage.delete(id).await;
        assert!(result.is_ok());

        let get_result = storage.get(id).await;
        assert!(get_result.is_err());
    }
}
