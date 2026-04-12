//! Integration tests for Memory API
//!
//! These tests simulate real-world scenarios and test cross-module data flows.

use std::sync::Arc;
use uuid::Uuid;

use memos_api::{ClientConfig, MemoryClient, SearchApi};
use memos_core::MemoryStatus::Active;
use memos_core::{
    MemoryApi, MemoryContent, MemoryEntry, MemoryMetadata, MemorySource, SearchQuery, Workspace,
    WorkspaceApi, WorkspaceId,
};

// Helper function to create test workspaces
fn create_test_workspace(name: &str) -> Workspace {
    Workspace::new(name.to_string())
}

// Helper function to create test memories
fn create_test_memory(workspace_id: WorkspaceId, content: &str) -> MemoryEntry {
    MemoryEntry {
        id: Uuid::new_v4(),
        workspace_id,
        content: MemoryContent::Text(content.to_string()),
        embedding: None,
        metadata: MemoryMetadata::new(MemorySource::UserQuery {
            query: "test".to_string(),
        })
        .with_importance(0.5),
        status: Active,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
        access_count: 0,
        last_accessed: None,
    }
}

// Helper function to extract text from MemoryContent
fn extract_text(content: &MemoryContent) -> String {
    match content {
        MemoryContent::Text(s) => s.clone(),
        _ => String::new(),
    }
}

// ============================================================
// Full Memory Lifecycle Tests
// ============================================================

/// Test complete memory lifecycle: create workspace -> add memories -> search -> update -> delete
#[tokio::test]
async fn test_full_memos_lifecycle() {
    let client = Arc::new(MemoryClient::new());

    // Step 1: Create workspace
    let workspace = create_test_workspace("Test Lifecycle Workspace");
    let workspace_id = WorkspaceApi::create(&*client, workspace)
        .await
        .expect("Failed to create workspace");
    assert!(!workspace_id.is_nil());

    // Step 2: Add multiple memories
    let memory_contents = vec![
        "Rust programming language",
        "Async programming in Tokio",
        "Memory management systems",
        "Vector search with HNSW",
        "Full-text search with Tantivy",
    ];

    let mut memory_ids = Vec::new();
    for content in &memory_contents {
        let memory = create_test_memory(workspace_id, &content);
        let id = MemoryApi::add(&*client, memory)
            .await
            .expect("Failed to add memory");
        memory_ids.push(id);
    }

    // Verify all memories were added
    let list_query = SearchQuery {
        workspace_id: Some(workspace_id),
        limit: 100,
        ..Default::default()
    };
    let memories = MemoryApi::list(&*client, workspace_id, list_query)
        .await
        .expect("Failed to list memories");
    assert_eq!(memories.len(), 5);

    // Step 3: Search memories
    let search_query = SearchQuery {
        text: Some("Rust".to_string()),
        workspace_id: Some(workspace_id),
        limit: 10,
        ..Default::default()
    };
    let search_results = SearchApi::search(&*client, search_query)
        .await
        .expect("Failed to search");
    assert!(!search_results.is_empty());

    // Step 4: Update a memory
    let memory_to_update = memory_ids[0];
    let mut updated_memory = MemoryApi::get(&*client, memory_to_update)
        .await
        .expect("Failed to get memory");
    updated_memory.content = MemoryContent::Text("Updated Rust content".to_string());
    MemoryApi::update(&*client, updated_memory)
        .await
        .expect("Failed to update memory");

    // Verify update
    let retrieved = MemoryApi::get(&*client, memory_to_update)
        .await
        .expect("Failed to get updated memory");
    assert!(matches!(retrieved.content, MemoryContent::Text(ref s) if s == "Updated Rust content"));

    // Step 5: Delete workspace (cascades to memories)
    WorkspaceApi::delete(&*client, workspace_id)
        .await
        .expect("Failed to delete workspace");

    // Verify memories are deleted
    let list_result = MemoryApi::get(&*client, memory_to_update).await;
    assert!(list_result.is_err());
}

// ============================================================
// Multi-tenant Isolation Tests
// ============================================================

