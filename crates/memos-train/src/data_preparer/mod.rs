//! Data Preparer - Prepares training data from memory entries

use crate::error::TrainError;
use memos_core::{MemoryContent, MemoryEntry, MemoryStatus, SearchQuery, TrainData, WorkspaceId};

/// Prepares training data from memory entries
pub struct DataPreparer {
    // Configuration options
    min_text_length: usize,
    include_code: bool,
    include_files: bool,
    extract_labels: bool,
}

impl Default for DataPreparer {
    fn default() -> Self {
        Self::new()
    }
}

impl DataPreparer {
    /// Create a new DataPreparer with default settings
    pub fn new() -> Self {
        Self {
            min_text_length: 10,
            include_code: true,
            include_files: true,
            extract_labels: true,
        }
    }

    /// Set minimum text length for training data
    pub fn with_min_text_length(mut self, length: usize) -> Self {
        self.min_text_length = length;
        self
    }

    /// Set whether to include code content
    pub fn with_include_code(mut self, include: bool) -> Self {
        self.include_code = include;
        self
    }

    /// Set whether to include file content
    pub fn with_include_files(mut self, include: bool) -> Self {
        self.include_files = include;
        self
    }

    /// Set whether to extract labels from tags
    pub fn with_extract_labels(mut self, extract: bool) -> Self {
        self.extract_labels = extract;
        self
    }

    /// Prepare training data from a list of memory entries
    pub fn prepare(&self, memories: Vec<MemoryEntry>) -> Result<TrainData, TrainError> {
        let mut texts = Vec::new();
        let mut labels = if self.extract_labels {
            Some(Vec::new())
        } else {
            None
        };

        for memory in memories {
            // Extract text content from memory
            if let Some(text) = self.extract_text(&memory.content) {
                if text.len() >= self.min_text_length {
                    texts.push(text);

                    // Extract labels if enabled
                    if self.extract_labels {
                        if let Some(ref mut label_vec) = labels {
                            // Use tags as labels, or default to workspace name
                            if !memory.metadata.tags.is_empty() {
                                label_vec.push(memory.metadata.tags.join(","));
                            } else {
                                label_vec.push("default".to_string());
                            }
                        }
                    }
                }
            }
        }

        if texts.is_empty() {
            return Err(TrainError::InsufficientData(
                "No valid text content found in memories".to_string(),
            ));
        }

        Ok(TrainData {
            texts,
            labels,
            embeddings: None,
        })
    }

    /// Extract text from memory content
    fn extract_text(&self, content: &MemoryContent) -> Option<String> {
        match content {
            MemoryContent::Text(text) => Some(text.clone()),
            MemoryContent::Code(code) => {
                if self.include_code {
                    Some(format!("{}: {}", code.language, code.code))
                } else {
                    None
                }
            }
            MemoryContent::File(file) => {
                if self.include_files {
                    Some(file.content.clone())
                } else {
                    None
                }
            }
            MemoryContent::Composite(contents) => {
                let mut combined = String::new();
                for c in contents {
                    if let Some(text) = self.extract_text(c) {
                        combined.push_str(&text);
                        combined.push('\n');
                    }
                }
                if combined.is_empty() {
                    None
                } else {
                    Some(combined)
                }
            }
        }
    }

