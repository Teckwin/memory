# Memory System Documentation

This directory contains documentation for the Memory Management System.

[中文版](./README-ZH.md)

## Contents

### Guides
- [Data Upload Protocol](./data-upload-protocol.md) - Protocol for uploading data
  - [English](./data-upload-protocol.md)
  - [中文](./data-upload-protocol-zh.md)

### Implementation Plans
- [Architecture Design](../plans/2026-04-09-local-memory-management-system-v1.md)
- [Training Analysis](../plans/2026-04-09-local-memory-training-analysis-v1.md)
- [Implementation Plan](../plans/2026-04-09-local-memory-implementation-plan-v1.md)

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