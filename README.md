# Memory Management System

A local memory management system for AI applications, providing efficient memory storage, retrieval, and automatic learning capabilities.

[中文版](./README-ZH.md)

## Features

- **Tiered Storage**: Hot (in-memory), Cold (SQLite), and Zombie (archive) storage layers
- **Multi-index Search**: Vector (HNSW), Full-text (Tantivy), and Hybrid search
- **Lifecycle Management**: Automatic memory status transitions (Active → Cooling → Cold → Zombie)
- **Training Module**: Data preparation and model management for continuous learning
- **Workspace Support**: Multi-workspace isolation with independent configurations

## Project Structure

```
memory/
├── Cargo.toml              # Workspace root
├── crates/
│   ├── memory-core/        # Core types and traits
│   ├── memory-api/         # Client API
│   ├── memory-storage/     # Storage layer (hot/cold/zombie)
│   ├── memory-index/       # Index layer (vector/fulltext/hybrid)
│   ├── memory-lifecycle/   # Lifecycle management
│   └── memory-train/       # Training module
├── docs/                   # Documentation
│   ├── README.md          # (EN) System documentation
│   ├── README-ZH.md       # (ZH) 系统文档
│   └── data-upload-protocol*.md
└── plans/                  # Implementation plans
```

## Quick Start

```rust
use memory_api::MemoryClient;
use memory_core::{MemoryEntry, MemoryContent, MemoryMetadata, MemorySource, WorkspaceId, Workspace};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create client
    let client = MemoryClient::new(Default::default());

    // Create workspace
    let workspace_id = client
        .create(Workspace::new("my-workspace".to_string()))
        .await?;

    // Add memory
    let memory = MemoryEntry::new(
        workspace_id,
        MemoryContent::Text("Hello, world!".to_string()),
        MemoryMetadata::new(MemorySource::UserQuery {
            query: "greeting".to_string(),
        }),
    );
    let memory_id = client.add(memory).await?;

    // Retrieve memory
    let retrieved = client.get(memory_id).await?;
    println!("Retrieved: {:?}", retrieved.content);

    Ok(())
}
```

## Modules

### memory-core
Core types and trait definitions:
- `MemoryEntry`: Individual memory unit
- `Workspace`: Memory container
- `SearchQuery` / `SearchResult`: Search operations
- `MemoryApi`, `SearchApi`, `WorkspaceApi` traits

### memory-api
High-level client API implementing all core traits.

### memory-storage
Tiered storage implementation:
- **Hot**: In-memory cache for Active memories
- **Cold**: SQLite for Cooling/Cold memories
- **Zombie**: File-based archive for archived memories

### memory-index
Search indexing:
- **VectorIndex**: HNSW-based approximate nearest neighbor search
- **FulltextIndex**: Tantivy-based full-text search
- **HybridIndex**: Combined vector + full-text search

### memory-lifecycle
Automatic lifecycle management:
- Configurable transition policies
- Scheduler for periodic transitions
- Archive management

### memory-train
Training capabilities:
- Data preparation from memories
- Model management (save/load)
- Embedding generation

## Configuration

### Storage Configuration
```rust
let config = ClientConfig {
    storage: Some(storage::Config {
        hot: hot::Config { max_entries: 10000 },
        cold: cold::Config { db_path: "memory.db".into() },
        zombie: zombie::Config { archive_dir: "archives".into() },
    }),
    ..Default::default()
};
```

### Lifecycle Configuration
```rust
let policy_config = PolicyConfig {
    active_to_cooling_days: 7,
    cooling_to_cold_days: 14,
    cold_to_zombie_days: 30,
    ..Default::default()
};
```

## Testing

```bash
# Run all tests
cargo test --workspace

# Run tests for specific crate
cargo test -p memory-core
cargo test -p memory-api
```

## Test Coverage

| Crate | Tests |
|-------|-------|
| memory-core | 36 |
| memory-api | 65 |
| memory-storage | 27 |
| memory-index | 36 |
| memory-lifecycle | 27 |
| memory-train | 20 |
| **Total** | **211** |

## License

MIT