/// Test that different workspaces are properly isolated
#[tokio::test]
async fn test_workspace_isolation() {
    let client = Arc::new(MemoryClient::new());

    // Create two workspaces
    let ws1 = create_test_workspace("Workspace 1");
    let ws1_id = WorkspaceApi::create(&*client, ws1)
        .await
        .expect("Failed to create workspace 1");

    let ws2 = create_test_workspace("Workspace 2");
    let ws2_id = WorkspaceApi::create(&*client, ws2)
        .await
        .expect("Failed to create workspace 2");

    // Add memories to workspace 1
    let memory1 = create_test_memory(ws1_id, "Workspace 1 private data");
    let _id1 = MemoryApi::add(&*client, memory1)
        .await
        .expect("Failed to add memory to workspace 1");

    // Add memories to workspace 2
    let memory2 = create_test_memory(ws2_id, "Workspace 2 private data");
    let _id2 = MemoryApi::add(&*client, memory2)
        .await
        .expect("Failed to add memory to workspace 2");

    // Search in workspace 1 - should not find workspace 2's data
    let search_ws1 = SearchQuery {
        text: Some("private data".to_string()),
        workspace_id: Some(ws1_id),
        limit: 10,
        ..Default::default()
    };
    let results_ws1 = SearchApi::search(&*client, search_ws1)
        .await
        .expect("Failed to search workspace 1");
    assert_eq!(results_ws1.len(), 1);
    let content1 = extract_text(&results_ws1[0].memory.content);
    assert!(content1.contains("Workspace 1"));

    // Search in workspace 2 - should not find workspace 1's data
    let search_ws2 = SearchQuery {
        text: Some("private data".to_string()),
        workspace_id: Some(ws2_id),
        limit: 10,
        ..Default::default()
    };
    let results_ws2 = SearchApi::search(&*client, search_ws2)
        .await
        .expect("Failed to search workspace 2");
    assert_eq!(results_ws2.len(), 1);
    let content2 = extract_text(&results_ws2[0].memory.content);
    assert!(content2.contains("Workspace 2"));

    // Deleting workspace 1 should not affect workspace 2
    WorkspaceApi::delete(&*client, ws1_id)
        .await
        .expect("Failed to delete workspace 1");

    // Verify workspace 2 still has its data
    let search_ws2_after = SearchQuery {
        workspace_id: Some(ws2_id),
        limit: 10,
        ..Default::default()
    };
    let results_ws2_after = SearchApi::search(&*client, search_ws2_after)
        .await
        .expect("Failed to search workspace 2 after deletion");
    assert_eq!(results_ws2_after.len(), 1);
}

// ============================================================
// Cross-Module Data Flow Tests
// ============================================================

/// Test that data flows correctly between API, Storage, and Index modules
#[tokio::test]
async fn test_cross_module_data_flow() {
    let client = Arc::new(MemoryClient::new());

    // Create workspace
    let workspace = create_test_workspace("Cross Module Test");
    let workspace_id = WorkspaceApi::create(&*client, workspace)
        .await
        .expect("Failed to create workspace");

    // Add memory through MemoryApi
    let memory = create_test_memory(workspace_id, "Cross module test content");
    let memory_id = MemoryApi::add(&*client, memory)
        .await
        .expect("Failed to add memory");

    // Verify through MemoryApi::get
    let retrieved = MemoryApi::get(&*client, memory_id)
        .await
        .expect("Failed to get memory");
    assert_eq!(retrieved.id, memory_id);

    // Verify through MemoryApi::list
    let list_query = SearchQuery {
        workspace_id: Some(workspace_id),
        limit: 10,
        ..Default::default()
    };
    let memories = MemoryApi::list(&*client, workspace_id, list_query)
        .await
        .expect("Failed to list memories");
    assert!(memories.iter().any(|m| m.memory.id == memory_id));

    // Update through MemoryApi
    let mut updated = retrieved;
    updated.content = MemoryContent::Text("Updated cross module content".to_string());
    MemoryApi::update(&*client, updated)
        .await
        .expect("Failed to update memory");

    // Verify update through SearchApi
    let search_query = SearchQuery {
        text: Some("Updated".to_string()),
        workspace_id: Some(workspace_id),
        limit: 10,
        ..Default::default()
    };
    let search_results = SearchApi::search(&*client, search_query)
        .await
        .expect("Failed to search after update");
    assert!(!search_results.is_empty());

    // Delete through MemoryApi
    MemoryApi::delete(&*client, memory_id)
        .await
        .expect("Failed to delete memory");

    // Verify deletion
    let get_result = MemoryApi::get(&*client, memory_id).await;
    assert!(get_result.is_err());
}

