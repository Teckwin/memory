//! Memos - A comprehensive memory management system
//!
//! This is the main entry point for the memos project, providing a unified
//! interface to all memos functionality including:
//!
//! - **Tiered Storage**: Hot, Cold, and Zombie storage tiers
//! - **Multi-index Search**: Vector, Fulltext, and Hybrid search
//! - **Lifecycle Management**: Automatic memory lifecycle transitions
//! - **Training Module**: Model training for embeddings
//!
//! # Quick Start
//!
//! ```rust,no_run
//! use memos::MemosClient;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Create a client with default configuration
//!     let client = MemosClient::new_in_memory();
//!
//!     // Create a workspace
//!     let workspace_id = client.create_workspace("default".to_string()).await?;
//!
//!     // Add a memory
//!     let memory_id = client.add_memory(&workspace_id, "Hello World", &["test"]).await?;
//!     println!("Created memory: {}", memory_id);
//!
//!     // Search memories
//!     let results = client.search("hello", &workspace_id).await?;
//!     println!("Found {} memories", results.len());
//!
//!     Ok(())
//! }
//! ```

use memos_api::MemoryClient;
use memos_core::types::*;
use memos_core::MemoryError;
use memos_core::{MemoryApi, SearchApi, WorkspaceApi};

/// Unified Memos client providing simple access to all memos functionality
pub struct MemosClient {
    inner: MemoryClient,
}

impl MemosClient {
    /// Create a new client with in-memory storage (for testing/development)
    ///
    /// Note: This method should be called outside of an async context.
    /// For use within async code, use `MemoryClient::new_in_memory()` directly.
    #[track_caller]
    pub fn new_in_memory() -> Self {
        // Check if we're already in a runtime
        if tokio::runtime::Handle::try_current().is_ok() {
            panic!("new_in_memory() cannot be called from within an async context. Use MemoryClient::new_in_memory() instead.");
        }

        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let client = MemoryClient::new_in_memory().await.unwrap();
            Self { inner: client }
        })
    }

    /// Create a new client with in-memory storage (async version)
    ///
    /// This is the preferred method when called from async code.
    pub async fn new_in_memory_async() -> Result<Self, MemoryError> {
        let client = MemoryClient::new_in_memory().await?;
        Ok(Self { inner: client })
    }

    /// Create a new workspace
    pub async fn create_workspace(&self, name: String) -> Result<WorkspaceId, MemoryError> {
        let workspace = Workspace::new(name);
        WorkspaceApi::create(&self.inner, workspace).await
    }

    /// Add a memory to a workspace
    pub async fn add_memory(
        &self,
        workspace_id: &WorkspaceId,
        content: &str,
        tags: &[&str],
    ) -> Result<MemoryId, MemoryError> {
        let mut metadata = MemoryMetadata::new(MemorySource::UserQuery {
            query: content.to_string(),
        });
        metadata.tags = tags.iter().map(|s| s.to_string()).collect();

        let entry = MemoryEntry::new(
            *workspace_id,
            MemoryContent::Text(content.to_string()),
            metadata,
        );

        MemoryApi::add(&self.inner, entry).await
    }

    /// Search memories by text query
    pub async fn search(
        &self,
        query: &str,
        workspace_id: &WorkspaceId,
    ) -> Result<Vec<SearchResult>, MemoryError> {
        let search_query = SearchQuery {
            text: Some(query.to_string()),
            workspace_id: Some(*workspace_id),
            tags: None,
            status: None,
            date_range: None,
            limit: 10,
            offset: 0,
        };

        SearchApi::search(&self.inner, search_query).await
    }

    /// List all memories in a workspace
    pub async fn list_memories(
        &self,
        workspace_id: &WorkspaceId,
    ) -> Result<Vec<SearchResult>, MemoryError> {
        let query = SearchQuery {
            workspace_id: Some(*workspace_id),
            text: None,
            tags: None,
            status: None,
            date_range: None,
            limit: 100,
            offset: 0,
        };

        SearchApi::search(&self.inner, query).await
    }

    /// Get workspace statistics
    pub async fn get_workspace_stats(
        &self,
        workspace_id: &WorkspaceId,
    ) -> Result<MemoryStats, MemoryError> {
        WorkspaceApi::stats(&self.inner, *workspace_id).await
    }

    /// Delete a memory by ID
    pub async fn delete_memory(&self, memory_id: MemoryId) -> Result<(), MemoryError> {
        MemoryApi::delete(&self.inner, memory_id).await
    }
}

pub mod prelude {
    //! Re-exports commonly used types

    pub use memos_api::client::MemoryClient;
    pub use memos_core::types::*;
    pub use memos_index::FulltextIndex;
    pub use memos_index::HybridConfig;
    pub use memos_index::HybridIndex;
    pub use memos_index::VectorIndex;
    pub use memos_lifecycle::scheduler::LifecycleScheduler;
    pub use memos_storage::cold::ColdStorage;
    pub use memos_storage::hot::HotStorage;
    pub use memos_storage::zombie::ZombieStorage;
    pub use memos_storage::UnifiedStorage;
}

// Re-export all public APIs
pub use memos_api;
pub use memos_core;
pub use memos_index;
pub use memos_lifecycle;
pub use memos_storage;
pub use memos_train;

/// Current version of memos
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version() {
        assert_eq!(VERSION, "0.1.0");
    }

    // Note: Full async tests are not supported in sync test context
    // Use examples for full integration testing
}
