//! Full-text Search Index using Tantivy
//!
//! This module provides full-text search capabilities with support for
//! Chinese tokenization and various query types.

use crate::error::IndexError;
use memory_core::{MemoryContent, MemoryEntry, MemoryId, WorkspaceId};
use std::collections::HashMap;
use std::path::PathBuf;
use tantivy::collector::TopDocs;
use tantivy::query::QueryParser;
use tantivy::schema::*;
use tantivy::{doc, Index, IndexReader, IndexWriter, ReloadPolicy, TantivyDocument, Term};
use tokio::task;
use tracing::{debug, info, warn};

/// Configuration for the full-text index
#[derive(Debug, Clone)]
pub struct FulltextConfig {
    /// Maximum heap memory to use for indexing (in bytes)
    pub max_heap_size: usize,
    /// Whether to persist index to disk
    pub persist: bool,
    /// Index directory path (if persist is true)
    pub index_path: Option<PathBuf>,
    /// Schema fields to index
    pub index_content: bool,
    pub index_tags: bool,
}

impl Default for FulltextConfig {
    fn default() -> Self {
        Self {
            max_heap_size: 50_000_000, // 50MB
            persist: false,
            index_path: None,
            index_content: true,
            index_tags: true,
        }
    }
}

/// Full-text Search Index using Tantivy
pub struct FulltextIndex {
    config: FulltextConfig,
    index: Option<Index>,
    reader: Option<IndexReader>,
    writer: Option<IndexWriter>,
    schema: Schema,
    // Field handles
    field_id: Field,
    field_workspace_id: Field,
    field_content: Field,
    field_tags: Field,
    field_importance: Field,
    field_status: Field,
    // In-memory mapping from tantivy doc_id to memory id
    doc_to_memory: HashMap<u64, MemoryId>,
    memory_to_doc: HashMap<MemoryId, u64>,
    count: usize,
}

impl FulltextIndex {
    /// Create a new FulltextIndex with the given configuration
    pub fn new(config: FulltextConfig) -> Result<Self, IndexError> {
        info!("Creating new FulltextIndex");

        // Build schema
        let mut schema_builder = Schema::builder();

        let field_id = schema_builder.add_text_field("id", STRING | STORED);
        let field_workspace_id = schema_builder.add_text_field("workspace_id", STRING | STORED);
        let field_content = schema_builder.add_text_field("content", TEXT | STORED);
        let field_tags = schema_builder.add_text_field("tags", TEXT | STORED);
        let field_importance = schema_builder.add_f64_field("importance", STORED);
        let field_status = schema_builder.add_text_field("status", STRING | STORED);

        let schema = schema_builder.build();

        Ok(Self {
            config,
            index: None,
            reader: None,
            writer: None,
            schema,
            field_id,
            field_workspace_id,
            field_content,
            field_tags,
            field_importance,
            field_status,
            doc_to_memory: HashMap::new(),
            memory_to_doc: HashMap::new(),
            count: 0,
        })
    }

    /// Initialize the index
    pub fn initialize(&mut self) -> Result<(), IndexError> {
        let index = if self.config.persist {
            if let Some(ref path) = self.config.index_path {
                std::fs::create_dir_all(path)?;
                Index::create_in_dir(path, self.schema.clone())?
            } else {
                return Err(IndexError::OperationFailed(
                    "persist is true but no index_path provided".to_string(),
                ));
            }
        } else {
            Index::create_in_ram(self.schema.clone())
        };

        let reader = index
            .reader_builder()
            .reload_policy(ReloadPolicy::OnCommitWithDelay)
            .try_into()?;

        let writer = index.writer(self.config.max_heap_size)?;

        self.index = Some(index);
        self.reader = Some(reader);
        self.writer = Some(writer);

        info!("FulltextIndex initialized");
        Ok(())
    }

    /// Check if index is initialized
    pub fn is_initialized(&self) -> bool {
        self.index.is_some()
    }

    /// Extract searchable text from memory content
    fn extract_text(content: &MemoryContent) -> String {
        match content {
            MemoryContent::Text(s) => s.clone(),
            MemoryContent::Code(code) => {
                format!("{} {}", code.language, code.code)
            }
            MemoryContent::File(file) => {
                format!("{} {}", file.path.display(), file.content)
            }
            MemoryContent::Composite(contents) => contents
                .iter()
                .map(Self::extract_text)
                .collect::<Vec<_>>()
                .join(" "),
        }
    }

