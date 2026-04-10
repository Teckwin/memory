//! Memory API - Client library for the memory management system
//!
//! This crate provides the main API entry point for the memory system,
//! implementing core traits from memory-core.

pub mod error;
pub mod client;

pub use error::ApiError;
pub use client::{MemoryClient, ClientConfig};

pub use memory_core::{
    MemoryApi, SearchApi, WorkspaceApi,
    MemoryEntry, MemoryId, WorkspaceId, Workspace,
    MemoryContent, MemoryMetadata, MemoryStatus,
    SearchQuery, SearchResult, BatchResult, MemoryStats,
    WorkspaceConfig, DateRange,
    MemoryError,
};
