//! Cold storage - SQLite backend for Cold/Zombie memories

use async_trait::async_trait;
use std::path::PathBuf;
use tokio::task;

use crate::error::StorageError;
use memory_core::{
    BatchResult, MemoryApi, MemoryContent, MemoryEntry, MemoryError, MemoryId, MemoryMetadata,
    MemoryStatus, SearchQuery, SearchResult, WorkspaceId,
};
/// Type alias for memory-to-row conversion result
type MemoryRow = (
    String,
    String,
    String,
    Option<Vec<u8>>,
    String,
    String,
    String,
    String,
    u64,
    Option<String>,
);

/// Cold storage implementation using SQLite
pub struct ColdStorage {
    /// Path to the SQLite database
    db_path: PathBuf,
}

impl ColdStorage {
    pub fn new(db_path: PathBuf) -> Self {
        Self { db_path }
    }

    /// Initialize the database schema
    pub async fn initialize(&self) -> Result<(), StorageError> {
        let path = self.db_path.clone();

        task::spawn_blocking(move || {
            let conn = rusqlite::Connection::open(&path)?;
            Self::create_tables(&conn)?;
            Ok(())
        })
        .await
        .map_err(|e| StorageError::ColdError(format!("Task join error: {}", e)))?
    }

    fn create_tables(conn: &rusqlite::Connection) -> Result<(), StorageError> {
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS memories (
                id TEXT PRIMARY KEY,
                workspace_id TEXT NOT NULL,
                content TEXT NOT NULL,
                embedding BLOB,
                metadata TEXT NOT NULL,
                status TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                access_count INTEGER NOT NULL DEFAULT 0,
                last_accessed TEXT
            );

            CREATE INDEX IF NOT EXISTS idx_memories_workspace
                ON memories(workspace_id);
            CREATE INDEX IF NOT EXISTS idx_memories_status
                ON memories(status);
            CREATE INDEX IF NOT EXISTS idx_memories_created
                ON memories(created_at);
            CREATE INDEX IF NOT EXISTS idx_memories_updated
                ON memories(updated_at);
            "#,
        )?;
        Ok(())
    }

    /// Convert MemoryEntry to row data
    fn memory_to_row(memory: &MemoryEntry) -> Result<MemoryRow, StorageError> {
        let content_json = serde_json::to_string(&memory.content)?;
        let metadata_json = serde_json::to_string(&memory.metadata)?;
        let embedding_bytes = memory
            .embedding
            .as_ref()
            .and_then(|e| serde_json::to_vec(e).ok());

        let status_str = serde_json::to_string(&memory.status)?;

        Ok((
            memory.id.to_string(),
            memory.workspace_id.to_string(),
            content_json,
            embedding_bytes,
            metadata_json,
            status_str,
            memory.created_at.to_rfc3339(),
            memory.updated_at.to_rfc3339(),
            memory.access_count,
            memory.last_accessed.map(|dt| dt.to_rfc3339()),
        ))
    }

    /// Parse row data to MemoryEntry
    #[allow(clippy::too_many_arguments)]
    fn parse_row(
        id_str: String,
        workspace_id_str: String,
        content_json: String,
        embedding_bytes: Option<Vec<u8>>,
        metadata_json: String,
        status_str: String,
        created_at_str: String,
        updated_at_str: String,
        access_count: u64,
        last_accessed_str: Option<String>,
    ) -> Result<MemoryEntry, StorageError> {
        let content: MemoryContent = serde_json::from_str(&content_json)?;
        let metadata: MemoryMetadata = serde_json::from_str(&metadata_json)?;
        // Parse status - strip quotes if present (stored as JSON string)
        let status_str = status_str.trim_matches('"');
        let status: MemoryStatus = match status_str {
            "Active" => MemoryStatus::Active,
            "Cooling" => MemoryStatus::Cooling,
            "Cold" => MemoryStatus::Cold,
            "Zombie" => MemoryStatus::Zombie,
            _ => {
                return Err(StorageError::SerializationError(format!(
                    "Invalid status: {}",
                    status_str
                )))
            }
        };

        let embedding: Option<Vec<f32>> =
            embedding_bytes.and_then(|bytes| serde_json::from_slice(&bytes).ok());

        let created_at = chrono::DateTime::parse_from_rfc3339(&created_at_str)
            .map(|dt| dt.with_timezone(&chrono::Utc))
            .map_err(|e| StorageError::SerializationError(e.to_string()))?;

        let updated_at = chrono::DateTime::parse_from_rfc3339(&updated_at_str)
            .map(|dt| dt.with_timezone(&chrono::Utc))
            .map_err(|e| StorageError::SerializationError(e.to_string()))?;

        let last_accessed = last_accessed_str
            .and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok())
            .map(|dt| dt.with_timezone(&chrono::Utc));

        Ok(MemoryEntry {
            id: uuid::Uuid::parse_str(&id_str)
                .map_err(|e| StorageError::SerializationError(e.to_string()))?,
            workspace_id: uuid::Uuid::parse_str(&workspace_id_str)
                .map_err(|e| StorageError::SerializationError(e.to_string()))?,
            content,
            embedding,
            metadata,
            status,
            created_at,
            updated_at,
            access_count,
            last_accessed,
        })
    }
}

