//! Memory Index Module
//!
//! This module provides various indexing strategies for memory search:
//! - **Vector Index**: HNSW-based approximate nearest neighbor search
//! - **Fulltext Index**: Tantivy-based full-text search
//! - **Metadata Index**: Tag and filter-based indexing
//! - **Hybrid Index**: Combined vector + full-text search

pub mod error;
pub mod fulltext_index;
pub mod hybrid_index;
pub mod metadata_index;
pub mod vector_index;

pub use error::IndexError;
pub use fulltext_index::{FulltextConfig, FulltextIndex, FulltextIndexBuilder};
pub use hybrid_index::{HybridConfig, HybridIndex, HybridIndexBuilder};
pub use metadata_index::{MetadataConfig, MetadataIndex, MetadataIndexBuilder};
pub use vector_index::{HnswConfig, VectorIndex, VectorIndexBuilder, VectorSearchResult};

pub use memory_core::{
    DateRange, MemoryEntry, MemoryId, MemoryStatus, SearchQuery, SearchResult, WorkspaceId,
};

// Re-export commonly used types
pub use memory_core::MemoryError;