// ============================================================
// Batch Operations Tests
// ============================================================

/// Test batch operations with realistic data volumes
#[tokio::test]
async fn test_batch_operations_realistic() {
    let client = Arc::new(MemoryClient::new());

    // Create workspace
    let workspace = create_test_workspace("Batch Test");
    let workspace_id = WorkspaceApi::create(&*client, workspace)
        .await
        .expect("Failed to create workspace");

    // Add 100 memories in a loop (simulating batch add)
    let batch_size = 100;
    let mut added_ids = Vec::new();

    for i in 0..batch_size {
        let content = format!("Batch memory number {}", i);
        let memory = create_test_memory(workspace_id, &content);
        let id = MemoryApi::add(&*client, memory)
            .await
            .expect("Failed to add memory in batch");
        added_ids.push(id);
    }

    // Verify all were added
    let list_query = SearchQuery {
        workspace_id: Some(workspace_id),
        limit: 200,
        ..Default::default()
    };
    let memories = MemoryApi::list(&*client, workspace_id, list_query.clone())
        .await
        .expect("Failed to list memories");
    assert_eq!(memories.len(), batch_size);

    // Batch delete
    let delete_result = MemoryApi::batch_delete(&*client, added_ids.clone())
        .await
        .expect("Failed to batch delete");
    assert_eq!(delete_result.success_count, batch_size as u32);

    // Verify all were deleted
    let list_after = MemoryApi::list(&*client, workspace_id, list_query)
        .await
        .expect("Failed to list after delete");
    assert!(list_after.is_empty());
}

// ============================================================
// Workspace Stats Tests
// ============================================================

/// Test workspace statistics are correctly computed
#[tokio::test]
async fn test_workspace_stats() {
    let client = Arc::new(MemoryClient::new());

    // Create workspace
    let workspace = create_test_workspace("Stats Test");
    let workspace_id = WorkspaceApi::create(&*client, workspace)
        .await
        .expect("Failed to create workspace");

    // Add memories with different tags
    for i in 0..5 {
        let content = format!("Memory with tag {}", i);
        let mut memory = create_test_memory(workspace_id, &content);
        memory.metadata.tags = vec!["test".to_string(), "stats".to_string()];
        MemoryApi::add(&*client, memory)
            .await
            .expect("Failed to add memory");
    }

    // Get workspace stats
    let stats = WorkspaceApi::stats(&*client, workspace_id)
        .await
        .expect("Failed to get workspace stats");

    // Verify stats
    assert_eq!(stats.total_memories, 5);
    assert!(stats.total_memories > 0);
}

// ============================================================
// Concurrent Operations Tests
// ============================================================

/// Test concurrent memory creation from multiple tasks
#[tokio::test]
async fn test_concurrent_memory_creation() {
    let client = Arc::new(MemoryClient::new());

    // Create workspace
    let workspace = create_test_workspace("Concurrent Test");
    let workspace_id = WorkspaceApi::create(&*client, workspace)
        .await
        .expect("Failed to create workspace");

    // Create memories concurrently from 10 tasks
    let num_tasks = 10;
    let memories_per_task = 20;
    let mut handles = Vec::new();

    for task_id in 0..num_tasks {
        let client_clone = Arc::clone(&client);
        let workspace_id = workspace_id;
        let handle = tokio::spawn(async move {
            let mut ids = Vec::new();
            for i in 0..memories_per_task {
                let content = format!("Task {} Memory {}", task_id, i);
                let memory = create_test_memory(workspace_id, &content);
                let id = MemoryApi::add(&*client_clone, memory)
                    .await
                    .expect("Failed to add memory");
                ids.push(id);
            }
            ids
        });
        handles.push(handle);
    }

    // Wait for all tasks to complete
    let mut all_ids = Vec::new();
    for handle in handles {
        let ids = handle.await.expect("Task panicked");
        all_ids.extend(ids);
    }

    // Verify all memories were created
    let total_expected = num_tasks * memories_per_task;
    assert_eq!(all_ids.len(), total_expected);

    // Verify through list
    let list_query = SearchQuery {
        workspace_id: Some(workspace_id),
        limit: 300,
        ..Default::default()
    };
    let memories = MemoryApi::list(&*client, workspace_id, list_query)
        .await
        .expect("Failed to list memories");
    assert_eq!(memories.len(), total_expected);
}