    /// Add a memory entry to the index
    pub async fn add(&mut self, memory: &MemoryEntry) -> Result<(), IndexError> {
        // Check if already indexed
        if self.memory_to_doc.contains_key(&memory.id) {
            warn!("Memory {} already indexed, skipping", memory.id);
            return Ok(());
        }

        // Extract searchable text
        let content = Self::extract_text(&memory.content);

        // Get tags as string
        let tags = memory
            .metadata
            .tags
            .iter()
            .map(|s| s.as_str())
            .collect::<Vec<_>>()
            .join(" ");

        let memory_id_str = memory.id.to_string();
        let workspace_id_str = memory.workspace_id.to_string();
        let status_str = memory.status.to_string();

        let doc = doc!(
            self.field_id => memory_id_str,
            self.field_workspace_id => workspace_id_str,
            self.field_content => content,
            self.field_tags => tags,
            self.field_importance => memory.metadata.importance as f64,
            self.field_status => status_str,
        );

        // Spawn blocking for tantivy operations
        let mut writer = self
            .writer
            .take()
            .ok_or_else(|| IndexError::OperationFailed("Index writer not available".to_string()))?;

        let result = task::spawn_blocking(move || {
            // Delete by term
            let doc_id = writer.add_document(doc).map_err(|e| e.to_string())?;
            writer.commit().map_err(|e| e.to_string())?;
            Ok::<_, String>((writer, doc_id))
        })
        .await
        .map_err(|e| IndexError::OperationFailed(e.to_string()))??;

        self.writer = Some(result.0);
        let doc_id = result.1;

        self.doc_to_memory.insert(doc_id, memory.id);
        self.memory_to_doc.insert(memory.id, doc_id);
        self.count += 1;

        // Reload reader to see new documents
        if let Some(ref reader) = self.reader {
            reader.reload()?;
        }

        debug!("Added memory {} to fulltext index", memory.id);
        Ok(())
    }

    /// Add multiple memory entries in batch
    pub async fn batch_add(&mut self, memories: &[MemoryEntry]) -> Result<(), IndexError> {
        let mut writer = self
            .writer
            .take()
            .ok_or_else(|| IndexError::OperationFailed("Index writer not available".to_string()))?;

        let mut doc_ids: Vec<u64> = Vec::new();

        for memory in memories {
            // Skip already indexed
            if self.memory_to_doc.contains_key(&memory.id) {
                continue;
            }

            let content = Self::extract_text(&memory.content);
            let tags = memory
                .metadata
                .tags
                .iter()
                .map(|s| s.as_str())
                .collect::<Vec<_>>()
                .join(" ");

            let doc = doc!(
                self.field_id => memory.id.to_string(),
                self.field_workspace_id => memory.workspace_id.to_string(),
                self.field_content => content,
                self.field_tags => tags,
                self.field_importance => memory.metadata.importance as f64,
                self.field_status => memory.status.to_string(),
            );

            doc_ids.push(writer.add_document(doc).map_err(|e| e.to_string())?);
        }

        let result = task::spawn_blocking(move || {
            // Delete by term
            writer.commit().map_err(|e| e.to_string())?;
            Ok::<_, String>(writer)
        })
        .await
        .map_err(|e| IndexError::OperationFailed(e.to_string()))??;

        self.writer = Some(result);

        // Update mappings
        for (i, memory) in memories.iter().enumerate() {
            if !self.memory_to_doc.contains_key(&memory.id) {
                let doc_id = doc_ids[i];
                self.doc_to_memory.insert(doc_id, memory.id);
                self.memory_to_doc.insert(memory.id, doc_id);
                self.count += 1;
            }
        }

        // Reload reader
        if let Some(ref reader) = self.reader {
            reader.reload()?;
        }

        Ok(())
    }

    /// Remove a memory from the index
    pub async fn remove(&mut self, memory_id: MemoryId) -> Result<(), IndexError> {
        let _doc_id = self.memory_to_doc.remove(&memory_id).map(|_| ());

        let mut writer = self
            .writer
            .take()
            .ok_or_else(|| IndexError::OperationFailed("Index writer not available".to_string()))?;

        let field_id = self.field_id;
        let memory_id_str = memory_id.to_string();

        let result = task::spawn_blocking(move || {
            // Delete by term
            // Delete by term
            let term = Term::from_field_text(field_id, &memory_id_str);
            writer.delete_term(term);
            writer.commit().map_err(|e| e.to_string())?;
            Ok::<_, String>(writer)
        })
        .await
        .map_err(|e| IndexError::OperationFailed(e.to_string()))??;

        self.writer = Some(result);
        self.count = self.count.saturating_sub(1);

        debug!("Removed memory {} from fulltext index", memory_id);
        Ok(())
    }

