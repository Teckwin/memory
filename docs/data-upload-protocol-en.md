# Workspace Data Upload Protocol Specification

## Overview

This document describes the data upload and synchronization protocol used by Forge to index local workspace files on a remote server for semantic code search and context-aware AI assistance.

---

## Table of Contents

1. [System Architecture](#1-system-architecture)
2. [Server Configuration](#2-server-configuration)
3. [Protocol Specification](#3-protocol-specification)
4. [Data Models](#4-data-models)
5. [Sync Flow](#5-sync-flow)
6. [Progress Events](#6-progress-events)
7. [HTTP Client Configuration](#7-http-client-configuration)
8. [Authentication](#8-authentication)
9. [Appendix](#appendix)

---

## 1. System Architecture

### 1.1 High-Level Overview

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

### 1.2 Core Components

| Component | File Location | Purpose |
|-----------|--------------|---------|
| `WorkspaceSyncEngine` | `crates/forge_services/src/sync.rs:50-360` | Main sync engine |
| `WorkspaceIndexRepository` | `crates/forge_domain/src/repo.rs:103-173` | Repository interface |
| `ForgeContextEngineRepository` | `crates/forge_repo/src/context_engine.rs:86-386` | gRPC implementation |
| `ForgeGrpcClient` | `crates/forge_infra/src/grpc.rs` | gRPC channel management |

---

## 2. Server Configuration

### 2.1 Endpoint

| Configuration | Value |
|---------------|-------|
| **Default URL** | `https://api.forgecode.dev/api` |
| **Protocol** | gRPC (HTTP/2) |
| **TLS** | Enabled automatically for HTTPS |
| **Config File** | `forge.yaml` → `services_url` field |

### 2.2 Configuration Location

**File**: `crates/forge_config/src/config.rs:177`

```rust
#[serde(default)]
#[dummy(expr = "\"https://api.forgecode.dev/api\".to_string()")]
pub services_url: String,
```

---

## 3. Protocol Specification

### 3.1 Protocol Type

- **Serialization**: Protocol Buffers (protobuf)
- **Transport**: gRPC over HTTP/2
- **Proto File**: `crates/forge_repo/proto/forge.proto`

### 3.2 Service Methods

| Method | Request | Response | Purpose |
|--------|---------|----------|---------|
| `Search` | `SearchRequest` | `SearchResponse` | Semantic search |
| `UploadFiles` | `UploadFilesRequest` | `UploadFilesResponse` | Upload file content |
| `DeleteFiles` | `DeleteFilesRequest` | `DeleteFilesResponse` | Delete files |
| `ListFiles` | `ListFilesRequest` | `ListFilesResponse` | List files with hashes |
| `ChunkFiles` | `ChunkFilesRequest` | `ChunkFilesResponse` | Split files into chunks |
| `HealthCheck` | `HealthCheckRequest` | `HealthCheckResponse` | Health check |
| `CreateWorkspace` | `CreateWorkspaceRequest` | `CreateWorkspaceResponse` | Create workspace |
| `ListWorkspaces` | `ListWorkspacesRequest` | `ListWorkspacesResponse` | List workspaces |
| `GetWorkspaceInfo` | `GetWorkspaceInfoRequest` | `GetWorkspaceInfoResponse` | Get workspace info |
| `DeleteWorkspace` | `DeleteWorkspaceRequest` | `DeleteWorkspaceResponse` | Delete workspace |
| `CreateApiKey` | `CreateApiKeyRequest` | `CreateApiKeyResponse` | Create API key |
| `ValidateFiles` | `ValidateFilesRequest` | `ValidateFilesResponse` | Syntax validation |
| `SelectSkill` | `SelectSkillRequest` | `SelectSkillResponse` | Skill selection |
| `FuzzySearch` | `FuzzySearchRequest` | `FuzzySearchResponse` | Fuzzy search |

---

## 4. Data Models

### 4.1 Protobuf Messages

#### File

```protobuf
message File {
  string path    = 1;    // File path (relative to workspace)
  string content = 2;    // UTF-8 file content
}
```

#### FileUploadContent

```protobuf
message FileUploadContent {
  repeated File    files = 1;
  optional GitInfo git   = 2;
}
```

#### GitInfo

```protobuf
message GitInfo {
  optional string commit = 1;
  optional string branch = 2;
}
```

#### UploadFilesRequest

```protobuf
message UploadFilesRequest {
  WorkspaceId       workspace_id = 1;
  FileUploadContent content      = 2;
}
```

#### UploadFilesResponse

```protobuf
message UploadFilesResponse {
  UploadResult result = 1;
}

message UploadResult {
  repeated string               node_ids  = 1;
  repeated RelationCreateResult relations = 2;
}
```

#### DeleteFilesRequest

```protobuf
message DeleteFilesRequest {
  WorkspaceId     workspace_id = 1;
  repeated string file_paths   = 2;
}
```

#### DeleteFilesResponse

```protobuf
message DeleteFilesResponse {
  uint32 deleted_nodes     = 1;
  uint32 deleted_relations = 2;
}
```

#### ListFilesRequest

```protobuf
message ListFilesRequest {
  WorkspaceId workspace_id = 1;
}
```

#### ListFilesResponse

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

#### Workspace

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

#### Node Types

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

#### Relation Types

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

### 4.2 Rust Domain Types

| Type | Location | Description |
|------------|----------------|-------------------|
| `FileRead` | `forge_domain/src/node.rs:108-120` | File content for upload |
| `FileHash` | `forge_domain/src/node.rs:370-384` | SHA-256 hash + path |
| `CodeBase<T>` | `forge_domain/src/node.rs:130-150` | Generic wrapper (user_id, workspace_id, data) |
| `WorkspaceAuth` | `forge_domain/src/node.rs:85-93` | Authentication token |
| `FileStatus` | `forge_domain/src/node.rs:250-290` | Sync status per file |
| `SyncStatus` | `forge_domain/src/node.rs:220-245` | Status enum: New/Modified/Unchanged/Deleted/Failed |

---

## 5. Sync Flow

### 5.1 Complete Sync Process

```
┌─────────────────────────────────────────────────────────────────────┐
│                     WorkspaceSyncEngine::run()                      │
├─────────────────────────────────────────────────────────────────────┤
│                                                                      │
│  Step 1: File Discovery                                             │
│  ┌─────────────────────────────────────────────────────────────┐    │
│  │ discover_sync_file_paths()                                  │    │
│  │ → Scans workspace directory                                 │    │
│  │ → Respects .gitignore and config filters                    │    │
│  │ → Returns Vec<PathBuf>                                      │    │
│  └─────────────────────────────────────────────────────────────┘    │
│                              │                                       │
│                              ▼                                       │
│  Step 2: Hash Comparison                                           │
│  ┌─────────────────────────────────────────────────────────────┐    │
│  │ Pass 1: Read files → Compute SHA-256 → Discard content      │    │
│  │                                                              │    │
│  │ ListFiles RPC → Get remote hashes                           │    │
│  │                                                              │    │
│  │ Compute diff: New / Modified / Deleted                      │    │
│  └─────────────────────────────────────────────────────────────┘    │
│                              │                                       │
│                              ▼                                       │
│  Step 3: Delete Stale Files                                         │
│  ┌─────────────────────────────────────────────────────────────┐    │
│  │ if (deleted_files > 0) {                                    │    │
│  │   DeleteFiles RPC { workspace_id, file_paths[] }            │    │
│  │ }                                                           │    │
│  └─────────────────────────────────────────────────────────────┘    │
│                              │                                       │
│                              ▼                                       │
│  Step 4: Upload New/Modified Files                                  │
│  ┌─────────────────────────────────────────────────────────────┐    │
│  │ For each file (batched, parallel):                          │    │
│  │   1. Read file content on-demand                            │    │
│  │   2. UploadFiles RPC { workspace_id, files[] }              │    │
│  │   3. Emit progress event                                    │    │
│  └─────────────────────────────────────────────────────────────┘    │
│                              │                                       │
│                              ▼                                       │
│  Step 5: Complete                                                  │
│  ┌─────────────────────────────────────────────────────────────┐    │
│  │ Emit SyncProgress::Completed { total_files,                 │    │
│  │                                 uploaded_files,              │    │
│  │                                 failed_files }               │    │
│  └─────────────────────────────────────────────────────────────┘    │
│                                                                      │
└─────────────────────────────────────────────────────────────────────┘
```

### 5.2 Memory Optimization

- **Pass 1**: Read files in batches, compute SHA-256 hash, then **discard content immediately**
- **Pass 2**: Read file content **on-demand** immediately before upload
- Peak memory: `batch_size × avg_file_size` instead of entire workspace size

### 5.3 gRPC Channel Management

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

## 6. Progress Events

### 6.1 SyncProgress Enum

```rust
pub enum SyncProgress {
    /// Sync operation is starting
    Starting,
    
    /// A new workspace was created on the server
    WorkspaceCreated { workspace_id: WorkspaceId },
    
    /// Discovering files in the directory
    DiscoveringFiles { workspace_id: WorkspaceId, path: PathBuf },
    
    /// Files have been discovered
    FilesDiscovered { count: usize },
    
    /// Comparing local files with server state
    ComparingFiles { remote_files: usize, local_files: usize },
    
    /// Diff computed showing breakdown of changes
    DiffComputed { added: usize, deleted: usize, modified: usize },
    
    /// Syncing files
    Syncing { current: usize, total: usize },
    
    /// Sync operation completed
    Completed { total_files: usize, uploaded_files: usize, failed_files: usize },
}
```

### 6.2 Progress Flow Example

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

## 7. HTTP Client Configuration

**File**: `crates/forge_domain/src/http_config.rs`

| Setting | Default Value | Description |
|---------------|----------------------|-------------------|
| Connect Timeout | 30 seconds | Maximum time to establish connection |
| Read Timeout | 900 seconds | Maximum time to wait for response |
| Pool Idle Timeout | 90 seconds | Idle time before closing connection |
| Max Idle per Host | 5 | Maximum idle connections per host |
| HTTP/2 | Enabled | HTTP/2 with adaptive window |
| TLS | 1.3 default | Minimum TLS version |

---

## 8. Authentication

### 8.1 Token Type

- **Type**: API Key (Bearer token)
- **Storage**: Local SQLite database
- **Structure**: `WorkspaceAuth` struct

### 8.2 WorkspaceAuth Struct

```rust
pub struct WorkspaceAuth {
    pub user_id: UserId,           // User ID that owns this authentication
    pub token: ApiKey,             // Authentication token
    pub created_at: DateTime<Utc>, // When token was stored locally
}
```

### 8.3 Authentication Flow

1. User authenticates via OAuth/device code flow
2. API key is obtained from authentication API
3. Key is stored locally in SQLite database
4. Each gRPC request includes the API key in metadata

---

## Appendix A: File Status Types

### SyncStatus Enum

```rust
pub enum SyncStatus {
    New,        // File exists locally but not on server
    Modified,   // File exists both places but content differs
    Unchanged,  // File identical on both
    Deleted,   // File exists on server but not locally
    Failed,    // Operation failed
}
```

---

## Appendix B: Configuration File Example

### forge.yaml

```yaml
services_url: "https://api.forgecode.dev/api"
max_parallel_file_reads: 50
model_cache_ttl_secs: 3600
```

---

## Appendix C: Related Files

| File | Description |
|------------|-------------------|
| `crates/forge_repo/proto/forge.proto` | Protocol Buffer definitions |
| `crates/forge_services/src/sync.rs` | Main sync engine implementation |
| `crates/forge_domain/src/node.rs` | Domain types |
| `crates/forge_domain/src/repo.rs` | Repository traits |
| `crates/forge_repo/src/context_engine.rs` | gRPC client implementation |
| `crates/forge_infra/src/grpc.rs` | gRPC channel management |
| `crates/forge_config/src/config.rs` | Configuration types |
| `crates/forge_domain/src/http_config.rs` | HTTP client configuration |

---

*Document Version: 1.0*  
*Last Updated: 2026-04-09*  
*Forge Codebase: https://github.com/agentic-forge/forgecode*