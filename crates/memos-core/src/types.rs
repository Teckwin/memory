//! Core memory types

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::path::PathBuf;
use uuid::Uuid;

/// Unique identifier for a memory entry
pub type MemoryId = Uuid;

/// Unique identifier for a workspace
pub type WorkspaceId = Uuid;

/// Memory entry - the core data structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEntry {
    pub id: MemoryId,
    pub workspace_id: WorkspaceId,
    pub content: MemoryContent,
    pub embedding: Option<Vec<f32>>,
    pub metadata: MemoryMetadata,
    pub status: MemoryStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub access_count: u64,
    pub last_accessed: Option<DateTime<Utc>>,
}

impl MemoryEntry {
    pub fn new(
        workspace_id: WorkspaceId,
        content: MemoryContent,
        metadata: MemoryMetadata,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            workspace_id,
            content,
            embedding: None,
            metadata,
            status: MemoryStatus::Active,
            created_at: now,
            updated_at: now,
            access_count: 0,
            last_accessed: None,
        }
    }

    pub fn increment_access(&mut self) {
        self.access_count += 1;
        self.last_accessed = Some(Utc::now());
    }
}

/// Memory content types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum MemoryContent {
    Text(String),
    Code(CodeContent),
    File(FileContent),
    Composite(Vec<MemoryContent>),
}

impl MemoryContent {
    pub fn text<S: Into<String>>(s: S) -> Self {
        MemoryContent::Text(s.into())
    }

    pub fn code<S: Into<String>>(language: S, code: S) -> Self {
        MemoryContent::Code(CodeContent {
            language: language.into(),
            code: code.into(),
            ast_hash: None,
        })
    }

    pub fn as_text(&self) -> Option<&str> {
        match self {
            MemoryContent::Text(s) => Some(s),
            _ => None,
        }
    }
}

/// Code content with language and optional AST hash
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeContent {
    pub language: String,
    pub code: String,
    pub ast_hash: Option<String>,
}

/// File content with path and type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileContent {
    pub path: PathBuf,
    pub content: String,
    pub file_type: FileType,
}

/// File type enumeration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum FileType {
    SourceCode,
    Documentation,
    Configuration,
    Data,
    Other,
}

impl FileType {
    pub fn from_extension(ext: &str) -> Self {
        match ext.to_lowercase().as_str() {
            // Source code
            "rs" | "js" | "ts" | "jsx" | "tsx" | "py" | "go" | "java" | "c" | "cpp" | "h"
            | "hpp" | "cs" | "rb" | "php" | "swift" | "kt" | "scala" | "vue" | "svelte" => {
                FileType::SourceCode
            }
            // Documentation
            "md" | "txt" | "rst" | "adoc" | "tex" => FileType::Documentation,
            // Configuration
            "json" | "yaml" | "yml" | "toml" | "xml" | "ini" | "env" | "conf" | "config" => {
                FileType::Configuration
            }
            // Data
            "csv" | "tsv" | "sql" | "parquet" | "arrow" => FileType::Data,
            _ => FileType::Other,
        }
    }
}

/// Memory metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryMetadata {
    pub source: MemorySource,
    pub tags: Vec<String>,
    pub custom_fields: HashMap<String, String>,
    pub importance: f32,
}

impl MemoryMetadata {
    pub fn new(source: MemorySource) -> Self {
        Self {
            source,
            tags: Vec::new(),
            custom_fields: HashMap::new(),
            importance: 0.5,
        }
    }

    pub fn with_importance(mut self, importance: f32) -> Self {
        self.importance = importance.clamp(0.0, 1.0);
        self
    }

    pub fn with_tags(mut self, tags: Vec<String>) -> Self {
        self.tags = tags;
        self
    }

    pub fn set_custom_field(&mut self, key: &str, value: &str) {
        self.custom_fields
            .insert(key.to_string(), value.to_string());
    }
}

