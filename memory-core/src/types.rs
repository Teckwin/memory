//! Core memory types

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
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
    pub fn new(workspace_id: WorkspaceId, content: MemoryContent, metadata: MemoryMetadata) -> Self {
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
            "rs" | "js" | "ts" | "jsx" | "tsx" | "py" | "go" | "java" | "c" | "cpp" | "h" | "hpp"
            | "cs" | "rb" | "php" | "swift" | "kt" | "scala" | "vue" | "svelte" => {
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
}

/// Memory source origin
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum MemorySource {
    File { path: PathBuf },
    UserQuery { query: String },
    System { source_type: String },
    Training { model_id: Uuid },
}

/// Memory lifecycle status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MemoryStatus {
    Active,
    Cooling,
    Cold,
    Zombie,
}

impl Default for MemoryStatus {
    fn default() -> Self {
        MemoryStatus::Active
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