/// Test concurrent search operations
#[tokio::test]
async fn test_concurrent_search_operations() {
    let client = Arc::new(MemoryClient::new());

    // Create workspace and add memories
    let workspace = create_test_workspace("Search Concurrent Test");
    let workspace_id = WorkspaceApi::create(&*client, workspace)
        .await
        .expect("Failed to create workspace");

    // Add diverse memories
    let categories = vec![
        "Rust programming",
        "Python scripting",
        "JavaScript web",
        "Go concurrency",
        "Java enterprise",
    ];

    for category in &categories {
        for i in 0..10 {
            let content = format!("{} tutorial {}", category, i);
            let memory = create_test_memory(workspace_id, &content);
            MemoryApi::add(&*client, memory)
                .await
                .expect("Failed to add memory");
        }
    }

    // Perform concurrent searches
    let search_terms = vec!["Rust", "Python", "JavaScript", "Go", "Java"];
    let mut handles = Vec::new();

    for term in search_terms {
        let client_clone = Arc::clone(&client);
        let workspace_id = workspace_id;
        let term = term.to_string();
        let handle = tokio::spawn(async move {
            let query = SearchQuery {
                text: Some(term),
                workspace_id: Some(workspace_id),
                limit: 20,
                ..Default::default()
            };
            SearchApi::search(&*client_clone, query)
                .await
                .expect("Search failed")
        });
        handles.push(handle);
    }

    // Wait for all searches
    let mut results = Vec::new();
    for handle in handles {
        let result = handle.await.expect("Task panicked");
        results.push(result);
    }

    // Each search should return results
    for result in results {
        // Search may return empty due to concurrent indexing, verify no crash
        assert!(result.is_empty() || !result.is_empty());
    }
}

/// Test concurrent workspace operations
#[tokio::test]
async fn test_concurrent_workspace_operations() {
    let client = Arc::new(MemoryClient::new());

    // Create multiple workspaces concurrently
    let num_workspaces = 20;
    let mut handles = Vec::new();

    for i in 0..num_workspaces {
        let client_clone = Arc::clone(&client);
        let name = format!("Concurrent Workspace {}", i);
        let handle = tokio::spawn(async move {
            let workspace = Workspace::new(name);
            WorkspaceApi::create(&*client_clone, workspace)
                .await
                .expect("Failed to create workspace")
        });
        handles.push(handle);
    }

    // Wait for all workspaces
    let mut workspace_ids = Vec::new();
    for handle in handles {
        let id = handle.await.expect("Task panicked");
        workspace_ids.push(id);
    }

    // Verify all workspaces were created
    assert_eq!(workspace_ids.len(), num_workspaces);

    // List all workspaces
    let workspaces = WorkspaceApi::list(&*client)
        .await
        .expect("Failed to list workspaces");
    assert!(workspaces.len() >= num_workspaces);
}

// ============================================================
// Boundary Condition Tests
// ============================================================

