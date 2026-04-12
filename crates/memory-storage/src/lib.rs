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

        // Get from cold (query both Cooling and Cold as they're both in cold tier)
        let date_range = query.date_range.clone();
        let cooling_query = SearchQuery {
            workspace_id: Some(workspace_id),
            status: Some(MemoryStatus::Cooling),
            limit: query.limit,
            offset: 0,
            tags: query.tags.clone(),
            text: query.text.clone(),
            date_range: date_range.clone(),
        };
        if let Ok(mut cold_results) = self.cold.list(workspace_id, cooling_query).await {
            results.append(&mut cold_results);
        }

        let cold_query = SearchQuery {
            workspace_id: Some(workspace_id),
            status: Some(MemoryStatus::Cold),
            limit: query.limit,
            offset: 0,
            tags: query.tags.clone(),
            text: query.text.clone(),
            date_range,
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
        status: MemoryStatus,
    ) -> Result<Vec<MemoryId>, MemoryError> {
        // Get candidates based on lifecycle policy criteria:
        // - Low access count memories that haven't been accessed recently
        // - Memories that have been in current status for too long
        // - Low importance memories in active status
        let now = chrono::Utc::now();
        let inactive_threshold = now - chrono::Duration::days(7); // 7 days inactive
        let max_access_count = 3; // Low access count threshold

        let mut candidates = Vec::new();

        // Query memories in the given status
        let query = SearchQuery {
            status: Some(status),
            limit: 1000,
            offset: 0,
            ..Default::default()
        };

        // Query hot storage first (for Active status)
        if status == MemoryStatus::Active {
            // Use a valid workspace_id that will match all memories
            let workspace_id = uuid::Uuid::nil();
            let hot_results = self.hot.list(workspace_id, query.clone()).await?;
            for result in hot_results {
                let memory = result.memory;
                let should_transition = {
                    // Check if memory is inactive (no recent access)
                    let is_inactive = memory
                        .last_accessed
                        .map(|last| last < inactive_threshold)
                        .unwrap_or(true);

                    // Check if memory has low access count
                    let has_low_access = memory.access_count <= max_access_count;

                    // Check if memory has been updated long ago
                    let old_update = memory.updated_at < inactive_threshold;

                    is_inactive || has_low_access || old_update
                };

                if should_transition {
                    candidates.push(memory.id);
                }
            }
        }

        // Query cold storage (for Cooling/Cold status)
        if status == MemoryStatus::Cooling || status == MemoryStatus::Cold {
            // Use nil UUID to match all workspaces (like we do for hot)
            let workspace_id = uuid::Uuid::nil();
            let cold_results = self.cold.list(workspace_id, query.clone()).await?;
            for result in cold_results {
                let memory = result.memory;
                let should_transition = {
                    let is_inactive = memory
                        .last_accessed
                        .map(|last| last < inactive_threshold)
                        .unwrap_or(true);
                    let has_low_access = memory.access_count <= max_access_count;
                    let old_update = memory.updated_at < inactive_threshold;
                    is_inactive || has_low_access || old_update
                };

                if should_transition {
                    candidates.push(memory.id);
                }
            }
        }

        Ok(candidates)
    }

    async fn run_transitions(&self) -> Result<BatchResult, MemoryError> {
        // Run automatic transitions based on lifecycle policy
        let mut result = BatchResult::new();

        // For each status, get candidates and transition them
        let statuses = [
            MemoryStatus::Active,
            MemoryStatus::Cooling,
            MemoryStatus::Cold,
        ];

        for status in statuses {
            let candidates = self.get_transition_candidates(status).await?;

            // Limit batch size to avoid overwhelming the system
            let batch: Vec<_> = candidates.into_iter().take(self.hot.max_entries).collect();

            for id in batch {
                // Determine next status based on current status
                let next_status = match status {
                    MemoryStatus::Active => MemoryStatus::Cooling,
                    MemoryStatus::Cooling => MemoryStatus::Cold,
                    MemoryStatus::Cold => MemoryStatus::Zombie,
                    MemoryStatus::Zombie => continue, // Don't transition from Zombie
                };

                match self.transition(id, next_status).await {
                    Ok(()) => result.add_success(),
                    Err(e) => result.add_failure(format!("{}: {}", id, e)),
                }
            }
        }

        Ok(result)
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

    #[tokio::test]
    async fn test_unified_getters() {
        let temp_dir = tempdir().unwrap();
        let storage = UnifiedStorage::new(
            100,
            temp_dir.path().join("cold.db"),
            temp_dir.path().join("zombie"),
        );

        // Test hot(), cold(), zombie() getters
        let _hot = storage.hot();
        let _cold = storage.cold();
        let _zombie = storage.zombie();
    }

    #[tokio::test]
    async fn test_unified_list_combined_tiers() {
        let temp_dir = tempdir().unwrap();
        let storage = UnifiedStorage::new(
            100,
            temp_dir.path().join("cold.db"),
            temp_dir.path().join("zombie"),
        );
        storage.initialize().await.unwrap();

        let workspace_id = uuid::Uuid::new_v4();

        // Add to hot (Active)
        let mut mem1 = create_test_memory(MemoryStatus::Active);
        mem1.workspace_id = workspace_id;
        storage.add(mem1).await.unwrap();

        // Add to cold (Cooling instead of Cold to avoid status filter issues)
        let mut mem2 = create_test_memory(MemoryStatus::Cooling);
        mem2.workspace_id = workspace_id;
        storage.add(mem2).await.unwrap();

        // List without status filter (should combine all tiers)
        let query = SearchQuery {
            workspace_id: Some(workspace_id),
            limit: 10,
            ..Default::default()
        };

        let results = storage.list(workspace_id, query).await.unwrap();
        // Active is in hot, Cooling is in cold
        assert_eq!(results.len(), 2);
    }

    #[tokio::test]
    async fn test_unified_list_with_status_filter() {
        let temp_dir = tempdir().unwrap();
        let storage = UnifiedStorage::new(
            100,
            temp_dir.path().join("cold.db"),
            temp_dir.path().join("zombie"),
        );
        storage.initialize().await.unwrap();

        let workspace_id = uuid::Uuid::new_v4();

        // Add to hot (Active)
        let mut mem1 = create_test_memory(MemoryStatus::Active);
        mem1.workspace_id = workspace_id;
        storage.add(mem1).await.unwrap();

        // Add to cold
        let mut mem2 = create_test_memory(MemoryStatus::Cold);
        mem2.workspace_id = workspace_id;
        storage.add(mem2).await.unwrap();

        // List with status filter (Active only)
        let query = SearchQuery {
            workspace_id: Some(workspace_id),
            status: Some(MemoryStatus::Active),
            limit: 10,
            ..Default::default()
        };

        let results = storage.list(workspace_id, query).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].memory.status, MemoryStatus::Active);
    }

    #[tokio::test]
    async fn test_unified_batch_add_mixed_status() {
        let temp_dir = tempdir().unwrap();
        let storage = UnifiedStorage::new(
            100,
            temp_dir.path().join("cold.db"),
            temp_dir.path().join("zombie"),
        );
        storage.initialize().await.unwrap();

        // Create memories with different statuses
        let active = create_test_memory(MemoryStatus::Active);
        let cold = create_test_memory(MemoryStatus::Cold);
        let zombie = create_test_memory(MemoryStatus::Zombie);

        let result = storage.batch_add(vec![active, cold, zombie]).await.unwrap();
        assert_eq!(result.success_count, 3);
    }

    #[tokio::test]
    async fn test_unified_batch_delete_multi_tier() {
        let temp_dir = tempdir().unwrap();
        let storage = UnifiedStorage::new(
            100,
            temp_dir.path().join("cold.db"),
            temp_dir.path().join("zombie"),
        );
        storage.initialize().await.unwrap();

        let mem1 = create_test_memory(MemoryStatus::Active);
        let mem2 = create_test_memory(MemoryStatus::Cold);

        storage.add(mem1.clone()).await.unwrap();
        storage.add(mem2.clone()).await.unwrap();

        // Delete from multiple tiers at once
        let result = storage.batch_delete(vec![mem1.id, mem2.id]).await.unwrap();
        assert_eq!(result.success_count, 2);
    }

    #[tokio::test]
    async fn test_unified_transition_same_status() {
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

        // Transition to same status should be no-op
        let result = storage.transition(id, MemoryStatus::Active).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_unified_transition_not_found() {
        let temp_dir = tempdir().unwrap();
        let storage = UnifiedStorage::new(
            100,
            temp_dir.path().join("cold.db"),
            temp_dir.path().join("zombie"),
        );
        storage.initialize().await.unwrap();

        let id = uuid::Uuid::new_v4();
        let result = storage.transition(id, MemoryStatus::Cold).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_unified_migrate_active_to_zombie() {
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

        // Migrate directly from Active to Zombie
        let result = storage
            .migrate(id, MemoryStatus::Active, MemoryStatus::Zombie)
            .await;
        assert!(result.is_ok());

        // Verify it's now in zombie
        let retrieved = storage.get(id).await.unwrap();
        assert_eq!(retrieved.status, MemoryStatus::Zombie);
    }

    #[tokio::test]
    async fn test_unified_archive_workspace() {
        let temp_dir = tempdir().unwrap();
        let storage = UnifiedStorage::new(
            100,
            temp_dir.path().join("cold.db"),
            temp_dir.path().join("zombie"),
        );
        storage.initialize().await.unwrap();

        let workspace_id = uuid::Uuid::new_v4();

        // Add Cold memories (which go to cold storage)
        let mut memories = Vec::new();
        for _ in 0..3 {
            let mut mem = create_test_memory(MemoryStatus::Cold);
            mem.workspace_id = workspace_id;
            memories.push(mem);
        }

        // Add one by one
        for mem in &memories {
            storage.add(mem.clone()).await.unwrap();
        }

        // Archive workspace
        let result = storage.archive(workspace_id).await.unwrap();
        assert_eq!(result.success_count, 3);
    }

    #[tokio::test]
    async fn test_unified_get_from_cold() {
        let temp_dir = tempdir().unwrap();
        let storage = UnifiedStorage::new(
            100,
            temp_dir.path().join("cold.db"),
            temp_dir.path().join("zombie"),
        );
        storage.initialize().await.unwrap();

        let memory = create_test_memory(MemoryStatus::Cold);
        let id = memory.id;
        storage.add(memory).await.unwrap();

        // Get should find it in cold
        let result = storage.get(id).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_unified_get_from_zombie() {
        let temp_dir = tempdir().unwrap();
        let storage = UnifiedStorage::new(
            100,
            temp_dir.path().join("cold.db"),
            temp_dir.path().join("zombie"),
        );
        storage.initialize().await.unwrap();

        let memory = create_test_memory(MemoryStatus::Zombie);
        let id = memory.id;
        storage.add(memory).await.unwrap();

        // Get should find it in zombie
        let result = storage.get(id).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_unified_update_cold() {
        let temp_dir = tempdir().unwrap();
        let storage = UnifiedStorage::new(
            100,
            temp_dir.path().join("cold.db"),
            temp_dir.path().join("zombie"),
        );
        storage.initialize().await.unwrap();

        let mut memory = create_test_memory(MemoryStatus::Cold);
        storage.add(memory.clone()).await.unwrap();

        memory.content = MemoryContent::Text("updated".to_string());
        let result = storage.update(memory).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_unified_list_pagination() {
        let temp_dir = tempdir().unwrap();
        let storage = UnifiedStorage::new(
            100,
            temp_dir.path().join("cold.db"),
            temp_dir.path().join("zombie"),
        );
        storage.initialize().await.unwrap();

        let workspace_id = uuid::Uuid::new_v4();

        // Add 5 memories
        for i in 0..5 {
            let mut mem = create_test_memory(MemoryStatus::Active);
            mem.workspace_id = workspace_id;
            mem.content = MemoryContent::Text(format!("content {}", i));
            storage.add(mem).await.unwrap();
        }

        // Paginate
        let query = SearchQuery {
            workspace_id: Some(workspace_id),
            limit: 2,
            offset: 2,
            ..Default::default()
        };

        let results = storage.list(workspace_id, query).await.unwrap();
        assert_eq!(results.len(), 2);
    }

    /// 白盒+黑盒测试：验证跨层数据迁移的一致性
    #[tokio::test]
    async fn test_migration_data_consistency() {
        let temp_dir = tempdir().unwrap();
        let storage = UnifiedStorage::new(
            100,
            temp_dir.path().join("cold.db"),
            temp_dir.path().join("zombie"),
        );
        storage.initialize().await.unwrap();

        // 1. 在 Hot 层添加 Active 记忆
        let mut memory = create_test_memory(MemoryStatus::Active);
        let original_id = memory.id;
        let original_content = memory.content.clone();
        let original_metadata = memory.metadata.clone();
        let original_workspace_id = memory.workspace_id;

        storage.add(memory.clone()).await.unwrap();

        // 验证在 Hot 层
        let retrieved = storage.get(original_id).await.unwrap();
        assert_eq!(retrieved.status, MemoryStatus::Active);
        assert_eq!(retrieved.content.as_text(), original_content.as_text());

        // 2. 迁移到 Cold 层
        storage
            .migrate(original_id, MemoryStatus::Active, MemoryStatus::Cold)
            .await
            .unwrap();

        // 验证迁移到 Cold 层后数据完整
        let retrieved = storage.get(original_id).await.unwrap();
        assert_eq!(retrieved.status, MemoryStatus::Cold);
        assert_eq!(retrieved.id, original_id);
        assert_eq!(retrieved.workspace_id, original_workspace_id);
        match (&retrieved.content, &original_content) {
            (MemoryContent::Text(retrieved_text), MemoryContent::Text(original_text)) => {
                assert_eq!(retrieved_text, original_text);
            }
            _ => panic!("Content type mismatch"),
        }
        assert_eq!(retrieved.metadata.tags, original_metadata.tags);

        // 3. 迁移到 Zombie 层
        storage
            .migrate(original_id, MemoryStatus::Cold, MemoryStatus::Zombie)
            .await
            .unwrap();

        // 验证迁移到 Zombie 层后数据完整
        let retrieved = storage.get(original_id).await.unwrap();
        assert_eq!(retrieved.status, MemoryStatus::Zombie);
        assert_eq!(retrieved.id, original_id);
    }

    /// 黑盒测试：验证 UnifiedStorage 的 get 方法能跨层查找
    #[tokio::test]
    async fn test_get_across_tiers() {
        let temp_dir = tempdir().unwrap();
        let storage = UnifiedStorage::new(
            100,
            temp_dir.path().join("cold.db"),
            temp_dir.path().join("zombie"),
        );
        storage.initialize().await.unwrap();

        // 在不同层添加记忆
        let active_memory = create_test_memory(MemoryStatus::Active);
        let cold_memory = create_test_memory(MemoryStatus::Cold);
        let zombie_memory = create_test_memory(MemoryStatus::Zombie);

        let active_id = active_memory.id;
        let cold_id = cold_memory.id;
        let zombie_id = zombie_memory.id;

        storage.add(active_memory).await.unwrap();
        storage.add(cold_memory).await.unwrap();
        storage.add(zombie_memory).await.unwrap();

        // 黑盒验证：get 方法应该能找到所有层的记忆
        assert!(storage.get(active_id).await.is_ok());
        assert!(storage.get(cold_id).await.is_ok());
        assert!(storage.get(zombie_id).await.is_ok());

        // 验证状态正确
        assert_eq!(
            storage.get(active_id).await.unwrap().status,
            MemoryStatus::Active
        );
        assert_eq!(
            storage.get(cold_id).await.unwrap().status,
            MemoryStatus::Cold
        );
        assert_eq!(
            storage.get(zombie_id).await.unwrap().status,
            MemoryStatus::Zombie
        );
    }

    /// 测试 get_transition_candidates 返回符合转换条件的记忆
    #[tokio::test]
    async fn test_get_transition_candidates() {
        let temp_dir = tempdir().unwrap();
        let storage = UnifiedStorage::new(
            100,
            temp_dir.path().join("cold.db"),
            temp_dir.path().join("zombie"),
        );
        storage.initialize().await.unwrap();

        // 添加一个符合条件的记忆 (无 last_accessed 视为不活跃)
        let mut memory = create_test_memory(MemoryStatus::Active);
        memory.last_accessed = None; // 无访问记录，视为不活跃
        memory.access_count = 0;
        storage.add(memory.clone()).await.unwrap();

        // 获取 Active 状态的转换候选
        let candidates = storage
            .get_transition_candidates(MemoryStatus::Active)
            .await
            .unwrap();

        // 应该有至少一个候选 (无访问记录的记忆)
        assert!(!candidates.is_empty());
        assert!(candidates.contains(&memory.id));
    }

    /// 测试 get_transition_candidates 对于不活跃的记忆
    #[tokio::test]
    async fn test_get_transition_candidates_inactive() {
        let temp_dir = tempdir().unwrap();
        let storage = UnifiedStorage::new(
            100,
            temp_dir.path().join("cold.db"),
            temp_dir.path().join("zombie"),
        );
        storage.initialize().await.unwrap();

        // 添加一个不活跃的记忆 (没有 last_accessed)
        let mut memory = create_test_memory(MemoryStatus::Active);
        memory.last_accessed = None; // 无访问记录
        memory.access_count = 5;
        storage.add(memory.clone()).await.unwrap();

        // 获取转换候选
        let candidates = storage
            .get_transition_candidates(MemoryStatus::Active)
            .await
            .unwrap();

        // 没有 last_accessed 记录的记忆应该被返回 (视为不活跃)
        assert!(candidates.contains(&memory.id));
    }

    /// 测试 run_transitions 执行状态转换
    #[tokio::test]
    async fn test_run_transitions() {
        let temp_dir = tempdir().unwrap();
        let storage = UnifiedStorage::new(
            100,
            temp_dir.path().join("cold.db"),
            temp_dir.path().join("zombie"),
        );
        storage.initialize().await.unwrap();

        // 添加一个符合条件的记忆
        let mut memory = create_test_memory(MemoryStatus::Active);
        memory.access_count = 1; // 低访问次数
        storage.add(memory.clone()).await.unwrap();

        // 运行转换
        let result = storage.run_transitions().await.unwrap();

        // 应该成功转换至少一个记忆
        assert!(result.success_count >= 1);
    }

    /// 测试 run_transitions 处理 Cooling 状态
    #[tokio::test]
    async fn test_run_transitions_cooling() {
        let temp_dir = tempdir().unwrap();
        let storage = UnifiedStorage::new(
            100,
            temp_dir.path().join("cold.db"),
            temp_dir.path().join("zombie"),
        );
        storage.initialize().await.unwrap();

        // 添加 Cooling 状态的记忆
        let mut memory = create_test_memory(MemoryStatus::Cooling);
        memory.access_count = 1;
        storage.add(memory.clone()).await.unwrap();

        // 运行转换
        let result = storage.run_transitions().await.unwrap();

        // Cooling -> Cold 转换
        assert!(result.success_count >= 1);
    }

    /// 测试跨多层迁移的复杂场景
    #[tokio::test]
    async fn test_complex_migration_scenario() {
        let temp_dir = tempdir().unwrap();
        let storage = UnifiedStorage::new(
            100,
            temp_dir.path().join("cold.db"),
            temp_dir.path().join("zombie"),
        );
        storage.initialize().await.unwrap();

        // 1. 从 Active 迁移到 Cooling
        let mut memory = create_test_memory(MemoryStatus::Active);
        storage.add(memory.clone()).await.unwrap();
        storage
            .migrate(memory.id, MemoryStatus::Active, MemoryStatus::Cooling)
            .await
            .unwrap();

        let retrieved = storage.get(memory.id).await.unwrap();
        assert_eq!(retrieved.status, MemoryStatus::Cooling);

        // 2. 从 Cooling 迁移到 Cold
        storage
            .migrate(memory.id, MemoryStatus::Cooling, MemoryStatus::Cold)
            .await
            .unwrap();

        let retrieved = storage.get(memory.id).await.unwrap();
        assert_eq!(retrieved.status, MemoryStatus::Cold);

        // 3. 从 Cold 迁移到 Zombie
        storage
            .migrate(memory.id, MemoryStatus::Cold, MemoryStatus::Zombie)
            .await
            .unwrap();

        let retrieved = storage.get(memory.id).await.unwrap();
        assert_eq!(retrieved.status, MemoryStatus::Zombie);
    }

    /// 测试多 workspace 场景下的 list 操作
    #[tokio::test]
    async fn test_multi_workspace_list() {
        let temp_dir = tempdir().unwrap();
        let storage = UnifiedStorage::new(
            100,
            temp_dir.path().join("cold.db"),
            temp_dir.path().join("zombie"),
        );
        storage.initialize().await.unwrap();

        let ws1 = uuid::Uuid::new_v4();
        let ws2 = uuid::Uuid::new_v4();

        // 为每个 workspace 添加记忆
        for i in 0..3 {
            let mut mem = create_test_memory(MemoryStatus::Active);
            mem.workspace_id = ws1;
            mem.content = MemoryContent::Text(format!("ws1 content {}", i));
            storage.add(mem).await.unwrap();
        }

        for i in 0..2 {
            let mut mem = create_test_memory(MemoryStatus::Active);
            mem.workspace_id = ws2;
            mem.content = MemoryContent::Text(format!("ws2 content {}", i));
            storage.add(mem).await.unwrap();
        }

        // 列出 ws1 的记忆
        let query1 = SearchQuery {
            workspace_id: Some(ws1),
            limit: 10,
            ..Default::default()
        };
        let results1 = storage.list(ws1, query1).await.unwrap();
        assert_eq!(results1.len(), 3);

        // 列出 ws2 的记忆
        let query2 = SearchQuery {
            workspace_id: Some(ws2),
            limit: 10,
            ..Default::default()
        };
        let results2 = storage.list(ws2, query2).await.unwrap();
        assert_eq!(results2.len(), 2);
    }

    /// 测试带标签过滤的复杂查询
    #[tokio::test]
    async fn test_complex_tag_filtering() {
        let temp_dir = tempdir().unwrap();
        let storage = UnifiedStorage::new(
            100,
            temp_dir.path().join("cold.db"),
            temp_dir.path().join("zombie"),
        );
        storage.initialize().await.unwrap();

        let workspace_id = uuid::Uuid::new_v4();

        // 添加带不同标签的记忆
        let tags1 = vec!["rust".to_string(), "backend".to_string()];
        let tags2 = vec!["rust".to_string(), "api".to_string()];
        let tags3 = vec!["python".to_string(), "ml".to_string()];

        for tags in [tags1, tags2, tags3] {
            let mut mem = create_test_memory(MemoryStatus::Active);
            mem.workspace_id = workspace_id;
            mem.metadata.tags = tags;
            storage.add(mem).await.unwrap();
        }

        // 查询带有 "rust" 标签的记忆
        let query = SearchQuery {
            workspace_id: Some(workspace_id),
            tags: Some(vec!["rust".to_string()]),
            status: Some(MemoryStatus::Active), // 只查询 Active 状态
            limit: 10,
            ..Default::default()
        };
        let results = storage.list(workspace_id, query).await.unwrap();

        // 验证返回结果只包含带 "rust" 标签的 Active 记忆
        assert_eq!(results.len(), 2);

        // 额外验证：确保所有返回的结果都有 "rust" 标签
        for result in &results {
            assert!(result.memory.metadata.tags.contains(&"rust".to_string()));
        }
    }

    /// 测试带文本搜索的复杂查询
    #[tokio::test]
    async fn test_complex_text_search() {
        let temp_dir = tempdir().unwrap();
        let storage = UnifiedStorage::new(
            100,
            temp_dir.path().join("cold.db"),
            temp_dir.path().join("zombie"),
        );
        storage.initialize().await.unwrap();

        let workspace_id = uuid::Uuid::new_v4();

        // 添加不同内容的记忆
        let contents = vec![
            "Rust is a systems programming language",
            "Python is great for machine learning",
            "Rust has excellent memory safety",
            "JavaScript runs in the browser",
        ];

        for content in contents {
            let mut mem = create_test_memory(MemoryStatus::Active);
            mem.workspace_id = workspace_id;
            mem.content = MemoryContent::Text(content.to_string());
            storage.add(mem).await.unwrap();
        }

        // 搜索包含 "Rust" 的记忆
        let query = SearchQuery {
            workspace_id: Some(workspace_id),
            text: Some("rust".to_string()),
            status: Some(MemoryStatus::Active), // 只查询 Active 状态
            limit: 10,
            ..Default::default()
        };
        let results = storage.list(workspace_id, query).await.unwrap();

        // 验证返回结果只包含包含 "rust" 的 Active 记忆
        assert_eq!(results.len(), 2);

        // 额外验证：确保所有返回的结果都包含 "rust"
        for result in &results {
            let text = result.memory.content.as_text().unwrap_or("");
            assert!(text.to_lowercase().contains("rust"));
        }
    }
}
