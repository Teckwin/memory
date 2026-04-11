# 记忆系统文档

本目录包含记忆管理系统的文档。

[English Version](./README.md)

## 内容

### 指南
- [数据上传协议](./data-upload-protocol-zh.md) - 向记忆系统上传数据的协议
  - [English](./data-upload-protocol.md)
  - [中文](./data-upload-protocol-zh.md)

### 实现计划
- [架构设计](../plans/2026-04-09-local-memory-management-system-v1.md)
- [训练分析](../plans/2026-04-09-local-memory-training-analysis-v1.md)
- [实施计划](../plans/2026-04-09-local-memory-implementation-plan-v1.md)

## API 文档

### 核心类型

```rust
// 记忆条目
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

### 记忆内容类型

```rust
pub enum MemoryContent {
    Text(String),
    Code(CodeContent),
    File(FileContent),
    Composite(Vec<MemoryContent>),
}
```

### 记忆状态生命周期

```
Active → Cooling → Cold → Zombie
   ↑        ↑        ↑        ↑
   └────────┴────────┴────────┘
        基于策略的自动转换
```

## 存储架构

```
┌─────────────────────────────────────────┐
│            MemoryClient                  │
└─────────────────┬───────────────────────┘
                  │
     ┌────────────┼────────────┐
     ▼            ▼            ▼
┌─────────┐ ┌─────────┐ ┌─────────┐
│   热存储 │ │  冷存储  │ │ 僵尸存储 │
│ (HashMap)│ │(SQLite) │ │ (文件)  │
└─────────┘ └─────────┘ └─────────┘
   Active   Cooling/Cold   Zombie
```

## 搜索架构

```
┌─────────────────────────────────────────┐
│            SearchQuery                   │
└─────────────────┬───────────────────────┘
                  │
     ┌────────────┼────────────┐
     ▼            ▼            ▼
┌─────────┐ ┌─────────┐ ┌─────────┐
│ 向量索引 │ │ 全文索引 │ │ 元数据  │
│ (HNSW)  │ │(Tantivy)│ │  索引   │
└─────────┘ └─────────┘ └─────────┘
                  │
                  ▼
         ┌───────────────┐
         │  混合搜索     │
         │(加权组合)     │
         └───────────────┘
```