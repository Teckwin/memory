//! Lifecycle Management Example
//!
//! Run with: cargo run --example lifecycle_example --package memos

use memos::MemosClient;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Memos Lifecycle Example ===\n");

    // Create client with in-memory storage (async version)
    let client = MemosClient::new_in_memory_async().await?;

    // Create a workspace
    let workspace_id = client
        .create_workspace("lifecycle-demo".to_string())
        .await?;
    println!("Created workspace: {}", workspace_id);

    // Add some memories
    let memories = vec![
        ("Active memory 1", vec!["tag1"]),
        ("Active memory 2", vec!["tag2"]),
        ("Old memory to archive", vec!["archive"]),
    ];

    for (content, tags) in &memories {
        client.add_memory(&workspace_id, content, tags).await?;
    }

    // List current memories
    let all = client.list_memories(&workspace_id).await?;
    println!("Total memories: {}", all.len());

    // Get workspace statistics
    let stats = client.get_workspace_stats(&workspace_id).await?;
    println!("Workspace stats: {} active memories", stats.active_count);

    println!("\n=== Example completed successfully! ===");
    Ok(())
}