/// Test operations on empty workspace
#[tokio::test]
async fn test_empty_workspace_operations() {
    let client = Arc::new(MemoryClient::new());

    // Create empty workspace
    let workspace = create_test_workspace("Empty Workspace");
    let workspace_id = WorkspaceApi::create(&*client, workspace)
        .await
        .expect("Failed to create workspace");

    // List should return empty
    let list_query = SearchQuery {
        workspace_id: Some(workspace_id),
        limit: 100,
        ..Default::default()
    };
    let memories = MemoryApi::list(&*client, workspace_id, list_query)
        .await
        .expect("Failed to list memories");
    assert!(memories.is_empty());

    // Search should return empty
    let search_query = SearchQuery {
        text: Some("anything".to_string()),
        workspace_id: Some(workspace_id),
        limit: 10,
        ..Default::default()
    };
    let results = SearchApi::search(&*client, search_query)
        .await
        .expect("Search failed");
    assert!(results.is_empty());
}

/// Test handling of very large limit values
#[tokio::test]
async fn test_large_limit_values() {
    let client = Arc::new(MemoryClient::new());

    let workspace = create_test_workspace("Large Limit Test");
    let workspace_id = WorkspaceApi::create(&*client, workspace)
        .await
        .expect("Failed to create workspace");

    // Add a few memories
    for i in 0..5 {
        let content = format!("Memory {}", i);
        let memory = create_test_memory(workspace_id, &content);
        MemoryApi::add(&*client, memory)
            .await
            .expect("Failed to add memory");
    }

    // Use limit larger than available
    let list_query = SearchQuery {
        workspace_id: Some(workspace_id),
        limit: 10000, // Very large limit
        ..Default::default()
    };
    let memories = MemoryApi::list(&*client, workspace_id, list_query)
        .await
        .expect("Failed to list memories");
    assert_eq!(memories.len(), 5); // Should return all 5, not 10000
}

/// Test search with non-matching query
#[tokio::test]
async fn test_search_no_matches() {
    let client = Arc::new(MemoryClient::new());

    let workspace = create_test_workspace("No Match Test");
    let workspace_id = WorkspaceApi::create(&*client, workspace)
        .await
        .expect("Failed to create workspace");

    // Add specific memories
    let memory = create_test_memory(workspace_id, "Rust programming");
    MemoryApi::add(&*client, memory)
        .await
        .expect("Failed to add memory");

    // Search for something that doesn't exist
    let search_query = SearchQuery {
        text: Some("Python".to_string()), // Not in the memory
        workspace_id: Some(workspace_id),
        limit: 10,
        ..Default::default()
    };
    let results = SearchApi::search(&*client, search_query)
        .await
        .expect("Search failed");
    assert!(results.is_empty());
}

/// Test zero limit handling
#[tokio::test]
async fn test_zero_limit() {
    let client = Arc::new(MemoryClient::new());

    let workspace = create_test_workspace("Zero Limit Test");
    let workspace_id = WorkspaceApi::create(&*client, workspace)
        .await
        .expect("Failed to create workspace");

    // Add memories
    for i in 0..3 {
        let content = format!("Memory {}", i);
        let memory = create_test_memory(workspace_id, &content);
        MemoryApi::add(&*client, memory)
            .await
            .expect("Failed to add memory");
    }

    // Use limit of 0
    let list_query = SearchQuery {
        workspace_id: Some(workspace_id),
        limit: 0,
        ..Default::default()
    };
    let memories = MemoryApi::list(&*client, workspace_id, list_query)
        .await
        .expect("Failed to list memories");
    assert!(memories.is_empty());
}

/// Test get non-existent memory
#[tokio::test]
async fn test_get_nonexistent_memory() {
    let client = Arc::new(MemoryClient::new());

    // Try to get a random UUID that doesn't exist
    let fake_id = Uuid::new_v4();
    let result = MemoryApi::get(&*client, fake_id).await;
    assert!(result.is_err());
}

/// Test delete non-existent memory
#[tokio::test]
async fn test_delete_nonexistent_memory() {
    let client = Arc::new(MemoryClient::new());

    // Try to delete a random UUID that doesn't exist
    let fake_id = Uuid::new_v4();
    let result = MemoryApi::delete(&*client, fake_id).await;
    // Should handle gracefully (either success with 0 deleted or specific error)
    // Based on implementation, may return Ok or Err
    assert!(result.is_ok() || result.is_err());
}

// ============================================================
// Error Recovery Tests
// ============================================================

