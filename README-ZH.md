# 记忆管理系统

一个面向 AI 应用的本地记忆管理系统，提供高效的记忆存储、检索和自动学习能力。

## 特性

- **分层存储**: 热数据（内存）、冷数据（SQLite）、僵尸数据（归档）三层存储
- **多索引搜索**: 向量（HNSW）、全文（Tantivy）和混合搜索
- **生命周期管理**: 自动记忆状态转换（Active → Cooling → Cold → Zombie）
- **训练模块**: 数据准备和模型管理，支持持续学习
- **工作区支持**: 多工作区隔离，独立配置

## 项目结构

```
memory/
├── Cargo.toml              # 工作区根配置
├── crates/
│   ├── memory-core/        # 核心类型和 Trait 定义
│   ├── memory-api/         # 客户端 API
│   ├── memory-storage/     # 存储层（热/冷/僵尸）
│   ├── memory-index/       # 索引层（向量/全文/混合）
│   ├── memory-lifecycle/   # 生命周期管理
│   └── memory-train/       # 训练模块
├── docs/
└── plans/
```

## 快速开始

```rust
use memory_api::MemoryClient;
use memory_core::{MemoryEntry, MemoryContent, MemoryMetadata, MemorySource, WorkspaceId, Workspace};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 创建客户端
    let client = MemoryClient::new(Default::default());

    // 创建工作区
    let workspace_id = client
        .create(Workspace::new("我的工作区".to_string()))
        .await?;

    // 添加记忆
    let memory = MemoryEntry::new(
        workspace_id,
        MemoryContent::Text("你好，世界！".to_string()),
        MemoryMetadata::new(MemorySource::UserQuery {
            query: "问候".to_string(),
        }),
    );
    let memory_id = client.add(memory).await?;

    // 检索记忆
    let retrieved = client.get(memory_id).await?;
    println!("检索到: {:?}", retrieved.content);

    Ok(())
}
```

## 模块说明

### memory-core
核心类型和 Trait 定义：
- `MemoryEntry`: 记忆单元
- `Workspace`: 记忆容器
- `SearchQuery` / `SearchResult`: 搜索操作
- `MemoryApi`, `SearchApi`, `WorkspaceApi` trait

### memory-api
实现所有核心 trait 的高级客户端 API。

### memory-storage
分层存储实现：
- **Hot**: Active 记忆的内存缓存
- **Cold**: Cooling/Cold 记忆的 SQLite 存储
- **Zombie**: 归档记忆的文件存储

### memory-index
搜索索引：
- **VectorIndex**: 基于 HNSW 的近似最近邻搜索
- **FulltextIndex**: 基于 Tantivy 的全文搜索
- **HybridIndex**: 向量 + 全文组合搜索

### memory-lifecycle
自动生命周期管理：
- 可配置的转换策略
- 周期转换调度器
- 归档管理

### memory-train
训练能力：
- 从记忆准备训练数据
- 模型管理（保存/加载）
- 向量嵌入生成

## 配置

### 存储配置
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

### 生命周期配置
```rust
let policy_config = PolicyConfig {
    active_to_cooling_days: 7,
    cooling_to_cold_days: 14,
    cold_to_zombie_days: 30,
    ..Default::default()
};
```

## 测试

```bash
# 运行所有测试
cargo test --workspace

# 运行特定 crate 的测试
cargo test -p memory-core
cargo test -p memory-api
```

## 测试覆盖率

| Crate | 测试数 |
|-------|--------|
| memory-core | 36 |
| memory-api | 65 |
| memory-storage | 27 |
| memory-index | 36 |
| memory-lifecycle | 27 |
| memory-train | 20 |
| **总计** | **211** |

## 许可证

MIT