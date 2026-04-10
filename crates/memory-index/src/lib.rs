//! Memory Index Module
//!
//! This module provides various indexing strategies for memory search:
//! - **Vector Index**: HNSW-based approximate nearest neighbor search
//! - **Fulltext Index**: Tantivy-based full-text search
//! - **Metadata Index**: Tag and filter-based indexing
//! - **Hybrid Index**: Combined vector + full-text search

pub mod error;
pub mod vector_index;
pub mod fulltext_index;
pub mod metadata_index;
pub mod hybrid_index;

pub use error::IndexError;
pub use vector_index::{VectorIndex, HnswConfig, VectorSearchResult, VectorIndexBuilder};
pub use fulltext_index::{FulltextIndex, FulltextConfig, FulltextIndexBuilder};
pub use metadata_index::{MetadataIndex, MetadataConfig, MetadataIndexBuilder};
pub use hybrid_index::{HybridIndex, HybridConfig, HybridIndexBuilder};

pub use memory_core::{
    MemoryEntry, MemoryId, WorkspaceId,
    SearchQuery, SearchResult,
    MemoryStatus, DateRange,
};

// Re-export commonly used types
pub use memory_core::MemoryError;