#[async_trait]
impl MemoryApi for ColdStorage {
    async fn add(&self, memory: MemoryEntry) -> Result<MemoryId, MemoryError> {
        let path = self.db_path.clone();
        let row = ColdStorage::memory_to_row(&memory)?;

        task::spawn_blocking(move || {
            let conn = rusqlite::Connection::open(&path)?;
            conn.execute(
                r#"
                INSERT INTO memories (id, workspace_id, content, embedding, metadata, status, created_at, updated_at, access_count, last_accessed)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
                ON CONFLICT(id) DO UPDATE SET
                    content = excluded.content,
                    embedding = excluded.embedding,
                    metadata = excluded.metadata,
                    status = excluded.status,
                    updated_at = excluded.updated_at,
                    access_count = excluded.access_count,
                    last_accessed = excluded.last_accessed
                "#,
                rusqlite::params![
                    row.0, row.1, row.2, row.3, row.4, row.5, row.6, row.7, row.8, row.9
                ],
            )?;
            Ok(())
        })
        .await
        .map_err(|e| MemoryError::StorageError(format!("Task join error: {}", e)))?
        .map_err(|e: StorageError| MemoryError::StorageError(e.to_string()))?;

        Ok(memory.id)
    }

    async fn get(&self, id: MemoryId) -> Result<MemoryEntry, MemoryError> {
        let path = self.db_path.clone();
        let id_str = id.to_string();

        task::spawn_blocking(move || {
            let conn = rusqlite::Connection::open(&path)?;

            let row_data = conn.query_row(
                "SELECT id, workspace_id, content, embedding, metadata, status, created_at, updated_at, access_count, last_accessed FROM memories WHERE id = ?1",
                [&id_str],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, Option<Vec<u8>>>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, String>(5)?,
                        row.get::<_, String>(6)?,
                        row.get::<_, String>(7)?,
                        row.get::<_, u64>(8)?,
                        row.get::<_, Option<String>>(9)?,
                    ))
                },
            )?;

            let result = ColdStorage::parse_row(
                row_data.0,
                row_data.1,
                row_data.2,
                row_data.3,
                row_data.4,
                row_data.5,
                row_data.6,
                row_data.7,
                row_data.8,
                row_data.9,
            )?;