/// Memory source origin
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum MemorySource {
    File { path: PathBuf },
    UserQuery { query: String },
    System { source_type: String },
    Training { model_id: Uuid },
}

/// Memory lifecycle status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum MemoryStatus {
    #[default]
    Active,
    Cooling,
    Cold,
    Zombie,
}

impl fmt::Display for MemoryStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MemoryStatus::Active => write!(f, "Active"),
            MemoryStatus::Cooling => write!(f, "Cooling"),
            MemoryStatus::Cold => write!(f, "Cold"),
            MemoryStatus::Zombie => write!(f, "Zombie"),
        }
    }
}

/// Workspace - container for memories
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workspace {
    pub id: WorkspaceId,
    pub name: String,
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub config: WorkspaceConfig,
}

impl Workspace {
    pub fn new(name: String) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            name,
            description: None,
            created_at: now,
            updated_at: now,
            config: WorkspaceConfig::default(),
        }
    }

    pub fn with_description(mut self, description: String) -> Self {
        self.description = Some(description);
        self
    }
}

/// Workspace configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceConfig {
    pub max_memory_size: usize,
    pub retention_days: u32,
    pub auto_archive: bool,
    pub embedding_model: Option<String>,
}

impl Default for WorkspaceConfig {
    fn default() -> Self {
        Self {
            max_memory_size: 10_000_000, // 10MB
            retention_days: 90,
            auto_archive: true,
            embedding_model: None,
        }
    }
}

/// Search result with score
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub memory: MemoryEntry,
    pub score: f32,
    pub highlights: Vec<String>,
}

/// Search query
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchQuery {
    pub text: Option<String>,
    pub tags: Option<Vec<String>>,
    pub workspace_id: Option<WorkspaceId>,
    pub status: Option<MemoryStatus>,
    pub date_range: Option<DateRange>,
    pub limit: usize,
    pub offset: usize,
}

impl Default for SearchQuery {
    fn default() -> Self {
        Self {
            text: None,
            tags: None,
            workspace_id: None,
            status: None,
            date_range: None,
            limit: 10,
            offset: 0,
        }
    }
}

/// Date range for search
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DateRange {
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
}

/// Batch operation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchResult {
    pub success_count: u32,
    pub failure_count: u32,
    pub errors: Vec<String>,
}

impl BatchResult {
    pub fn new() -> Self {
        Self {
            success_count: 0,
            failure_count: 0,
            errors: Vec::new(),
        }
    }

    pub fn add_success(&mut self) {
        self.success_count += 1;
    }

    pub fn add_failure(&mut self, error: String) {
        self.failure_count += 1;
        self.errors.push(error);
    }
}

impl Default for BatchResult {
    fn default() -> Self {
        Self::new()
    }
}

