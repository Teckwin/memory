//! Search Example - Demonstrates search capabilities
//!
//! Run with: cargo run --example search_example --package memos

use memos::MemosClient;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Memos Search Example ===\n");

    // Create client with in-memory storage (async version)
    let client = MemosClient::new_in_memory_async().await?;

    // Create a workspace
    let workspace_id = client.create_workspace("search-demo".to_string()).await?;
    println!("Created workspace: {}", workspace_id);

    // Add some memories for testing
    let memories = vec![
        (
            "Rust is a systems programming language",
            vec!["rust", "programming"],
        ),
        ("Python is great for data science", vec!["python", "data"]),
        ("JavaScript runs in the browser", vec!["javascript", "web"]),
        ("Rust has great memory safety", vec!["rust", "memory"]),
    ];

    for (content, tags) in &memories {
        let _id = client.add_memory(&workspace_id, content, tags).await?;
        let short = content.chars().take(30).collect::<String>();
        println!("Added memory: {}", short);
    }

    // Search memories
    let results = client.search("Rust", &workspace_id).await?;
    println!("\nSearch for 'Rust': {} results", results.len());

    // List all memories
    let all = client.list_memories(&workspace_id).await?;
    println!("Total memories: {}", all.len());

    println!("\n=== Example completed successfully! ===");
    Ok(())
}