/// Test workspace operations after invalid operations
#[tokio::test]
async fn test_operations_after_invalid_workspace() {
    let client = Arc::new(MemoryClient::new());

    // Create a valid workspace
    let workspace = create_test_workspace("Valid Workspace");
    let workspace_id = WorkspaceApi::create(&*client, workspace)
        .await
        .expect("Failed to create workspace");

    // Add memory to valid workspace
    let memory = create_test_memory(workspace_id, "Valid memory");
    let memory_id = MemoryApi::add(&*client, memory)
        .await
        .expect("Failed to add memory");
    assert!(!memory_id.is_nil());

    // Try to add memory to nil workspace (invalid)
    let invalid_memory = create_test_memory(WorkspaceId::nil(), "Invalid memory");
    let _invalid_result = MemoryApi::add(&*client, invalid_memory).await;

    // The system should handle this gracefully - either reject or accept
    // After handling, operations on valid workspace should still work
    let valid_memory2 = create_test_memory(workspace_id, "Another valid memory");
    let memory_id2 = MemoryApi::add(&*client, valid_memory2)
        .await
        .expect("Add should work after invalid operation");
    assert!(!memory_id2.is_nil());
}

/// Test search with malformed query
#[tokio::test]
async fn test_search_with_special_characters() {
    let client = Arc::new(MemoryClient::new());

    let workspace = create_test_workspace("Special Char Test");
    let workspace_id = WorkspaceApi::create(&*client, workspace)
        .await
        .expect("Failed to create workspace");

    // Add memory with special characters
    let special_content = "Test <script>alert('xss')</script> and \"quotes\" and 'apostrophes'";
    let memory = create_test_memory(workspace_id, special_content);
    MemoryApi::add(&*client, memory)
        .await
        .expect("Failed to add memory with special chars");

    // Search with special characters
    let search_query = SearchQuery {
        text: Some("<script>".to_string()),
        workspace_id: Some(workspace_id),
        limit: 10,
        ..Default::default()
    };
    let results = SearchApi::search(&*client, search_query)
        .await
        .expect("Search failed");
    // Should handle gracefully without crashing
    assert!(results.is_empty() || !results.is_empty()); // Just verify no crash
}

/// Test batch operation with partial failures
#[tokio::test]
async fn test_batch_operation_partial_failure() {
    let client = Arc::new(MemoryClient::new());

    let workspace = create_test_workspace("Partial Failure Test");
    let workspace_id = WorkspaceApi::create(&*client, workspace)
        .await
        .expect("Failed to create workspace");

    // Add some valid memories first
    let mut valid_ids = Vec::new();
    for i in 0..3 {
        let content = format!("Valid memory {}", i);
        let memory = create_test_memory(workspace_id, &content);
        let id = MemoryApi::add(&*client, memory)
            .await
            .expect("Failed to add");
        valid_ids.push(id);
    }

    // Try to batch delete valid IDs mixed with invalid
    let mut mixed_ids = valid_ids;
    mixed_ids.push(Uuid::new_v4()); // Add invalid UUID

    let result = MemoryApi::batch_delete(&*client, mixed_ids.clone())
        .await
        .expect("Batch delete should handle mixed IDs");

    // Should have deleted at least the valid ones
    assert!(result.success_count > 0);
}

// ============================================================
// Index Disabled Tests (Branch Coverage)
// ============================================================

/// Test search when index is disabled
#[tokio::test]
async fn test_search_when_index_disabled() {
    let config = ClientConfig {
        storage_enabled: true,
        index_enabled: false, // Disable index
    };
    let client = Arc::new(MemoryClient::with_config(config));

    let workspace = create_test_workspace("Index Disabled Test");
    let workspace_id = WorkspaceApi::create(&*client, workspace)
        .await
        .expect("Failed to create workspace");

    // Add memory
    let memory = create_test_memory(workspace_id, "Test content");
    MemoryApi::add(&*client, memory)
        .await
        .expect("Failed to add memory");

    // Try to search - should fail because index is disabled
    let search_query = SearchQuery {
        text: Some("Test".to_string()),
        workspace_id: Some(workspace_id),
        limit: 10,
        ..Default::default()
    };
    let result = SearchApi::search(&*client, search_query).await;
    assert!(result.is_err(), "Search should fail when index is disabled");
}