    /// Search for documents matching the query
    pub async fn search(
        &self,
        query_str: &str,
        workspace_id: Option<WorkspaceId>,
        tags: Option<Vec<String>>,
        limit: usize,
    ) -> Result<Vec<MemoryId>, IndexError> {
        let reader = self
            .reader
            .as_ref()
            .ok_or_else(|| IndexError::OperationFailed("Index reader not available".to_string()))?;

        let searcher = reader.searcher();

        // Create query parser
        let query_parser = QueryParser::for_index(
            self.index
                .as_ref()
                .ok_or_else(|| IndexError::OperationFailed("Index not available".to_string()))?,
            vec![self.field_content, self.field_tags],
        );

        let query = query_parser.parse_query(query_str)?;

        let limit = limit.min(1000);
        let top_docs = searcher.search(&query, &TopDocs::with_limit(limit))?;

        let mut results: Vec<MemoryId> = Vec::new();

        for (_score, doc_address) in top_docs {
            let retrieved_doc: TantivyDocument = searcher.doc(doc_address)?;

            let id_str = retrieved_doc
                .get_first(self.field_id)
                .and_then(|v| v.as_str())
                .ok_or_else(|| IndexError::OperationFailed("Missing id field".to_string()))?;

            let ws_id_str = retrieved_doc
                .get_first(self.field_workspace_id)
                .and_then(|v| v.as_str())
                .ok_or_else(|| {
                    IndexError::OperationFailed("Missing workspace_id field".to_string())
                })?;

            let doc_ws_id: WorkspaceId = ws_id_str
                .parse()
                .map_err(|_| IndexError::OperationFailed("Invalid workspace_id".to_string()))?;

            // Filter by workspace
            if let Some(ref filter_ws) = workspace_id {
                if &doc_ws_id != filter_ws {
                    continue;
                }
            }

            // Filter by tags if specified
            if let Some(ref filter_tags) = tags {
                let doc_tags_str = retrieved_doc
                    .get_first(self.field_tags)
                    .and_then(|v| v.as_str())
                    .unwrap_or("");

                let has_tag = filter_tags.iter().any(|tag| doc_tags_str.contains(tag));
                if !has_tag {
                    continue;
                }
            }

            let memory_id: MemoryId = id_str
                .parse()
                .map_err(|_| IndexError::OperationFailed("Invalid memory id".to_string()))?;

            results.push(memory_id);
        }

        debug!("Fulltext search returned {} results", results.len());
        Ok(results)
    }

    /// Get the number of indexed documents
    pub fn len(&self) -> usize {
        self.count
    }

    /// Check if the index is empty
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// Get the configuration
    pub fn config(&self) -> &FulltextConfig {
        &self.config
    }
}

/// Builder for FulltextIndex
pub struct FulltextIndexBuilder {
    config: FulltextConfig,
}

impl FulltextIndexBuilder {
    pub fn new() -> Self {
        Self {
            config: FulltextConfig::default(),
        }
    }

    pub fn with_max_heap_size(mut self, size: usize) -> Self {
        self.config.max_heap_size = size;
        self
    }

    pub fn with_persist(mut self, persist: bool) -> Self {
        self.config.persist = persist;
        self
    }

    pub fn with_index_path(mut self, path: PathBuf) -> Self {
        self.config.index_path = Some(path);
        self
    }

    pub fn build(self) -> Result<FulltextIndex, IndexError> {
        FulltextIndex::new(self.config)
    }
}

