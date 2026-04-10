# 工作区数据上传协议规范

## 概述

本文档描述了 Forge 用于将本地工作区文件同步到远程服务器进行语义代码搜索和上下文感知 AI 辅助的数据上传和同步协议。

---

## 目录

1. [系统架构](#1-系统架构)
2. [服务器配置](#2-服务器配置)
3. [协议规范](#3-协议规范)
4. [数据模型](#4-数据模型)
5. [同步流程](#5-同步流程)
6. [进度事件](#6-进度事件)
7. [HTTP 客户端配置](#7-http-客户端配置)
8. [认证](#8-认证)
9. [附录](#附录)

---

## 1. 系统架构

### 1.1 高层概览

```
┌─────────────────┐      gRPC       ┌──────────────────────┐
│   本地文件      │ ──────────────> │   api.forgecode.dev  │
│  （工作区）     │                 │   （索引服务）        │
└─────────────────┘                 └──────────────────────┘
        │
        ▼
┌─────────────────────────────────────────────────────────┐
│           WorkspaceSyncEngine (sync.rs)                 │
│  - FileDiscovery: 发现文件                               │
│  - 哈希比较 (SHA-256)                                    │
│  - 批量上传并发送进度事件                                │
└─────────────────────────────────────────────────────────┘
```

### 1.2 核心组件

| 组件 | 文件位置 | 用途 |
|------|----------|------|
| `WorkspaceSyncEngine` | `crates/forge_services/src/sync.rs:50-360` | 主同步引擎 |
| `WorkspaceIndexRepository` | `crates/forge_domain/src/repo.rs:103-173` | 仓库接口 |
| `ForgeContextEngineRepository` | `crates/forge_repo/src/context_engine.rs:86-386` | gRPC 实现 |
| `ForgeGrpcClient` | `crates/forge_infra/src/grpc.rs` | gRPC 通道管理 |

---

## 2. 服务器配置

### 2.1 端点

| 配置项 | 值 |
|--------|-----|
| **默认 URL** | `https://api.forgecode.dev/api` |
| **协议** | gRPC (HTTP/2) |
| **TLS** | 对 HTTPS 自动启用 |
| **配置文件** | `forge.yaml` → `services_url` 字段 |

### 2.2 配置位置

**文件**: `crates/forge_config/src/config.rs:177`

```rust
#[serde(default)]
#[dummy(expr = "\"https://api.forgecode.dev/api\".to_string()")]
pub services_url: String,
```

---

## 3. 协议规范

### 3.1 协议类型

- **序列化**: Protocol Buffers (protobuf)
- **传输**: gRPC over HTTP/2
- **Proto 文件**: `crates/forge_repo/proto/forge.proto`

### 3.2 服务方法

| 方法 | 请求 | 响应 | 用途 |
|------|------|------|------|
| `Search` | `SearchRequest` | `SearchResponse` | 语义搜索 |
| `UploadFiles` | `UploadFilesRequest` | `UploadFilesResponse` | 上传文件内容 |
| `DeleteFiles` | `DeleteFilesRequest` | `DeleteFilesResponse` | 删除文件 |
| `ListFiles` | `ListFilesRequest` | `ListFilesResponse` | 列出文件及哈希 |
| `ChunkFiles` | `ChunkFilesRequest` | `ChunkFilesResponse` | 分块文件 |
| `HealthCheck` | `HealthCheckRequest` | `HealthCheckResponse` | 健康检查 |
| `CreateWorkspace` | `CreateWorkspaceRequest` | `CreateWorkspaceResponse` | 创建工作区 |
| `ListWorkspaces` | `ListWorkspacesRequest` | `ListWorkspacesResponse` | 列出工作区 |
| `GetWorkspaceInfo` | `GetWorkspaceInfoRequest` | `GetWorkspaceInfoResponse` | 获取工作区信息 |
| `DeleteWorkspace` | `DeleteWorkspaceRequest` | `DeleteWorkspaceResponse` | 删除工作区 |
| `CreateApiKey` | `CreateApiKeyRequest` | `CreateApiKeyResponse` | 创建 API 密钥 |
| `ValidateFiles` | `ValidateFilesRequest` | `ValidateFilesResponse` | 语法验证 |
| `SelectSkill` | `SelectSkillRequest` | `SelectSkillResponse` | 技能选择 |
| `FuzzySearch` | `FuzzySearchRequest` | `FuzzySearchResponse` | 模糊搜索 |

---

## 4. 数据模型

### 4.1 Protobuf 消息

#### File（文件）

```protobuf
message File {
  string path    = 1;    // 文件路径（相对于工作区）
  string content = 2;    // UTF-8 文件内容
}
```

#### FileUploadContent（文件上传内容）

```protobuf
message FileUploadContent {
  repeated File    files = 1;
  optional GitInfo git   = 2;
}
```

#### GitInfo（Git 信息）

```protobuf
message GitInfo {
  optional string commit = 1;
  optional string branch = 2;
}
```

#### UploadFilesRequest（上传文件请求）

```protobuf
message UploadFilesRequest {
  WorkspaceId       workspace_id = 1;
  FileUploadContent content      = 2;
}
```

#### UploadFilesResponse（上传文件响应）

```protobuf
message UploadFilesResponse {
  UploadResult result = 1;
}

message UploadResult {
  repeated string               node_ids  = 1;
  repeated RelationCreateResult relations = 2;
}
```

#### DeleteFilesRequest（删除文件请求）

```protobuf
message DeleteFilesRequest {
  WorkspaceId     workspace_id = 1;
  repeated string file_paths   = 2;
}
```

#### DeleteFilesResponse（删除文件响应）

```protobuf
message DeleteFilesResponse {
  uint32 deleted_nodes     = 1;
  uint32 deleted_relations = 2;
}
```

#### ListFilesRequest（列出文件请求）

```protobuf
message ListFilesRequest {
  WorkspaceId workspace_id = 1;
}
```

#### ListFilesResponse（列出文件响应）

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

#### Workspace（工作区）

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

#### NodeKind（节点类型）

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

#### RelationType（关系类型）

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

### 4.2 Rust 领域类型

| 类型 | 位置 | 描述 |
|------|------|------|
| `FileRead` | `forge_domain/src/node.rs:108-120` | 用于上传的文件内容 |
| `FileHash` | `forge_domain/src/node.rs:370-384` | SHA-256 哈希 + 路径 |
| `CodeBase<T>` | `forge_domain/src/node.rs:130-150` | 通用包装器 (user_id, workspace_id, data) |
| `WorkspaceAuth` | `forge_domain/src/node.rs:85-93` | 认证令牌 |
| `FileStatus` | `forge_domain/src/node.rs:250-290` | 每个文件的同步状态 |
| `SyncStatus` | `forge_domain/src/node.rs:220-245` | 状态枚举: New/Modified/Unchanged/Deleted/Failed |

---

## 5. 同步流程

### 5.1 完整同步流程

```
┌─────────────────────────────────────────────────────────────────────┐
│                     WorkspaceSyncEngine::run()                      │
├─────────────────────────────────────────────────────────────────────┤
│                                                                      │
│  步骤 1: 文件发现                                                    │
│  ┌─────────────────────────────────────────────────────────────┐    │
│  │ discover_sync_file_paths()                                  │    │
│  │ → 扫描工作区目录                                             │    │
│  │ → 遵循 .gitignore 和配置过滤器                               │    │
│  │ → 返回 Vec<PathBuf>                                         │    │
│  └─────────────────────────────────────────────────────────────┘    │
│                              │                                       │
│                              ▼                                       │
│  步骤 2: 哈希比较                                                    │
│  ┌─────────────────────────────────────────────────────────────┐    │
│  │ 第一遍: 读取文件 → 计算 SHA-256 → 立即丢弃内容               │    │
│  │                                                              │    │
│  │ ListFiles RPC → 获取远程哈希                                 │    │
│  │                                                              │    │
│  │ 计算差异: 新增 / 修改 / 删除                                  │    │
│  └─────────────────────────────────────────────────────────────┘    │
│                              │                                       │
│                              ▼                                       │
│  步骤 3: 删除过时文件                                                │
│  ┌─────────────────────────────────────────────────────────────┐    │
│  │ if (deleted_files > 0) {                                    │    │
│  │   DeleteFiles RPC { workspace_id, file_paths[] }            │    │
│  │ }                                                           │    │
│  └─────────────────────────────────────────────────────────────┘    │
│                              │                                       │
│                              ▼                                       │
│  步骤 4: 上传新增/修改的文件                                         │
│  ┌─────────────────────────────────────────────────────────────┐    │
│  │ 对每个文件 (批量并行):                                       │    │
│  │   1. 按需读取文件内容                                        │    │
│  │   2. UploadFiles RPC { workspace_id, files[] }              │    │
│  │   3. 发送进度事件                                            │    │
│  └─────────────────────────────────────────────────────────────┘    │
│                              │                                       │
│                              ▼                                       │
│  步骤 5: 完成                                                        │
│  ┌─────────────────────────────────────────────────────────────┐    │
│  │ 发送 SyncProgress::Completed { total_files,                 │    │
│  │                              uploaded_files,                 │    │
│  │                              failed_files }                  │    │
│  └─────────────────────────────────────────────────────────────┘    │
│                                                                      │
└─────────────────────────────────────────────────────────────────────┘
```

### 5.2 内存优化

- **第一遍**: 批量读取文件，计算 SHA-256 哈希后**立即丢弃内容**
- **第二遍**: 在上传前**按需**读取文件内容
- 峰值内存: `batch_size × avg_file_size` 而非整个工作区大小

### 5.3 gRPC 通道管理

**文件**: `crates/forge_infra/src/grpc.rs`

```rust
impl ForgeGrpcClient {
    pub fn channel(&self) -> Channel {
        // 延迟连接 - 首次使用时创建
        let mut channel = Channel::from_shared(self.server_url.to_string())
            .concurrency_limit(256);

        // 对 HTTPS URL 自动启用 TLS
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

## 6. 进度事件

### 6.1 SyncProgress 枚举

```rust
pub enum SyncProgress {
    /// 同步操作开始
    Starting,
    
    /// 服务器上创建了新工作区
    WorkspaceCreated { workspace_id: WorkspaceId },
    
    /// 正在发现目录中的文件
    DiscoveringFiles { workspace_id: WorkspaceId, path: PathBuf },
    
    /// 已发现文件
    FilesDiscovered { count: usize },
    
    /// 正在比较本地文件与服务器状态
    ComparingFiles { remote_files: usize, local_files: usize },
    
    /// 计算差异，显示变更明细
    DiffComputed { added: usize, deleted: usize, modified: usize },
    
    /// 正在同步文件
    Syncing { current: usize, total: usize },
    
    /// 同步操作完成
    Completed { total_files: usize, uploaded_files: usize, failed_files: usize },
}
```

### 6.2 进度流程示例

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

## 7. HTTP 客户端配置

**文件**: `crates/forge_domain/src/http_config.rs`

| 设置 | 默认值 | 描述 |
|------|--------|------|
| 连接超时 | 30 秒 | 建立连接的最长时间 |
| 读取超时 | 900 秒 | 等待响应的最长时间 |
| 池空闲超时 | 90 秒 | 关闭连接前的空闲时间 |
| 每主机最大空闲数 | 5 | 每主机的最大空闲连接数 |
| HTTP/2 | 已启用 | 自适应窗口的 HTTP/2 |
| TLS | 默认 1.3 | 最低 TLS 版本 |

---

## 8. 认证

### 8.1 令牌类型

- **类型**: API Key (Bearer token)
- **存储**: 本地 SQLite 数据库
- **结构**: `WorkspaceAuth` 结构体

### 8.2 WorkspaceAuth 结构体

```rust
pub struct WorkspaceAuth {
    pub user_id: UserId,           // 拥有此认证的用户 ID
    pub token: ApiKey,             // 认证令牌
    pub created_at: DateTime<Utc>, // 令牌存储本地的时间
}
```

### 8.3 认证流程

1. 用户通过 OAuth/device code 流程进行身份验证
2. 从认证 API 获取 API 密钥
3. 密钥存储在本地 SQLite 数据库
4. 每个 gRPC 请求在元数据中包含 API 密钥

---

## 附录 A: 文件状态类型

### SyncStatus 枚举

```rust
pub enum SyncStatus {
    New,        // 文件本地存在但服务器上不存在
    Modified,   // 两边都存在但内容不同
    Unchanged,  // 两边相同
    Deleted,   // 服务器上存在但本地不存在
    Failed,    // 操作失败
}
```

---

## 附录 B: 配置文件示例

### forge.yaml

```yaml
services_url: "https://api.forgecode.dev/api"
max_parallel_file_reads: 50
model_cache_ttl_secs: 3600
```

---

## 附录 C: 相关文件

| 文件 | 描述 |
|------|------|
| `crates/forge_repo/proto/forge.proto` | Protocol Buffer 定义 |
| `crates/forge_services/src/sync.rs` | 主同步引擎实现 |
| `crates/forge_domain/src/node.rs` | 领域类型 |
| `crates/forge_domain/src/repo.rs` | 仓库 trait |
| `crates/forge_repo/src/context_engine.rs` | gRPC 客户端实现 |
| `crates/forge_infra/src/grpc.rs` | gRPC 通道管理 |
| `crates/forge_config/src/config.rs` | 配置类型 |
| `crates/forge_domain/src/http_config.rs` | HTTP 客户端配置 |

---

*文档版本: 1.0*  
*最后更新: 2026-04-09*  
*Forge 代码库: https://github.com/agentic-forge/forgecode*