            Ok(result)
        })
        .await
        .map_err(|e| MemoryError::StorageError(format!("Task join error: {}", e)))?
        .map_err(|e: StorageError| MemoryError::StorageError(e.to_string()))
    }

    async fn update(&self, memory: MemoryEntry) -> Result<(), MemoryError> {
        // Reuse add which has ON CONFLICT UPDATE
        self.add(memory).await?;
        Ok(())
    }

    async fn delete(&self, id: MemoryId) -> Result<(), MemoryError> {
        let path = self.db_path.clone();
        let id_str = id.to_string();

        task::spawn_blocking(move || {
            let conn = rusqlite::Connection::open(&path)?;
            let affected = conn.execute("DELETE FROM memories WHERE id = ?1", [&id_str])?;

            if affected == 0 {
                return Err(StorageError::ColdError(format!("Memory {} not found", id)));
            }
            Ok(())
        })
        .await
        .map_err(|e| MemoryError::StorageError(format!("Task join error: {}", e)))?
        .map_err(|e: StorageError| MemoryError::StorageError(e.to_string()))
    }

    async fn list(
        &self,
        workspace_id: WorkspaceId,
        query: SearchQuery,
    ) -> Result<Vec<SearchResult>, MemoryError> {
        let path = self.db_path.clone();
        let workspace_id_str = workspace_id.to_string();

        task::spawn_blocking(move || {
            let conn = rusqlite::Connection::open(&path)?;

            // If workspace_id is nil, don't filter by workspace (return all)
            let mut sql = if workspace_id.is_nil() {
                String::from("SELECT id, workspace_id, content, embedding, metadata, status, created_at, updated_at, access_count, last_accessed FROM memories WHERE 1=1")
            } else {
                String::from("SELECT id, workspace_id, content, embedding, metadata, status, created_at, updated_at, access_count, last_accessed FROM memories WHERE workspace_id = ?1")
            };
            let mut params: Vec<Box<dyn rusqlite::ToSql>> = if !workspace_id.is_nil() {
                vec![Box::new(workspace_id_str)]
            } else {
                vec![]
            };

            if let Some(status) = &query.status {
                sql.push_str(" AND status = ?");
                let status_str = serde_json::to_string(status)?;  // Use JSON serialization to match stored format
                params.push(Box::new(status_str));
            }

            sql.push_str(" ORDER BY created_at DESC");

            if query.limit > 0 {
                sql.push_str(&format!(" LIMIT {}", query.limit));
            }

            if query.offset > 0 {
                sql.push_str(&format!(" OFFSET {}", query.offset));
            }

            let mut stmt = conn.prepare(&sql)?;

            let params_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();

            let row_datas: Vec<(String, String, String, Option<Vec<u8>>, String, String, String, String, u64, Option<String>)> = stmt.query_map(params_refs.as_slice(), |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Option<Vec<u8>>>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, String>(7)?,
                    row.get::<_, u64>(8)?,
                    row.get::<_, Option<String>>(9)?,
                ))
            })?
            .filter_map(|r| r.ok())
            .collect();

            let results = row_datas
                .into_iter()
                .map(|row_data| ColdStorage::parse_row(
                    row_data.0,
                    row_data.1,
                    row_data.2,
                    row_data.3,
                    row_data.4,
                    row_data.5,
                    row_data.6,
                    row_data.7,
                    row_data.8,
                    row_data.9,
                ))
                .filter_map(|r| r.ok())
                // Apply tags filter in-memory if specified
                .filter(|memory| {
                    if let Some(ref tags) = query.tags {
                        if tags.is_empty() {
                            return true;
                        }
                        tags.iter().any(|tag| memory.metadata.tags.contains(tag))
                    } else {
                        true
                    }
                })
                // Apply text search filter in-memory if specified
                .filter(|memory| {
                    if let Some(ref text) = query.text {
                        let content_text = memory.content.as_text().unwrap_or("");
                        content_text.to_lowercase().contains(&text.to_lowercase())
                    } else {
                        true
                    }
                })
                .map(|memory| SearchResult {
                    memory,
                    score: 1.0,
                    highlights: vec![],
                })
                .collect();

            Ok(results)
        })
        .await
        .map_err(|e| MemoryError::StorageError(format!("Task join error: {}", e)))?
        .map_err(|e: StorageError| MemoryError::StorageError(e.to_string()))
    }

    async fn batch_add(&self, memories: Vec<MemoryEntry>) -> Result<BatchResult, MemoryError> {
        let path = self.db_path.clone();

        // Process in spawn_blocking to avoid many context switches
        task::spawn_blocking(move || {
            let conn = rusqlite::Connection::open(&path)?;
            let mut result = BatchResult::new();

            // Use transaction for batch efficiency
            let tx = conn.unchecked_transaction()?;

            for memory in memories {
                match ColdStorage::memory_to_row(&memory) {
                    Ok(row) => {
                        let res = tx.execute(
                            r#"
                            INSERT INTO memories (id, workspace_id, content, embedding, metadata, status, created_at, updated_at, access_count, last_accessed)
                            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
                            ON CONFLICT(id) DO UPDATE SET
                                content = excluded.content,
                                embedding = excluded.embedding,
                                metadata = excluded.metadata,
                                status = excluded.status,
                                updated_at = excluded.updated_at,
                                access_count = excluded.access_count,
                                last_accessed = excluded.last_accessed
                            "#,
                            rusqlite::params![
                                row.0, row.1, row.2, row.3, row.4, row.5, row.6, row.7, row.8, row.9
                            ],
                        );

                        match res {
                            Ok(_) => result.add_success(),
                            Err(e) => result.add_failure(format!("{}: {}", memory.id, e)),
                        }
                    }
                    Err(e) => result.add_failure(format!("{}: {}", memory.id, e)),
                }
            }

            tx.commit()?;
            Ok(result)
        })
        .await
        .map_err(|e| MemoryError::StorageError(format!("Task join error: {}", e)))?
        .map_err(|e: StorageError| MemoryError::StorageError(e.to_string()))
    }

    async fn batch_delete(&self, ids: Vec<MemoryId>) -> Result<BatchResult, MemoryError> {
        let path = self.db_path.clone();

        task::spawn_blocking(move || {
            let conn = rusqlite::Connection::open(&path)?;
            let mut result = BatchResult::new();

            let tx = conn.unchecked_transaction()?;

            for id in ids {
                let id_str = id.to_string();
                match tx.execute("DELETE FROM memories WHERE id = ?1", [&id_str]) {
                    Ok(affected) => {
                        if affected > 0 {
                            result.add_success();
                        } else {
                            result.add_failure(format!("Memory {} not found", id));
                        }
                    }
                    Err(e) => result.add_failure(format!("{}: {}", id, e)),
                }
            }

            tx.commit()?;
            Ok(result)
        })
        .await
        .map_err(|e| MemoryError::StorageError(format!("Task join error: {}", e)))?
        .map_err(|e: StorageError| MemoryError::StorageError(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use memory_core::{MemoryContent, MemoryMetadata};

    use tempfile::tempdir;

    fn create_test_memory(status: MemoryStatus) -> MemoryEntry {
        MemoryEntry {
            id: uuid::Uuid::new_v4(),
            workspace_id: uuid::Uuid::new_v4(),
            content: MemoryContent::Text("test content".to_string()),
            embedding: None,
            metadata: MemoryMetadata::new(memory_core::MemorySource::UserQuery {
                query: "test".to_string(),
            })
            .with_tags(vec!["test".to_string()]),
            status,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            access_count: 0,
            last_accessed: None,
        }
    }

    #[tokio::test]
    async fn test_cold_storage_initialize() {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let storage = ColdStorage::new(db_path);

        let result = storage.initialize().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_add_and_get_memory() {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let storage = ColdStorage::new(db_path);
        storage.initialize().await.unwrap();

        let memory = create_test_memory(MemoryStatus::Cold);
        let id = memory.id;

        let add_result = storage.add(memory).await;
        assert!(add_result.is_ok());

        let get_result = storage.get(id).await;
        assert!(get_result.is_ok());
        assert_eq!(get_result.unwrap().id, id);
    }

    #[tokio::test]
    async fn test_update_memory() {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let storage = ColdStorage::new(db_path);
        storage.initialize().await.unwrap();

        let memory = create_test_memory(MemoryStatus::Cold);
        let id = memory.id;

        storage.add(memory).await.unwrap();

        let mut updated_memory = create_test_memory(MemoryStatus::Cold);
        updated_memory.id = id;
        updated_memory.content = MemoryContent::Text("updated content".to_string());

        let update_result = storage.update(updated_memory).await;
        assert!(update_result.is_ok());

        let get_result = storage.get(id).await.unwrap();
        assert_eq!(get_result.content.as_text(), Some("updated content"));
    }

    #[tokio::test]
    async fn test_delete_memory() {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let storage = ColdStorage::new(db_path);
        storage.initialize().await.unwrap();

        let memory = create_test_memory(MemoryStatus::Cold);
        let id = memory.id;

        storage.add(memory).await.unwrap();
        let delete_result = storage.delete(id).await;
        assert!(delete_result.is_ok());

        let get_result = storage.get(id).await;
        assert!(get_result.is_err());
    }

    #[tokio::test]
    async fn test_list_memories() {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let storage = ColdStorage::new(db_path);
        storage.initialize().await.unwrap();

        let workspace_id = uuid::Uuid::new_v4();

        for i in 0..5 {
            let mut memory = create_test_memory(MemoryStatus::Cold);
            memory.workspace_id = workspace_id;
            memory.content = MemoryContent::Text(format!("test content {}", i));
            storage.add(memory).await.unwrap();
        }

        let query = SearchQuery {
            workspace_id: Some(workspace_id),
            limit: 10,
            ..Default::default()
        };

        let results = storage.list(workspace_id, query).await.unwrap();
        assert_eq!(results.len(), 5);
    }

    #[tokio::test]
    async fn test_batch_operations() {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let storage = ColdStorage::new(db_path);
        storage.initialize().await.unwrap();

        let memories: Vec<MemoryEntry> = (0..10)
            .map(|_| create_test_memory(MemoryStatus::Cold))
            .collect();

        let add_result = storage.batch_add(memories).await.unwrap();
        assert_eq!(add_result.success_count, 10);

        let ids: Vec<MemoryId> = (0..5).map(|_| uuid::Uuid::new_v4()).collect();

        let delete_result = storage.batch_delete(ids).await.unwrap();
        assert_eq!(delete_result.success_count, 0); // None exist
    }

    #[tokio::test]
    async fn test_add_cooling_memory() {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let storage = ColdStorage::new(db_path);
        storage.initialize().await.unwrap();

        // Use Cooling status instead of Cold
        let memory = create_test_memory(MemoryStatus::Cooling);
        let id = memory.id;

        let add_result = storage.add(memory).await;
        assert!(add_result.is_ok());

        // Verify by getting it back using get() method
        let get_result = storage.get(id).await;
        assert!(get_result.is_ok());
        assert_eq!(get_result.unwrap().status, MemoryStatus::Cooling);
    }

    /// 白盒测试：验证所有 MemoryStatus 的序列化/反序列化一致性
    #[tokio::test]
    async fn test_status_serialization_roundtrip() {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let storage = ColdStorage::new(db_path);
        storage.initialize().await.unwrap();

        // 测试所有状态
        let statuses = vec![
            MemoryStatus::Active,
            MemoryStatus::Cooling,
            MemoryStatus::Cold,
            MemoryStatus::Zombie,
        ];

        for status in statuses {
            let mut memory = create_test_memory(status);
            let original_id = memory.id;
            let original_content = memory.content.clone();
            let original_metadata = memory.metadata.clone();
            let original_workspace_id = memory.workspace_id;
            let original_created_at = memory.created_at;
            let original_updated_at = memory.updated_at;
            let original_access_count = memory.access_count;

            // 存储
            storage.add(memory).await.unwrap();

            // 读取
            let retrieved = storage.get(original_id).await.unwrap();

            // 验证数据完全对等（白盒验证内部实现）
            assert_eq!(retrieved.id, original_id, "ID should match");
            assert_eq!(
                retrieved.workspace_id, original_workspace_id,
                "WorkspaceId should match"
            );
            assert_eq!(
                retrieved.status, status,
                "Status should match for {:?}",
                status
            );
            assert_eq!(
                retrieved.access_count, original_access_count,
                "Access count should match"
            );
            assert_eq!(
                retrieved.created_at, original_created_at,
                "Created at should match"
            );
            assert_eq!(
                retrieved.updated_at, original_updated_at,
                "Updated at should match"
            );

            // 验证 content 对等
            match (&original_content, &retrieved.content) {
                (MemoryContent::Text(original), MemoryContent::Text(retrieved)) => {
                    assert_eq!(original, retrieved, "Content should match");
                }
                _ => panic!("Content type mismatch"),
            }

            // 验证 metadata 对等
            assert_eq!(
                retrieved.metadata.tags, original_metadata.tags,
                "Tags should match"
            );
        }
    }

    /// 黑盒测试：通过 list API 验证状态过滤正确性
    #[tokio::test]
    async fn test_list_with_all_status_filters() {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let storage = ColdStorage::new(db_path);
        storage.initialize().await.unwrap();

        let workspace_id = uuid::Uuid::new_v4();

        // 添加不同状态的记忆
        for status in &[MemoryStatus::Cooling, MemoryStatus::Cold] {
            let mut memory = create_test_memory(*status);
            memory.workspace_id = workspace_id;
            storage.add(memory).await.unwrap();
        }

        // 黑盒验证：通过 list 接口查询不同状态
        for status in &[MemoryStatus::Cooling, MemoryStatus::Cold] {
            let query = SearchQuery {
                workspace_id: Some(workspace_id),
                status: Some(*status),
                limit: 10,
                ..Default::default()
            };

            let results = storage.list(workspace_id, query).await.unwrap();
            assert_eq!(
                results.len(),
                1,
                "Should find exactly 1 {:?} memory",
                status
            );

            // 验证返回的状态确实是我们查询的
            for result in &results {
                assert_eq!(
                    result.memory.status, *status,
                    "Returned status should match query"
                );
            }
        }
    }

    /// 测试空内容边界情况
    #[tokio::test]
    async fn test_empty_content_storage() {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let storage = ColdStorage::new(db_path);
        storage.initialize().await.unwrap();

        let mut memory = create_test_memory(MemoryStatus::Cold);
        memory.content = MemoryContent::Text(String::new());

        let id = memory.id;
        storage.add(memory).await.unwrap();

        let retrieved = storage.get(id).await.unwrap();
        assert_eq!(retrieved.content.as_text(), Some(""));
    }

    /// 测试特殊字符内容
    #[tokio::test]
    async fn test_special_characters_storage() {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let storage = ColdStorage::new(db_path);
        storage.initialize().await.unwrap();

        let test_cases = vec![
            "Hello World",
            "中文测试",
            "emoji 😄",
            "special <>&\"' chars",
            "newlines\n\nand\ttabs",
            "unicode © ® ™",
        ];

        for content in test_cases {
            let mut memory = create_test_memory(MemoryStatus::Cold);
            memory.content = MemoryContent::Text(content.to_string());

            let id = memory.id;
            storage.add(memory).await.unwrap();

            let retrieved = storage.get(id).await.unwrap();
            assert_eq!(
                retrieved.content.as_text(),
                Some(content),
                "Content should match for: {}",
                content
            );
        }
    }

    #[tokio::test]
    async fn test_list_cooling_memories() {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let storage = ColdStorage::new(db_path);
        storage.initialize().await.unwrap();

        let workspace_id = uuid::Uuid::new_v4();

        // Add Cooling memories
        for i in 0..3 {
            let mut memory = create_test_memory(MemoryStatus::Cooling);
            memory.workspace_id = workspace_id;
            memory.content = MemoryContent::Text(format!("cooling content {}", i));
            storage.add(memory).await.unwrap();
        }

        // Query for Cooling status
        let query = SearchQuery {
            workspace_id: Some(workspace_id),
            status: Some(MemoryStatus::Cooling),
            limit: 10,
            ..Default::default()
        };

        let results = storage.list(workspace_id, query).await.unwrap();
        assert_eq!(results.len(), 3);
    }
}