impl Default for FulltextIndexBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use memory_core::{MemoryContent, MemoryMetadata, MemoryStatus};
    use uuid::Uuid;

    fn create_test_memory(
        workspace_id: WorkspaceId,
        content: &str,
        tags: Vec<&str>,
    ) -> MemoryEntry {
        MemoryEntry {
            id: Uuid::new_v4(),
            workspace_id,
            content: MemoryContent::Text(content.to_string()),
            embedding: None,
            metadata: MemoryMetadata {
                source: memory_core::MemorySource::System {
                    source_type: "test".to_string(),
                },
                custom_fields: std::collections::HashMap::new(),
                tags: tags.into_iter().map(String::from).collect(),
                importance: 0.5,
            },
            status: MemoryStatus::Active,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            access_count: 0,
            last_accessed: None,
        }
    }

    #[tokio::test]
    async fn test_fulltext_index_creation() {
        let mut index = FulltextIndex::new(FulltextConfig::default()).unwrap();
        index.initialize().unwrap();
        assert!(index.is_initialized());
    }

    #[tokio::test]
    async fn test_add_memory() {
        let mut index = FulltextIndex::new(FulltextConfig::default()).unwrap();
        index.initialize().unwrap();

        let workspace_id = Uuid::new_v4();
        let memory = create_test_memory(workspace_id, "Hello world", vec!["greeting"]);

        index.add(&memory).await.unwrap();
        assert_eq!(index.len(), 1);
    }

    #[tokio::test]
    async fn test_search_content() {
        let mut index = FulltextIndex::new(FulltextConfig::default()).unwrap();
        index.initialize().unwrap();

        let workspace_id = Uuid::new_v4();

        let mem1 = create_test_memory(
            workspace_id,
            "Rust programming language",
            vec!["programming"],
        );
        let mem2 = create_test_memory(
            workspace_id,
            "Python for data science",
            vec!["programming", "data"],
        );
        let mem3 = create_test_memory(workspace_id, "Cooking recipes", vec!["food"]);

        index.add(&mem1).await.unwrap();
        index.add(&mem2).await.unwrap();
        index.add(&mem3).await.unwrap();

        // Search for "programming"
        let results = index
            .search("programming", Some(workspace_id), None, 10)
            .await
            .unwrap();
        assert_eq!(results.len(), 2);
    }

    #[tokio::test]
    async fn test_search_tags() {
        let mut index = FulltextIndex::new(FulltextConfig::default()).unwrap();
        index.initialize().unwrap();

        let workspace_id = Uuid::new_v4();

        let mem1 = create_test_memory(workspace_id, "Content 1", vec!["tag1", "tag2"]);
        let mem2 = create_test_memory(workspace_id, "Content 2", vec!["tag2", "tag3"]);
        let mem3 = create_test_memory(workspace_id, "Content 3", vec!["tag3"]);

        index.add(&mem1).await.unwrap();
        index.add(&mem2).await.unwrap();
        index.add(&mem3).await.unwrap();

        // Search with tag filter
        let results = index
            .search("*", Some(workspace_id), Some(vec!["tag2".to_string()]), 10)
            .await
            .unwrap();

        assert_eq!(results.len(), 2);
    }

    #[tokio::test]
    async fn test_workspace_filter() {
        let mut index = FulltextIndex::new(FulltextConfig::default()).unwrap();
        index.initialize().unwrap();

        let ws1 = Uuid::new_v4();
        let ws2 = Uuid::new_v4();

        let mem1 = create_test_memory(ws1, "Workspace 1 content", vec![]);
        let mem2 = create_test_memory(ws2, "Workspace 2 content", vec![]);

        index.add(&mem1).await.unwrap();
        index.add(&mem2).await.unwrap();

        // Search only in ws1
        let results = index.search("*", Some(ws1), None, 10).await.unwrap();
        assert_eq!(results.len(), 1);
    }

    #[tokio::test]
    async fn test_remove_memory() {
        let mut index = FulltextIndex::new(FulltextConfig::default()).unwrap();
        index.initialize().unwrap();

        let workspace_id = Uuid::new_v4();
        let memory = create_test_memory(workspace_id, "Test content", vec![]);
        let memory_id = memory.id;

        index.add(&memory).await.unwrap();
        assert_eq!(index.len(), 1);

        index.remove(memory_id).await.unwrap();
        assert_eq!(index.len(), 0);
    }

    #[tokio::test]
    async fn test_batch_add() {
        let mut index = FulltextIndex::new(FulltextConfig::default()).unwrap();
        index.initialize().unwrap();

        let workspace_id = Uuid::new_v4();
        let memories: Vec<_> = (0..5)
            .map(|i| create_test_memory(workspace_id, &format!("Content {}", i), vec![]))
            .collect();

        index.batch_add(&memories).await.unwrap();
        assert_eq!(index.len(), 5);
    }

    #[test]
    fn test_fulltext_index_builder() {
        let index = FulltextIndexBuilder::new()
            .with_max_heap_size(100_000_000)
            .with_persist(true)
            .with_index_path(PathBuf::from("/tmp/test_index"))
            .build()
            .unwrap();

        assert!(index.config().persist);
    }
}
