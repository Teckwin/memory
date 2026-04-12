//! Basic Usage Example
//!
//! Run with: cargo run --example basic_usage --package memos

use memos::MemosClient;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Memos Basic Usage Example ===\n");

    // Create client with in-memory storage (async version)
    let client = MemosClient::new_in_memory_async().await?;

    // Create a workspace
    let workspace_id = client
        .create_workspace("demo-workspace".to_string())
        .await?;
    println!("Created workspace: {}", workspace_id);

    // Add a memory
    let content = "This is a comprehensive memory management system in Rust.".to_string();
    let memory_id = client
        .add_memory(&workspace_id, &content, &["rust", "memory"])
        .await?;
    println!("Added memory: {}", memory_id);

    // Search memories
    let results = client.search("memory", &workspace_id).await?;
    println!("Search results: {} memories found", results.len());

    // List all memories
    let all = client.list_memories(&workspace_id).await?;
    println!("Total memories in workspace: {}", all.len());

    println!("\n=== Example completed successfully! ===");
    Ok(())
}