/// Memory statistics
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MemoryStats {
    pub total_memories: u64,
    pub active_count: u64,
    pub cooling_count: u64,
    pub cold_count: u64,
    pub zombie_count: u64,
    pub total_size_bytes: u64,
    pub average_importance: f32,
}
#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    // ==================== MemoryEntry Tests ====================

    #[test]
    fn test_memory_entry_new() {
        let workspace_id = Uuid::new_v4();
        let content = MemoryContent::text("test content");
        let metadata = MemoryMetadata::new(MemorySource::UserQuery {
            query: "test query".to_string(),
        });

        let entry = MemoryEntry::new(workspace_id, content, metadata);

        // Verify ID is generated
        assert_ne!(entry.id, Uuid::nil());

        // Verify workspace_id matches
        assert_eq!(entry.workspace_id, workspace_id);

        // Verify default values
        assert!(entry.embedding.is_none());
        assert_eq!(entry.status, MemoryStatus::Active);
        assert_eq!(entry.access_count, 0);
        assert!(entry.last_accessed.is_none());

        // Verify timestamps are set
        assert!(entry.created_at <= Utc::now());
        assert!(entry.updated_at <= Utc::now());
    }

    #[test]
    fn test_memory_entry_increment_access() {
        let workspace_id = Uuid::new_v4();
        let content = MemoryContent::text("test content");
        let metadata = MemoryMetadata::new(MemorySource::File {
            path: PathBuf::from("/test/file.rs"),
        });

        let mut entry = MemoryEntry::new(workspace_id, content, metadata);

        assert_eq!(entry.access_count, 0);
        assert!(entry.last_accessed.is_none());

        // First access
        entry.increment_access();
        assert_eq!(entry.access_count, 1);
        assert!(entry.last_accessed.is_some());

        let first_access = entry.last_accessed;

        // Small delay to ensure different timestamps
        std::thread::sleep(std::time::Duration::from_millis(10));

        // Second access
        entry.increment_access();
        assert_eq!(entry.access_count, 2);
        assert!(entry.last_accessed.is_some());
        assert!(entry.last_accessed >= first_access);
    }

    // ==================== MemoryContent Tests ====================

    #[test]
    fn test_memory_content_text() {
        let content = MemoryContent::text("hello world");

        match content {
            MemoryContent::Text(s) => assert_eq!(s, "hello world"),
            _ => panic!("Expected Text variant"),
        }
    }

    #[test]
    fn test_memory_content_code() {
        let content = MemoryContent::code("rust", "fn main() {}");

        match content {
            MemoryContent::Code(code) => {
                assert_eq!(code.language, "rust");
                assert_eq!(code.code, "fn main() {}");
                assert!(code.ast_hash.is_none());
            }
            _ => panic!("Expected Code variant"),
        }
    }

    #[test]
    fn test_memory_content_as_text() {
        let text_content = MemoryContent::text("hello");
        assert_eq!(text_content.as_text(), Some("hello"));

        let code_content = MemoryContent::code("rust", "fn main() {}");
        assert_eq!(code_content.as_text(), None);

        let file_content = MemoryContent::File(FileContent {
            path: PathBuf::from("test.rs"),
            content: "".to_string(),
            file_type: FileType::SourceCode,
        });
        assert_eq!(file_content.as_text(), None);
    }

    #[test]
    fn test_memory_content_composite() {
        let contents = vec![
            MemoryContent::text("part 1"),
            MemoryContent::code("js", "console.log(1)"),
        ];
        let composite = MemoryContent::Composite(contents);

        match composite {
            MemoryContent::Composite(items) => assert_eq!(items.len(), 2),
            _ => panic!("Expected Composite variant"),
        }
    }

    // ==================== FileType Tests ====================

    #[test]
    fn test_file_type_from_extension_source_code() {
        let extensions = vec![
            "rs", "js", "ts", "jsx", "tsx", "py", "go", "java", "c", "cpp", "h", "hpp", "cs", "rb",
            "php", "swift", "kt", "scala", "vue", "svelte",
        ];

        for ext in extensions {
            assert_eq!(
                FileType::from_extension(ext),
                FileType::SourceCode,
                "Failed for extension: {}",
                ext
            );
        }

        // Test case insensitivity
        assert_eq!(FileType::from_extension("RS"), FileType::SourceCode);
        assert_eq!(FileType::from_extension("JS"), FileType::SourceCode);
        assert_eq!(FileType::from_extension("Py"), FileType::SourceCode);
    }

    #[test]
    fn test_file_type_from_extension_documentation() {
        let extensions = vec!["md", "txt", "rst", "adoc", "tex"];

        for ext in extensions {
            assert_eq!(
                FileType::from_extension(ext),
                FileType::Documentation,
                "Failed for extension: {}",
                ext
            );
        }
    }

    #[test]
    fn test_file_type_from_extension_configuration() {
        let extensions = vec![
            "json", "yaml", "yml", "toml", "xml", "ini", "env", "conf", "config",
        ];

        for ext in extensions {
            assert_eq!(
                FileType::from_extension(ext),
                FileType::Configuration,
                "Failed for extension: {}",
                ext
            );
        }
    }

    #[test]
    fn test_file_type_from_extension_data() {
        let extensions = vec!["csv", "tsv", "sql", "parquet", "arrow"];

        for ext in extensions {
            assert_eq!(
                FileType::from_extension(ext),
                FileType::Data,
                "Failed for extension: {}",
                ext
            );
        }
    }

    #[test]
    fn test_file_type_from_extension_other() {
        assert_eq!(FileType::from_extension("unknown"), FileType::Other);
        assert_eq!(FileType::from_extension(""), FileType::Other);
        assert_eq!(FileType::from_extension("xyz"), FileType::Other);
    }

    // ==================== MemoryMetadata Tests ====================

    #[test]
    fn test_memory_metadata_new() {
        let source = MemorySource::File {
            path: PathBuf::from("/test.rs"),
        };
        let metadata = MemoryMetadata::new(source.clone());

        assert_eq!(metadata.source, source);
        assert!(metadata.tags.is_empty());
        assert!(metadata.custom_fields.is_empty());
        assert_eq!(metadata.importance, 0.5);
    }

    #[test]
    fn test_memory_metadata_with_importance() {
        let source = MemorySource::System {
            source_type: "test".to_string(),
        };
        let metadata = MemoryMetadata::new(source).with_importance(0.9);

        assert_eq!(metadata.importance, 0.9);
    }

    #[test]
    fn test_memory_metadata_with_tags() {
        let source = MemorySource::UserQuery {
            query: "test".to_string(),
        };
        let metadata =
            MemoryMetadata::new(source).with_tags(vec!["tag1".to_string(), "tag2".to_string()]);

        assert_eq!(metadata.tags, vec!["tag1", "tag2"]);
    }

    #[test]
    fn test_memory_metadata_with_custom_field() {
        let source = MemorySource::File {
            path: PathBuf::from("/test.rs"),
        };
        let mut metadata = MemoryMetadata::new(source);
        metadata.set_custom_field("key1", "value1");

        assert_eq!(
            metadata.custom_fields.get("key1"),
            Some(&"value1".to_string())
        );
    }

    // ==================== Workspace Tests ====================

    #[test]
    fn test_workspace_new() {
        let workspace = Workspace::new("test workspace".to_string());

        assert_ne!(workspace.id, Uuid::nil());
        assert_eq!(workspace.name, "test workspace");
        assert!(workspace.description.is_none());

        // Verify default config
        assert_eq!(workspace.config.max_memory_size, 10_000_000);
        assert_eq!(workspace.config.retention_days, 90);
        assert!(workspace.config.auto_archive);
        assert!(workspace.config.embedding_model.is_none());
    }

    #[test]
    fn test_workspace_with_description() {
        let workspace =
            Workspace::new("test".to_string()).with_description("A test workspace".to_string());

        assert_eq!(workspace.description, Some("A test workspace".to_string()));
    }

    #[test]
    fn test_workspace_default_config() {
        let config = WorkspaceConfig::default();

        assert_eq!(config.max_memory_size, 10_000_000);
        assert_eq!(config.retention_days, 90);
        assert!(config.auto_archive);
        assert!(config.embedding_model.is_none());
    }

    // ==================== SearchQuery Tests ====================

    #[test]
    fn test_search_query_default() {
        let query = SearchQuery::default();

        assert!(query.text.is_none());
        assert!(query.tags.is_none());
        assert!(query.workspace_id.is_none());
        assert!(query.status.is_none());
        assert!(query.date_range.is_none());
        assert_eq!(query.limit, 10);
        assert_eq!(query.offset, 0);
    }

    // ==================== BatchResult Tests ====================

    #[test]
    fn test_batch_result_new() {
        let result = BatchResult::new();

        assert_eq!(result.success_count, 0);
        assert_eq!(result.failure_count, 0);
        assert!(result.errors.is_empty());
    }

    #[test]
    fn test_batch_result_default() {
        let result = BatchResult::default();

        assert_eq!(result.success_count, 0);
        assert_eq!(result.failure_count, 0);
    }

    #[test]
    fn test_batch_result_add_success() {
        let mut result = BatchResult::new();

        result.add_success();
        assert_eq!(result.success_count, 1);

        result.add_success();
        assert_eq!(result.success_count, 2);
    }

    #[test]
    fn test_batch_result_add_failure() {
        let mut result = BatchResult::new();

        result.add_failure("error 1".to_string());
        assert_eq!(result.failure_count, 1);
        assert_eq!(result.errors.len(), 1);
        assert_eq!(result.errors[0], "error 1");

        result.add_failure("error 2".to_string());
        assert_eq!(result.failure_count, 2);
        assert_eq!(result.errors.len(), 2);
    }

    // ==================== MemoryStats Tests ====================

    #[test]
    fn test_memory_stats_default() {
        let stats = MemoryStats::default();

        assert_eq!(stats.total_memories, 0);
        assert_eq!(stats.active_count, 0);
        assert_eq!(stats.cooling_count, 0);
        assert_eq!(stats.cold_count, 0);
        assert_eq!(stats.zombie_count, 0);
        assert_eq!(stats.total_size_bytes, 0);
        assert_eq!(stats.average_importance, 0.0);
    }

    #[test]
    fn test_memory_stats_with_values() {
        let stats = MemoryStats {
            total_memories: 100,
            active_count: 50,
            cooling_count: 20,
            cold_count: 20,
            zombie_count: 10,
            total_size_bytes: 1_000_000,
            average_importance: 0.75,
        };

        assert_eq!(stats.total_memories, 100);
        assert_eq!(stats.active_count, 50);
        assert_eq!(stats.cooling_count, 20);
        assert_eq!(stats.cold_count, 20);
        assert_eq!(stats.zombie_count, 10);
        assert_eq!(stats.total_size_bytes, 1_000_000);
        assert_eq!(stats.average_importance, 0.75);
    }

    // ==================== MemoryStatus Tests ====================

    #[test]
    fn test_memory_status_default() {
        let status = MemoryStatus::default();
        assert_eq!(status, MemoryStatus::Active);
    }

    #[test]
    fn test_memory_status_display() {
        assert_eq!(format!("{}", MemoryStatus::Active), "Active");
        assert_eq!(format!("{}", MemoryStatus::Cooling), "Cooling");
        assert_eq!(format!("{}", MemoryStatus::Cold), "Cold");
        assert_eq!(format!("{}", MemoryStatus::Zombie), "Zombie");
    }

    // ==================== MemorySource Tests ====================

    #[test]
    fn test_memory_source_variants() {
        let file_source = MemorySource::File {
            path: PathBuf::from("/test.rs"),
        };
        match file_source {
            MemorySource::File { path } => assert_eq!(path, PathBuf::from("/test.rs")),
            _ => panic!("Expected File variant"),
        }

        let query_source = MemorySource::UserQuery {
            query: "test query".to_string(),
        };
        match query_source {
            MemorySource::UserQuery { query } => assert_eq!(query, "test query"),
            _ => panic!("Expected UserQuery variant"),
        }

        let system_source = MemorySource::System {
            source_type: "auto".to_string(),
        };
        match system_source {
            MemorySource::System { source_type } => assert_eq!(source_type, "auto"),
            _ => panic!("Expected System variant"),
        }

        let training_source = MemorySource::Training {
            model_id: Uuid::nil(),
        };
        match training_source {
            MemorySource::Training { model_id } => assert_eq!(model_id, Uuid::nil()),
            _ => panic!("Expected Training variant"),
        }
    }
}
