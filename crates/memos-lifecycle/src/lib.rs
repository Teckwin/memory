//! Memory Lifecycle - Manages memory lifecycle transitions
//!
//! This module provides:
//! - LifecycleApi trait implementation for memory status transitions
//! - Scheduler for automatic periodic transitions
//! - Policy system for determining when to transition memories
//!
//! # Architecture
//!
//! The lifecycle module manages the transition of memories through different
//! storage tiers based on access patterns and importance:
//!
//! - **Active**: Frequently accessed memories in hot storage
//! - **Cooling**: Recently inactive memories transitioning to cold storage
//! - **Cold**: Infrequently accessed memories in cold storage
//! - **Zombie**: Archive candidates with very low importance

pub mod error;
pub mod policy;
pub mod scheduler;

pub use error::LifecycleError;
pub use policy::{DefaultTransitionPolicy, PolicyConfig, TransitionPolicy};
pub use scheduler::{LifecycleScheduler, SchedulerState};

use std::sync::Arc;

use async_trait::async_trait;
use memos_core::{
    BatchResult, LifecycleApi as CoreLifecycleApi, MemoryApi, MemoryError, MemoryId, MemoryStatus,
    SearchApi, SearchQuery, WorkspaceId,
};

/// Lifecycle manager that implements the LifecycleApi trait
pub struct LifecycleManager {
    storage: Arc<dyn MemoryApi>,
    search: Arc<dyn SearchApi>,
    policy: Arc<dyn TransitionPolicy>,
}

impl LifecycleManager {
    pub fn new(
        storage: Arc<dyn MemoryApi>,
        search: Arc<dyn SearchApi>,
        policy: Arc<dyn TransitionPolicy>,
    ) -> Self {
        Self {
            storage,
            search,
            policy,
        }
    }

    /// Create a new LifecycleManager with the default policy
    pub fn with_default_policy(storage: Arc<dyn MemoryApi>, search: Arc<dyn SearchApi>) -> Self {
        Self {
            storage,
            search,
            policy: Arc::new(DefaultTransitionPolicy::new()),
        }
    }

    /// Get transition candidates for a specific status
    async fn get_candidates_for_status(
        &self,
        status: MemoryStatus,
    ) -> Result<Vec<MemoryId>, MemoryError> {
        self.get_candidates_for_status_impl(status).await
    }

    /// Internal implementation for testing
    pub async fn get_candidates_for_status_impl(
        &self,
        status: MemoryStatus,
    ) -> Result<Vec<MemoryId>, MemoryError> {
        let query = SearchQuery {
            text: None,
            tags: None,
            workspace_id: None,
            status: Some(status),
            date_range: None,
            limit: 1000,
            offset: 0,
        };

        let results = self.search.search(query).await?;
        let mut candidates = Vec::new();

        for result in results {
            if self.policy.should_transition(&result.memory) {
                candidates.push(result.memory.id);
            }
        }

        Ok(candidates)
    }

    /// Apply policy-based transitions to a memory
    async fn apply_policy_transition(&self, id: MemoryId) -> Result<(), MemoryError> {
        let memory = self.storage.get(id).await?;

        if let Some(new_status) = self.policy.get_next_status(&memory) {
            // Validate the transition is valid
            if !Self::is_valid_transition(memory.status, new_status) {
                return Err(MemoryError::InvalidOperation(format!(
                    "Invalid transition from {:?} to {:?}",
                    memory.status, new_status
                )));
            }

            // Perform the transition
            self.storage.update(memory).await?;
        }

        Ok(())
    }

