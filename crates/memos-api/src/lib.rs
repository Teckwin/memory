//! Memory API - Client library for the memory management system
//!
//! This crate provides the main API entry point for the memory system,
//! implementing core traits from memory-core.
//!
//! # Usage
//!
//! ```rust,no_run
//! use memos_api::MemoryClient;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let client = MemoryClient::new_in_memory().await?;
//!     // ... use the client
//!     Ok(())
//! }
//! ```

pub mod client;
pub mod error;

pub use client::{ClientConfig, MemoryClient};
pub use error::ApiError;

// Re-export core traits and types
pub use memos_core::{
    BatchResult, DateRange, MemoryApi, MemoryContent, MemoryEntry, MemoryError, MemoryId,
    MemoryMetadata, MemoryStats, MemoryStatus, SearchApi, SearchQuery, SearchResult, Workspace,
    WorkspaceApi, WorkspaceConfig, WorkspaceId,
};
