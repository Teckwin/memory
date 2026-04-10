# Memory System Documentation

This directory contains documentation for the Memory Management System.

## Contents

- [Data Upload Protocol](./data-upload-protocol.md) - Protocol for uploading data to the memory system
- Implementation Plans
  - [Architecture Design](./2026-04-09-local-memory-management-system-v1.md)
  - [Training Analysis](./2026-04-09-local-memory-training-analysis-v1.md)
  - [Implementation Plan](./2026-04-09-local-memory-implementation-plan-v1.md)

## API Documentation

### Core Types

```rust
// Memory Entry
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
```

### Memory Content Types

```rust
pub enum MemoryContent {
    Text(String),
    Code(CodeContent),
    File(FileContent),
    Composite(Vec<MemoryContent>),
}
```

### Memory Status Lifecycle

```
Active → Cooling → Cold → Zombie
   ↑        ↑        ↑        ↑
   └────────┴────────┴────────┘
        Auto-transition based on policy
```

## Storage Architecture

```
┌─────────────────────────────────────────┐
│            MemoryClient                  │
└─────────────────┬───────────────────────┘
                  │
     ┌────────────┼────────────┐
     ▼            ▼            ▼
┌─────────┐ ┌─────────┐ ┌─────────┐
│   Hot   │ │  Cold   │ │ Zombie  │
│ (HashMap)│ │(SQLite) │ │ (Files) │
└─────────┘ └─────────┘ └─────────┘
   Active   Cooling/Cold   Zombie
```

## Search Architecture

```
┌─────────────────────────────────────────┐
│            SearchQuery                   │
└─────────────────┬───────────────────────┘
                  │
     ┌────────────┼────────────┐
     ▼            ▼            ▼
┌─────────┐ ┌─────────┐ ┌─────────┐
│ Vector  │ │Fulltext │ │Metadata │
│ (HNSW)  │ │(Tantivy)│ │  Index  │
└─────────┘ └─────────┘ └─────────┘
                  │
                  ▼
         ┌───────────────┐
         │Hybrid Search  │
         │(Weighted Combo)│
         └───────────────┘
```