    /// Create a search query for fetching memories for training
    pub fn create_search_query(&self, workspace_id: WorkspaceId) -> SearchQuery {
        SearchQuery {
            text: None,
            tags: None,
            workspace_id: Some(workspace_id),
            status: Some(MemoryStatus::Active),
            date_range: None,
            limit: 10000,
            offset: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use memos_core::MemoryMetadata;
    use uuid::Uuid;

    fn create_test_memory(text: &str, tags: Vec<&str>) -> MemoryEntry {
        let workspace_id = Uuid::new_v4();
        MemoryEntry::new(
            workspace_id,
            MemoryContent::Text(text.to_string()),
            MemoryMetadata::new(memos_core::MemorySource::System {
                source_type: "test".to_string(),
            })
            .with_tags(tags.into_iter().map(|s| s.to_string()).collect()),
        )
    }

    #[test]
    fn test_prepare_basic_text() {
        let preparer = DataPreparer::new();
        let memories = vec![
            create_test_memory("This is a sample memory for training", vec!["tag1"]),
            create_test_memory("Another memory with different content", vec!["tag2"]),
        ];

        let result = preparer.prepare(memories);
        assert!(result.is_ok());
        let data = result.unwrap();
        assert_eq!(data.texts.len(), 2);
        assert!(data.labels.is_some());
    }

    #[test]
    fn test_prepare_empty_memories() {
        let preparer = DataPreparer::new();
        let memories: Vec<MemoryEntry> = vec![];

        let result = preparer.prepare(memories);
        assert!(result.is_err());
    }

    #[test]
    fn test_prepare_short_text_filtered() {
        let preparer = DataPreparer::new().with_min_text_length(50);
        let memories = vec![
            create_test_memory("Short", vec![]),
            create_test_memory(
                "This is a much longer text that should pass the filter",
                vec![],
            ),
        ];

        let result = preparer.prepare(memories);
        assert!(result.is_ok());
        let data = result.unwrap();
        assert_eq!(data.texts.len(), 1);
    }

    #[test]
    fn test_extract_text_code() {
        let preparer = DataPreparer::new().with_include_code(true);
        let memory = MemoryEntry::new(
            Uuid::new_v4(),
            MemoryContent::Code(memos_core::CodeContent {
                language: "rust".to_string(),
                code: "fn main() {}".to_string(),
                ast_hash: None,
            }),
            MemoryMetadata::new(memos_core::MemorySource::System {
                source_type: "test".to_string(),
            }),
        );

        let result = preparer.prepare(vec![memory]);
        assert!(result.is_ok());
    }

    #[test]
    fn test_extract_labels_disabled() {
        let preparer = DataPreparer::new().with_extract_labels(false);
        let memories = vec![create_test_memory(
            "Sample text for training",
            vec!["tag1", "tag2"],
        )];

        let result = preparer.prepare(memories);
        assert!(result.is_ok());
        let data = result.unwrap();
        assert!(data.labels.is_none());
    }

    #[test]
    fn test_create_search_query() {
        let preparer = DataPreparer::new();
        let workspace_id = Uuid::new_v4();
        let query = preparer.create_search_query(workspace_id);

        assert_eq!(query.workspace_id, Some(workspace_id));
        assert_eq!(query.status, Some(MemoryStatus::Active));
    }

    #[test]
    fn test_prepare_with_special_characters() {
        // Test handling of special characters in text
        let preparer = DataPreparer::new();
        let memories = vec![create_test_memory(
            "Test with <script>alert('xss')</script> and \"quotes\" and 'apostrophes'",
            vec![],
        )];

        let result = preparer.prepare(memories);
        assert!(result.is_ok());
        let data = result.unwrap();
        assert_eq!(data.texts.len(), 1);
    }

    #[test]
    fn test_prepare_with_unicode() {
        // Test handling of unicode characters
        let preparer = DataPreparer::new();
        let memories = vec![create_test_memory(
            "Unicode test: 你好世界 🌍 émoji",
            vec!["unicode"],
        )];

        let result = preparer.prepare(memories);
        assert!(result.is_ok());
    }

    #[test]
    fn test_prepare_with_empty_tags() {
        // Test with empty tags list
        let preparer = DataPreparer::new();
        let memories = vec![create_test_memory("Test content", vec![])];

        let result = preparer.prepare(memories.clone());
        assert!(result.is_ok());

        // With label extraction enabled
        let result2 = preparer.prepare(memories);
        assert!(result2.is_ok());
    }

    #[test]
    fn test_prepare_with_very_long_text() {
        // Test with very long text (boundary condition)
        let preparer = DataPreparer::new();
        let long_text = "word ".repeat(10000); // Very long text
        let memories = vec![create_test_memory(&long_text, vec!["long"])];

        let result = preparer.prepare(memories);
        assert!(result.is_ok());
    }

    #[test]
    fn test_prepare_max_memories() {
        // Test boundary: max_memories limit is not exposed as builder method
        // Instead test that prepare handles large datasets
        let preparer = DataPreparer::new();
        let memories: Vec<_> = (0..100)
            .map(|i| create_test_memory(&format!("Memory number {}", i), vec![]))
            .collect();

        let result = preparer.prepare(memories);
        assert!(result.is_ok());
        // Should process all memories
        let data = result.unwrap();
        assert_eq!(data.texts.len(), 100);
    }
}