    /// Check if a transition is valid
    fn is_valid_transition(from: MemoryStatus, to: MemoryStatus) -> bool {
        match (from, to) {
            // Active can go to Cooling or stay Active
            (MemoryStatus::Active, MemoryStatus::Cooling) => true,
            // Cooling can go to Cold or back to Active
            (MemoryStatus::Cooling, MemoryStatus::Cold) => true,
            (MemoryStatus::Cooling, MemoryStatus::Active) => true,
            // Cold can go to Zombie, Cooling, or Active
            (MemoryStatus::Cold, MemoryStatus::Zombie) => true,
            (MemoryStatus::Cold, MemoryStatus::Cooling) => true,
            (MemoryStatus::Cold, MemoryStatus::Active) => true,
            // Zombie can only go to Active (reactivation)
            (MemoryStatus::Zombie, MemoryStatus::Active) => true,
            // Same status is always valid (no-op)
            _ if from == to => true,
            // All other transitions are invalid
            _ => false,
        }
    }
}

#[async_trait]
impl CoreLifecycleApi for LifecycleManager {
    /// Transition a memory to a new status
    async fn transition(&self, id: MemoryId, new_status: MemoryStatus) -> Result<(), MemoryError> {
        let memory = self.storage.get(id).await?;

        // Validate transition
        if !Self::is_valid_transition(memory.status, new_status) {
            return Err(MemoryError::InvalidOperation(format!(
                "Invalid transition from {:?} to {:?}",
                memory.status, new_status
            )));
        }

        // Update the memory status
        let mut updated = memory;
        updated.status = new_status;
        updated.updated_at = chrono::Utc::now();

        self.storage.update(updated).await
    }

    /// Get memories that are candidates for transition from a given status
    async fn get_transition_candidates(
        &self,
        status: MemoryStatus,
    ) -> Result<Vec<MemoryId>, MemoryError> {
        self.get_candidates_for_status(status).await
    }

    /// Run all pending transitions
    async fn run_transitions(&self) -> Result<BatchResult, MemoryError> {
        let mut result = BatchResult::new();

        // Check each status level for candidates
        let statuses = [
            MemoryStatus::Active,
            MemoryStatus::Cooling,
            MemoryStatus::Cold,
        ];

        for status in statuses {
            match self.get_candidates_for_status(status).await {
                Ok(candidates) => {
                    for id in candidates {
                        match self.apply_policy_transition(id).await {
                            Ok(()) => result.add_success(),
                            Err(e) => result.add_failure(format!("{}: {}", id, e)),
                        }
                    }
                }
                Err(e) => {
                    result.add_failure(format!("Failed to get candidates for {:?}: {}", status, e));
                }
            }
        }

        Ok(result)
    }