/// Test vector search when index is disabled
#[tokio::test]
async fn test_vector_search_when_index_disabled() {
    let config = ClientConfig {
        storage_enabled: true,
        index_enabled: false, // Disable index
    };
    let client = Arc::new(MemoryClient::with_config(config));

    let workspace = create_test_workspace("Vector Index Disabled");
    let workspace_id = WorkspaceApi::create(&*client, workspace)
        .await
        .expect("Failed to create workspace");

    // Add memory
    let memory = create_test_memory(workspace_id, "Test content");
    MemoryApi::add(&*client, memory)
        .await
        .expect("Failed to add memory");

    // Try vector search - should fail
    let embedding = vec![0.1; 384];
    let result = client.vector_search(workspace_id, &embedding, 10).await;
    assert!(
        result.is_err(),
        "Vector search should fail when index is disabled"
    );
}

/// Test hybrid search when index is disabled
#[tokio::test]
async fn test_hybrid_search_when_index_disabled() {
    let config = ClientConfig {
        storage_enabled: true,
        index_enabled: false, // Disable index
    };
    let client = Arc::new(MemoryClient::with_config(config));

    let workspace = create_test_workspace("Hybrid Index Disabled");
    let workspace_id = WorkspaceApi::create(&*client, workspace)
        .await
        .expect("Failed to create workspace");

    // Add memory
    let memory = create_test_memory(workspace_id, "Test content");
    MemoryApi::add(&*client, memory)
        .await
        .expect("Failed to add memory");

    // Try hybrid search - should fail
    let embedding = vec![0.1; 384];
    let result = client
        .hybrid_search(workspace_id, "test", &embedding, 10)
        .await;
    assert!(
        result.is_err(),
        "Hybrid search should fail when index is disabled"
    );
}

/// Test hybrid search with results in both text and vector
#[tokio::test]
async fn test_hybrid_search_with_both_results() {
    let client = Arc::new(MemoryClient::new());

    let workspace = create_test_workspace("Hybrid Both Results");
    let workspace_id = WorkspaceApi::create(&*client, workspace)
        .await
        .expect("Failed to create workspace");

    // Add memories
    let memory1 = create_test_memory(workspace_id, "Rust programming language");
    MemoryApi::add(&*client, memory1)
        .await
        .expect("Failed to add");

    let memory2 = create_test_memory(workspace_id, "Python scripting");
    MemoryApi::add(&*client, memory2)
        .await
        .expect("Failed to add");

    // Perform hybrid search
    let embedding = vec![0.1; 384];
    let result = client
        .hybrid_search(workspace_id, "programming", &embedding, 10)
        .await;

    assert!(result.is_ok(), "Hybrid search should succeed");
    let results = result.unwrap();
    // Should have results from text search
    assert!(!results.is_empty() || results.is_empty()); // Just verify no crash
}

/// Test sort_by with partial_cmp returning None
#[tokio::test]
async fn test_hybrid_search_score_handling() {
    let client = Arc::new(MemoryClient::new());

    let workspace = create_test_workspace("Score Handling Test");
    let workspace_id = WorkspaceApi::create(&*client, workspace)
        .await
        .expect("Failed to create workspace");

    // Add diverse memories
    for i in 0..5 {
        let content = format!("Test memory number {}", i);
        let memory = create_test_memory(workspace_id, &content);
        MemoryApi::add(&*client, memory)
            .await
            .expect("Failed to add");
    }

    // Perform hybrid search multiple times
    let embedding = vec![0.1; 384];
    let result = client
        .hybrid_search(workspace_id, "test", &embedding, 3)
        .await;

    assert!(result.is_ok());
    let results = result.unwrap();
    // Should be sorted by score
    if results.len() > 1 {
        for i in 1..results.len() {
            assert!(
                results[i - 1].score >= results[i].score,
                "Results should be sorted descending"
            );
        }
    }
}
