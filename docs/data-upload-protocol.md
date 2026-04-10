# Workspace Data Upload Protocol Specification

## Overview

This document describes the data upload and synchronization protocol used by Forge to index local workspace files on a remote server for semantic code search and context-aware AI assistance.

本文档描述了 Forge 用于将本地工作区文件同步到远程服务器进行语义代码搜索和上下文感知 AI 辅助的数据上传和同步协议。

---

## Table of Contents / 目录

1. [System Architecture / 系统架构](#1-system-architecture--系统架构)
2. [Server Configuration / 服务器配置](#2-server-configuration--服务器配置)
3. [Protocol Specification / 协议规范](#3-protocol-specification--协议规范)
4. [Data Models / 数据模型](#4-data-models--数据模型)
5. [Sync Flow / 同步流程](#5-sync-flow--同步流程)
6. [Progress Events / 进度事件](#6-progress-events--进度事件)
7. [HTTP Client Configuration / HTTP 客户端配置](#7-http-client-configuration--http-客户端配置)
8. [Authentication / 认证](#8-authentication--认证)

---

## 1. System Architecture / 系统架构

### 1.1 High-Level Overview / 高层概览

```
┌─────────────────┐      gRPC       ┌──────────────────────┐
│   Local Files   │ ──────────────> │   api.forgecode.dev  │
│  (workspace)    │                 │   (indexing service) │
└─────────────────┘                 └──────────────────────┘
        │
        ▼
┌─────────────────────────────────────────────────────────┐
│           WorkspaceSyncEngine (sync.rs)                 │
│  - FileDiscovery: finds files                           │
│  - Hash comparison (SHA-256)                            │
│  - Batch upload with progress events                    │
└─────────────────────────────────────────────────────────┘
```

### 1.2 Core Components / 核心组件

| Component / 组件 | File Location / 文件位置 | Purpose / 用途 |
|-----------------|-------------------------|----------------|
| `WorkspaceSyncEngine` | `crates/forge_services/src/sync.rs:50-360` | Main sync engine / 主同步引擎 |
| `WorkspaceIndexRepository` | `crates/forge_domain/src/repo.rs:103-173` | Repository interface / 仓库接口 |
| `ForgeContextEngineRepository` | `crates/forge_repo/src/context_engine.rs:86-386` | gRPC implementation / gRPC 实现 |
| `ForgeGrpcClient` | `crates/forge_infra/src/grpc.rs` | gRPC channel management / gRPC 通道管理 |

---

## 2. Server Configuration / 服务器配置

### 2.1 Endpoint / 端点

| Configuration / 配置 | Value / 值 |
|---------------------|-----------|
| **Default URL** | `https://api.forgecode.dev/api` |
| **Protocol** | gRPC (HTTP/2) |
| **TLS** | Enabled automatically for HTTPS / 对 HTTPS 自动启用 |
| **Config File** | `forge.yaml` → `services_url` field |

### 2.2 Configuration Location / 配置位置

**File**: `crates/forge_config/src/config.rs:177`

```rust
#[serde(default)]
#[dummy(expr = "\"https://api.forgecode.dev/api\".to_string()")]
pub services_url: String,
```

---

## 3. Protocol Specification / 协议规范

### 3.1 Protocol Type / 协议类型

- **Serialization**: Protocol Buffers (protobuf)
- **Transport**: gRPC over HTTP/2
- **Proto File**: `crates/forge_repo/proto/forge.proto`

### 3.2 Service Methods / 服务方法

| Method / 方法 | Request / 请求 | Response / 响应 | Purpose / 用途 |
|--------------|---------------|-----------------|----------------|
| `Search` | `SearchRequest` | `SearchResponse` | Semantic search / 语义搜索 |
| `UploadFiles` | `UploadFilesRequest` | `UploadFilesResponse` | Upload file content / 上传文件内容 |
| `DeleteFiles` | `DeleteFilesRequest` | `DeleteFilesResponse` | Delete files / 删除文件 |
| `ListFiles` | `ListFilesRequest` | `ListFilesResponse` | List files with hashes / 列出文件及哈希 |
| `ChunkFiles` | `ChunkFilesRequest` | `ChunkFilesResponse` | Split files into chunks / 分块文件 |
| `HealthCheck` | `HealthCheckRequest` | `HealthCheckResponse` | Health check / 健康检查 |
| `CreateWorkspace` | `CreateWorkspaceRequest` | `CreateWorkspaceResponse` | Create workspace / 创建工作区 |
| `ListWorkspaces` | `ListWorkspacesRequest` | `ListWorkspacesResponse` | List workspaces / 列出工作区 |
| `GetWorkspaceInfo` | `GetWorkspaceInfoRequest` | `GetWorkspaceInfoResponse` | Get workspace info / 获取工作区信息 |
| `DeleteWorkspace` | `DeleteWorkspaceRequest` | `DeleteWorkspaceResponse` | Delete workspace / 删除工作区 |
| `CreateApiKey` | `CreateApiKeyRequest` | `CreateApiKeyResponse` | Create API key / 创建 API 密钥 |
| `ValidateFiles` | `ValidateFilesRequest` | `ValidateFilesResponse` | Syntax validation / 语法验证 |
| `SelectSkill` | `SelectSkillRequest` | `SelectSkillResponse` | Skill selection / 技能选择 |
| `FuzzySearch` | `FuzzySearchRequest` | `FuzzySearchResponse` | Fuzzy search / 模糊搜索 |

---

## 4. Data Models / 数据模型

### 4.1 Protobuf Messages / Protobuf 消息

#### File / 文件

```protobuf
message File {
  string path    = 1;    // File path (relative to workspace) / 文件路径（相对于工作区）
  string content = 2;    // UTF-8 file content / UTF-8 文件内容
}
```

#### FileUploadContent / 文件上传内容

```protobuf
message FileUploadContent {
  repeated File    files = 1;
  optional GitInfo git   = 2;
}
```

#### GitInfo / Git 信息

```protobuf
message GitInfo {
  optional string commit = 1;
  optional string branch = 2;
}
```

#### UploadFilesRequest / 上传文件请求

```protobuf
message UploadFilesRequest {
  WorkspaceId       workspace_id = 1;
  FileUploadContent content      = 2;
}
```

#### UploadFilesResponse / 上传文件响应

```protobuf
message UploadFilesResponse {
  UploadResult result = 1;
}

message UploadResult {
  repeated string               node_ids  = 1;
  repeated RelationCreateResult relations = 2;
}
```

#### DeleteFilesRequest / 删除文件请求

```protobuf
message DeleteFilesRequest {
  WorkspaceId     workspace_id = 1;
  repeated string file_paths   = 2;
}
```

#### DeleteFilesResponse / 删除文件响应

```protobuf
message DeleteFilesResponse {
  uint32 deleted_nodes     = 1;
  uint32 deleted_relations = 2;
}
```

#### ListFilesRequest / 列出文件请求

```protobuf
message ListFilesRequest {
  WorkspaceId workspace_id = 1;
}
```

#### ListFilesResponse / 列出文件响应

```protobuf
message ListFilesResponse {
  repeated FileRefNode files = 1;
}

message FileRefNode {
  NodeId           node_id = 1;
  string           hash    = 2;
  optional GitInfo git     = 3;
  FileRef          data    = 4;
}
```

#### Workspace / 工作区

```protobuf
message Workspace {
  WorkspaceId                        workspace_id   = 1;
  string                             working_dir    = 2;
  optional uint64                    node_count     = 3;
  optional uint64                    relation_count = 4;
  optional google.protobuf.Timestamp last_updated   = 5;
  uint32                             min_chunk_size = 6;
  uint32                             max_chunk_size = 7;
  google.protobuf.Timestamp          created_at     = 8;
}
```

#### Node Types / 节点类型

```protobuf
enum NodeKind {
  NODE_KIND_UNSPECIFIED = 0;
  NODE_KIND_FILE        = 1;
  NODE_KIND_FILE_CHUNK  = 2;
  NODE_KIND_FILE_REF    = 3;
  NODE_KIND_NOTE        = 4;
  NODE_KIND_TASK        = 5;
}
```

#### Relation Types / 关系类型

```protobuf
enum RelationType {
  RELATION_TYPE_UNSPECIFIED = 0;
  RELATION_TYPE_CALLS       = 1;
  RELATION_TYPE_EXTENDS     = 2;
  RELATION_TYPE_IMPLEMENTS  = 3;
  RELATION_TYPE_USES        = 4;
  RELATION_TYPE_DEFINES     = 5;
  RELATION_TYPE_REFERENCES  = 6;
  RELATION_TYPE_CONTAINS    = 7;
  RELATION_TYPE_DEPENDS_ON  = 8;
  RELATION_TYPE_RELATED_TO  = 9;
  RELATION_TYPE_INVERSE     = 10;
}
```

### 4.2 Rust Domain Types / Rust 领域类型

| Type / 类型 | Location / 位置 | Description / 描述 |
|------------|----------------|-------------------|
| `FileRead` | `forge_domain/src/node.rs:108-120` | File content for upload / 用于上传的文件内容 |
| `FileHash` | `forge_domain/src/node.rs:370-384` | SHA-256 hash + path / SHA-256 哈希 + 路径 |
| `CodeBase<T>` | `forge_domain/src/node.rs:130-150` | Generic wrapper (user_id, workspace_id, data) / 通用包装器 |
| `WorkspaceAuth` | `forge_domain/src/node.rs:85-93` | Authentication token / 认证令牌 |
| `FileStatus` | `forge_domain/src/node.rs:250-290` | Sync status per file / 每个文件的同步状态 |
| `SyncStatus` | `forge_domain/src/node.rs:220-245` | Status enum: New/Modified/Unchanged/Deleted/Failed / 状态枚举 |

---

## 5. Sync Flow / 同步流程

### 5.1 Complete Sync Process / 完整同步流程

```
┌─────────────────────────────────────────────────────────────────────┐
│                     WorkspaceSyncEngine::run()                      │
├─────────────────────────────────────────────────────────────────────┤
│                                                                      │
│  Step 1: File Discovery / 步骤 1: 文件发现                          │
│  ┌─────────────────────────────────────────────────────────────┐    │
│  │ discover_sync_file_paths()                                  │    │
│  │ → Scans workspace directory                                 │    │
│  │ → Respects .gitignore and config filters                    │    │
│  │ → Returns Vec<PathBuf>                                      │    │
│  └─────────────────────────────────────────────────────────────┘    │
│                              │                                       │
│                              ▼                                       │
│  Step 2: Hash Comparison / 步骤 2: 哈希比较                         │
│  ┌─────────────────────────────────────────────────────────────┐    │
│  │ Pass 1: Read files → Compute SHA-256 → Discard content      │    │
│  │                                                              │    │
│  │ ListFiles RPC → Get remote hashes                           │    │
│  │                                                              │    │
│  │ Compute diff: New / Modified / Deleted                      │    │
│  └─────────────────────────────────────────────────────────────┘    │
│                              │                                       │
│                              ▼                                       │
│  Step 3: Delete Stale Files / 步骤 3: 删除过时文件                  │
│  ┌─────────────────────────────────────────────────────────────┐    │
│  │ if (deleted_files > 0) {                                    │    │
│  │   DeleteFiles RPC { workspace_id, file_paths[] }            │    │
│  │ }                                                           │    │
│  └─────────────────────────────────────────────────────────────┘    │
│                              │                                       │
│                              ▼                                       │
│  Step 4: Upload New/Modified Files / 步骤 4: 上传新增/修改的文件    │
│  ┌─────────────────────────────────────────────────────────────┐    │
│  │ For each file (batched, parallel):                          │    │
│  │   1. Read file content on-demand                            │    │
│  │   2. UploadFiles RPC { workspace_id, files[] }              │    │
│  │   3. Emit progress event                                    │    │
│  └─────────────────────────────────────────────────────────────┘    │
│                              │                                       │
│                              ▼                                       │
│  Step 5: Complete / 步骤 5: 完成                                    │
│  ┌─────────────────────────────────────────────────────────────┐    │
│  │ Emit SyncProgress::Completed { total_files,                 │    │
│  │                                 uploaded_files,              │    │
│  │                                 failed_files }               │    │
│  └─────────────────────────────────────────────────────────────┘    │
│                                                                      │
└─────────────────────────────────────────────────────────────────────┘
```

### 5.2 Memory Optimization / 内存优化

- **Pass 1**: Read files in batches, compute SHA-256 hash, then **discard content immediately**
- **Pass 2**: Read file content **on-demand** immediately before upload
- Peak memory: `batch_size × avg_file_size` instead of entire workspace size

### 5.3 gRPC Channel Management / gRPC 通道管理

**File**: `crates/forge_infra/src/grpc.rs`

```rust
impl ForgeGrpcClient {
    pub fn channel(&self) -> Channel {
        // Lazy connection - created on first use
        let mut channel = Channel::from_shared(self.server_url.to_string())
            .concurrency_limit(256);

        // Auto-enable TLS for https URLs
        if self.server_url.scheme().contains("https") {
            let tls_config = tonic::transport::ClientTlsConfig::new()
                .with_webpki_roots();
            channel = channel.tls_config(tls_config);
        }

        channel.connect_lazy()
    }
}
```

---

## 6. Progress Events / 进度事件

### 6.1 SyncProgress Enum / SyncProgress 枚举

```rust
pub enum SyncProgress {
    /// Sync operation is starting / 同步操作开始
    Starting,
    
    /// A new workspace was created on the server / 服务器上创建了新工作区
    WorkspaceCreated { workspace_id: WorkspaceId },
    
    /// Discovering files in the directory / 正在发现目录中的文件
    DiscoveringFiles { workspace_id: WorkspaceId, path: PathBuf },
    
    /// Files have been discovered / 已发现文件
    FilesDiscovered { count: usize },
    
    /// Comparing local files with server state / 正在比较本地文件与服务器状态
    ComparingFiles { remote_files: usize, local_files: usize },
    
    /// Diff computed showing breakdown of changes / 计算差异，显示变更明细
    DiffComputed { added: usize, deleted: usize, modified: usize },
    
    /// Syncing files / 正在同步文件
    Syncing { current: usize, total: usize },
    
    /// Sync operation completed / 同步操作完成
    Completed { total_files: usize, uploaded_files: usize, failed_files: usize },
}
```

### 6.2 Progress Flow Example / 进度流程示例

```
DiscoveringFiles { workspace_id: "xxx", path: "/project" }
  → FilesDiscovered { count: 150 }
  → ComparingFiles { remote_files: 145, local_files: 150 }
  → DiffComputed { added: 3, deleted: 0, modified: 2 }
  → Syncing { current: 1, total: 5 }
  → Syncing { current: 2, total: 5 }
  → Syncing { current: 3, total: 5 }
  → Syncing { current: 4, total: 5 }
  → Syncing { current: 5, total: 5 }
  → Completed { total_files: 150, uploaded_files: 5, failed_files: 0 }
```

---

## 7. HTTP Client Configuration / HTTP 客户端配置

**File**: `crates/forge_domain/src/http_config.rs`

| Setting / 设置 | Default Value / 默认值 | Description / 描述 |
|---------------|----------------------|-------------------|
| Connect Timeout / 连接超时 | 30 seconds | Maximum time to establish connection / 建立连接的最长时间 |
| Read Timeout / 读取超时 | 900 seconds | Maximum time to wait for response / 等待响应的最长时间 |
| Pool Idle Timeout / 池空闲超时 | 90 seconds | Idle time before closing connection / 关闭连接前的空闲时间 |
| Max Idle per Host / 每主机最大空闲数 | 5 | Maximum idle connections per host / 每主机的最大空闲连接数 |
| HTTP/2 | Enabled | HTTP/2 with adaptive window / 自适应窗口的 HTTP/2 |
| TLS | 1.3 default | Minimum TLS version / 最低 TLS 版本 |

---

## 8. Authentication / 认证

### 8.1 Token Type / 令牌类型

- **Type**: API Key (Bearer token)
- **Storage**: Local SQLite database
- **Structure**: `WorkspaceAuth` struct

### 8.2 WorkspaceAuth Struct / WorkspaceAuth 结构体

```rust
pub struct WorkspaceAuth {
    pub user_id: UserId,           // User ID that owns this authentication / 拥有此认证的用户 ID
    pub token: ApiKey,             // Authentication token / 认证令牌
    pub created_at: DateTime<Utc>, // When token was stored locally / 令牌存储本地的 时间
}
```

### 8.3 Authentication Flow / 认证流程

1. User authenticates via OAuth/device code flow
2. API key is obtained from authentication API
3. Key is stored locally in SQLite database
4. Each gRPC request includes the API key in metadata

---

## Appendix A: File Status Types / 附录 A: 文件状态类型

### SyncStatus Enum / SyncStatus 枚举

```rust
pub enum SyncStatus {
    New,        // File exists locally but not on server / 文件本地存在但服务器上不存在
    Modified,   // File exists both places but content differs / 两边都存在但内容不同
    Unchanged,  // File identical on both / 两边相同
    Deleted,   // File exists on server but not locally / 服务器上存在但本地不存在
    Failed,    // Operation failed / 操作失败
}
```

---

## Appendix B: Configuration File Example / 附录 B: 配置文件示例

### forge.yaml

```yaml
services_url: "https://api.forgecode.dev/api"
max_parallel_file_reads: 50
model_cache_ttl_secs: 3600
```

---

## Appendix C: Related Files / 附录 C: 相关文件

| File / 文件 | Description / 描述 |
|------------|-------------------|
| `crates/forge_repo/proto/forge.proto` | Protocol Buffer definitions / Protocol Buffer 定义 |
| `crates/forge_services/src/sync.rs` | Main sync engine implementation / 主同步引擎实现 |
| `crates/forge_domain/src/node.rs` | Domain types / 领域类型 |
| `crates/forge_domain/src/repo.rs` | Repository traits / 仓库 trait |
| `crates/forge_repo/src/context_engine.rs` | gRPC client implementation / gRPC 客户端实现 |
| `crates/forge_infra/src/grpc.rs` | gRPC channel management / gRPC 通道管理 |
| `crates/forge_config/src/config.rs` | Configuration types / 配置类型 |
| `crates/forge_domain/src/http_config.rs` | HTTP client configuration / HTTP 客户端配置 |

---

*Document Version: 1.0*  
*Last Updated: 2026-04-09*  
*Forge Codebase: https://github.com/agentic-forge/forgecode*