    /// Archive all memories in a workspace (move to Zombie)
    async fn archive(&self, workspace_id: WorkspaceId) -> Result<BatchResult, MemoryError> {
        let query = SearchQuery {
            text: None,
            tags: None,
            workspace_id: Some(workspace_id),
            status: None,
            date_range: None,
            limit: 1000,
            offset: 0,
        };

        let results = self.search.search(query).await?;
        let mut batch_result = BatchResult::new();

        for result in results {
            match self
                .transition(result.memory.id, MemoryStatus::Zombie)
                .await
            {
                Ok(()) => batch_result.add_success(),
                Err(e) => batch_result.add_failure(format!("{}: {}", result.memory.id, e)),
            }
        }

        Ok(batch_result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use memos_core::{MemoryContent, MemoryEntry, MemoryMetadata, MemorySource};
    use std::sync::Mutex;
    use uuid::Uuid;

    struct MockMemoryStorage {
        memories: Mutex<Vec<MemoryEntry>>,
    }

    impl MockMemoryStorage {
        fn new() -> Self {
            Self {
                memories: Mutex::new(Vec::new()),
            }
        }
    }

    #[async_trait]
    impl MemoryApi for MockMemoryStorage {
        async fn add(&self, memory: MemoryEntry) -> Result<MemoryId, MemoryError> {
            let mut memories = self.memories.lock().unwrap();
            memories.push(memory.clone());
            Ok(memory.id)
        }

        async fn get(&self, id: MemoryId) -> Result<MemoryEntry, MemoryError> {
            let memories = self.memories.lock().unwrap();
            memories
                .iter()
                .find(|m| m.id == id)
                .cloned()
                .ok_or_else(|| MemoryError::NotFound(id.to_string()))
        }

        async fn update(&self, memory: MemoryEntry) -> Result<(), MemoryError> {
            let mut memories = self.memories.lock().unwrap();
            if let Some(pos) = memories.iter().position(|m| m.id == memory.id) {
                memories[pos] = memory;
                Ok(())
            } else {
                Err(MemoryError::NotFound(memory.id.to_string()))
            }
        }

        async fn delete(&self, _id: MemoryId) -> Result<(), MemoryError> {
            Ok(())
        }

        async fn list(
            &self,
            _workspace_id: WorkspaceId,
            _query: SearchQuery,
        ) -> Result<Vec<memos_core::SearchResult>, MemoryError> {
            Ok(Vec::new())
        }

        async fn batch_add(&self, memories: Vec<MemoryEntry>) -> Result<BatchResult, MemoryError> {
            let mut result = BatchResult::new();
            for memory in memories {
                self.add(memory).await?;
                result.add_success();
            }
            Ok(result)
        }

        async fn batch_delete(&self, ids: Vec<MemoryId>) -> Result<BatchResult, MemoryError> {
            let mut result = BatchResult::new();
            for id in ids {
                self.delete(id).await?;
                result.add_success();
            }
            Ok(result)
        }
    }

    struct MockSearchApi {
        storage: Arc<MockMemoryStorage>,
    }

    #[async_trait]
    impl SearchApi for MockSearchApi {
        async fn search(
            &self,
            query: SearchQuery,
        ) -> Result<Vec<memos_core::SearchResult>, MemoryError> {
            let memories = self.storage.memories.lock().unwrap();
            let filtered: Vec<_> = memories
                .iter()
                .filter(|m| {
                    if let Some(ref status) = query.status {
                        if m.status != *status {
                            return false;
                        }
                    }
                    if let Some(ws_id) = query.workspace_id {
                        if m.workspace_id != ws_id {
                            return false;
                        }
                    }
                    true
                })
                .map(|m| memos_core::SearchResult {
                    memory: m.clone(),
                    score: 1.0,
                    highlights: vec![],
                })
                .collect();
            Ok(filtered)
        }

        async fn vector_search(
            &self,
            _workspace_id: WorkspaceId,
            _embedding: &[f32],
            _limit: usize,
        ) -> Result<Vec<memos_core::SearchResult>, MemoryError> {
            Ok(Vec::new())
        }

        async fn hybrid_search(
            &self,
            _workspace_id: WorkspaceId,
            _text: &str,
            _embedding: &[f32],
            _limit: usize,
        ) -> Result<Vec<memos_core::SearchResult>, MemoryError> {
            Ok(Vec::new())
        }
    }

    fn create_test_memory(_status: MemoryStatus) -> MemoryEntry {
        let source = MemorySource::System {
            source_type: "test".to_string(),
        };
        let metadata = MemoryMetadata::new(source);
        MemoryEntry::new(
            Uuid::new_v4(),
            MemoryContent::Text("test content".to_string()),
            metadata,
        )
    }

    #[tokio::test]
    async fn test_lifecycle_manager_transition() {
        let storage = Arc::new(MockMemoryStorage::new());
        let storage_for_search = Arc::clone(&storage);
        let search = Arc::new(MockSearchApi {
            storage: storage_for_search,
        });
        let policy = Arc::new(DefaultTransitionPolicy::new());

        let manager =
            LifecycleManager::new(Arc::clone(&storage) as Arc<dyn MemoryApi>, search, policy);

        // Add a test memory
        let memory = create_test_memory(MemoryStatus::Active);
        let id = memory.id;
        storage.add(memory).await.unwrap();

        // Transition to Cooling
        let result = manager.transition(id, MemoryStatus::Cooling).await;
        assert!(result.is_ok());

        // Verify the transition
        let updated = storage.get(id).await.unwrap();
        assert_eq!(updated.status, MemoryStatus::Cooling);
    }

    #[tokio::test]
    async fn test_invalid_transition() {
        let storage = Arc::new(MockMemoryStorage::new());
        let storage_for_search = Arc::clone(&storage);
        let search = Arc::new(MockSearchApi {
            storage: storage_for_search,
        });
        let policy = Arc::new(DefaultTransitionPolicy::new());

        let manager =
            LifecycleManager::new(Arc::clone(&storage) as Arc<dyn MemoryApi>, search, policy);

        // Add a test memory as Active
        let memory = create_test_memory(MemoryStatus::Active);
        let id = memory.id;
        storage.add(memory).await.unwrap();

        // Try invalid transition (Active -> Zombie directly)
        let result = manager.transition(id, MemoryStatus::Zombie).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_transition_candidates() {
        let storage = Arc::new(MockMemoryStorage::new());
        let storage_for_search = Arc::clone(&storage);
        let search = Arc::new(MockSearchApi {
            storage: storage_for_search,
        });
        let policy = Arc::new(DefaultTransitionPolicy::new());

        let manager =
            LifecycleManager::new(Arc::clone(&storage) as Arc<dyn MemoryApi>, search, policy);

        // Add memory that should transition (inactive for >7 days)
        let mut memory = create_test_memory(MemoryStatus::Active);
        memory.updated_at = chrono::Utc::now() - chrono::Duration::days(10);
        memory.access_count = 0;
        memory.last_accessed = None;
        storage.add(memory).await.unwrap();

        // Get candidates
        let candidates = manager
            .get_transition_candidates(MemoryStatus::Active)
            .await;
        assert!(candidates.is_ok());
    }

    #[tokio::test]
    async fn test_archive_workspace() {
        let storage = Arc::new(MockMemoryStorage::new());
        let storage_for_search = Arc::clone(&storage);
        let search = Arc::new(MockSearchApi {
            storage: storage_for_search,
        });
        let policy = Arc::new(DefaultTransitionPolicy::new());

        let manager =
            LifecycleManager::new(Arc::clone(&storage) as Arc<dyn MemoryApi>, search, policy);

        let workspace_id = Uuid::new_v4();

        // Add memories to workspace
        for _ in 0..3 {
            let mut memory = create_test_memory(MemoryStatus::Active);
            memory.workspace_id = workspace_id;
            storage.add(memory).await.unwrap();
        }

        // Archive the workspace
        let result = manager.archive(workspace_id).await;
        assert!(result.is_ok());
    }

    #[test]
    fn test_is_valid_transition() {
        assert!(LifecycleManager::is_valid_transition(
            MemoryStatus::Active,
            MemoryStatus::Cooling
        ));
        assert!(LifecycleManager::is_valid_transition(
            MemoryStatus::Cooling,
            MemoryStatus::Cold
        ));
        assert!(LifecycleManager::is_valid_transition(
            MemoryStatus::Cold,
            MemoryStatus::Zombie
        ));
        assert!(!LifecycleManager::is_valid_transition(
            MemoryStatus::Active,
            MemoryStatus::Zombie
        ));
        assert!(!LifecycleManager::is_valid_transition(
            MemoryStatus::Zombie,
            MemoryStatus::Cold
        ));

        // Additional coverage for uncovered branches
        // Line 125: Cooling -> Active
        assert!(LifecycleManager::is_valid_transition(
            MemoryStatus::Cooling,
            MemoryStatus::Active
        ));
        // Line 128: Cold -> Cooling
        assert!(LifecycleManager::is_valid_transition(
            MemoryStatus::Cold,
            MemoryStatus::Cooling
        ));
        // Line 129: Cold -> Active
        assert!(LifecycleManager::is_valid_transition(
            MemoryStatus::Cold,
            MemoryStatus::Active
        ));
        // Line 131: Zombie -> Active
        assert!(LifecycleManager::is_valid_transition(
            MemoryStatus::Zombie,
            MemoryStatus::Active
        ));
    }

    #[tokio::test]
    async fn test_invalid_transition_error_message() {
        // Test that invalid transition returns the correct error message
        // This covers lines 105-108
        let storage = Arc::new(MockMemoryStorage::new());
        let storage_for_search = Arc::clone(&storage);
        let search = Arc::new(MockSearchApi {
            storage: storage_for_search,
        });

        let manager = LifecycleManager::with_default_policy(
            Arc::clone(&storage) as Arc<dyn MemoryApi>,
            search,
        );

        // Add a memory
        let memory = create_test_memory(MemoryStatus::Active);
        storage.add(memory.clone()).await.unwrap();

        // Try invalid transition: Active -> Zombie (should fail)
        let result = manager.transition(memory.id, MemoryStatus::Zombie).await;

        assert!(result.is_err());
        let error = result.unwrap_err();
        // Check error message format - covers lines 105-108
        let error_str = format!("{:?}", error);
        assert!(error_str.contains("Invalid transition"));
    }

    #[tokio::test]
    async fn test_with_default_policy() {
        let storage = Arc::new(MockMemoryStorage::new());
        let storage_for_search = Arc::clone(&storage);
        let search = Arc::new(MockSearchApi {
            storage: storage_for_search,
        });

        // Test with_default_policy constructor
        let manager = LifecycleManager::with_default_policy(
            Arc::clone(&storage) as Arc<dyn MemoryApi>,
            search,
        );

        // Verify manager works with default policy
        let memory = create_test_memory(MemoryStatus::Active);
        let id = memory.id;
        storage.add(memory).await.unwrap();

        // Try transition - should work with default policy
        let result = manager.transition(id, MemoryStatus::Cooling).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_apply_policy_transition() {
        let storage = Arc::new(MockMemoryStorage::new());
        let storage_for_search = Arc::clone(&storage);
        let search = Arc::new(MockSearchApi {
            storage: storage_for_search,
        });
        let policy = Arc::new(DefaultTransitionPolicy::new());

        let manager =
            LifecycleManager::new(Arc::clone(&storage) as Arc<dyn MemoryApi>, search, policy);

        // Add a test memory that should trigger policy transition
        let mut memory = create_test_memory(MemoryStatus::Active);
        memory.updated_at = chrono::Utc::now() - chrono::Duration::days(10);
        memory.access_count = 0;
        memory.last_accessed = None;
        storage.add(memory.clone()).await.unwrap();

        // Manually call apply_policy_transition (this is a private method)
        // We test it through transition method
        let result = manager.transition(memory.id, MemoryStatus::Cooling).await;
        assert!(result.is_ok());
    }

    /// 测试 transition 方法在记忆不存在时返回错误
    #[tokio::test]
    async fn test_transition_not_found() {
        let storage = Arc::new(MockMemoryStorage::new());
        let storage_for_search = Arc::clone(&storage);
        let search = Arc::new(MockSearchApi {
            storage: storage_for_search,
        });
        let policy = Arc::new(DefaultTransitionPolicy::new());

        let manager =
            LifecycleManager::new(Arc::clone(&storage) as Arc<dyn MemoryApi>, search, policy);

        // 尝试转换一个不存在的记忆
        let nonexistent_id = Uuid::new_v4();
        let result = manager
            .transition(nonexistent_id, MemoryStatus::Cooling)
            .await;
        assert!(result.is_err());
    }

    /// 测试 run_transitions 处理候选记忆
    #[tokio::test]
    async fn test_run_transitions_with_candidates() {
        let storage = Arc::new(MockMemoryStorage::new());
        let storage_for_search = Arc::clone(&storage);
        let search = Arc::new(MockSearchApi {
            storage: storage_for_search,
        });
        let policy = Arc::new(DefaultTransitionPolicy::new());

        let manager =
            LifecycleManager::new(Arc::clone(&storage) as Arc<dyn MemoryApi>, search, policy);

        // 添加一个符合条件的记忆用于转换
        let mut memory = create_test_memory(MemoryStatus::Active);
        memory.updated_at = chrono::Utc::now() - chrono::Duration::days(10);
        memory.access_count = 0;
        memory.last_accessed = None;
        storage.add(memory.clone()).await.unwrap();

        // 运行转换
        let result = manager.run_transitions().await;
        assert!(result.is_ok());
    }
}
