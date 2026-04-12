//! Memory API - Client library for the memory management system
//!
//! This crate provides the main API entry point for the memory system,
//! implementing core traits from memory-core.

pub mod client;
pub mod error;

pub use client::{ClientConfig, MemoryClient};
pub use error::ApiError;

pub use memos_core::{
    BatchResult, DateRange, MemoryApi, MemoryContent, MemoryEntry, MemoryError, MemoryId,
    MemoryMetadata, MemoryStats, MemoryStatus, SearchApi, SearchQuery, SearchResult, Workspace,
    WorkspaceApi, WorkspaceConfig, WorkspaceId,
};
