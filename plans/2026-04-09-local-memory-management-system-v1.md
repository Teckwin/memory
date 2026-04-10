# 本地记忆管理系统架构设计方案

## 1. 系统概述与设计目标

### 1.1 项目背景

基于 `data-upload-protocol.md` 协议文档的设计理念，本方案旨在构建一个本地记忆管理系统，用于本地工作区文件的语义搜索和上下文感知。该系统将作为 Forge 生态的本地记忆核心组件，提供高效的记忆存储、检索和自动学习能力。

### 1.2 核心设计目标

| 目标 | 描述 |
|-----|------|
| **本地化部署** | 所有数据存储在本地，不依赖远程服务器 |
| **高效检索** | 支持语义向量搜索和关键词检索 |
| **自动学习** | 边学边练的增量训练能力 |
| **分层存储** | 热数据内存缓存，冷数据磁盘存储，僵尸数据归档 |
| **生命周期管理** | 完善的数据老化机制和自动清理 |
| **API 开放** | 通过 Rust 包方法 API 开放对接 |

### 1.3 技术选型依据

基于协议文档中的技术栈和 Rust 生态系统，推荐以下核心技术：

- **向量存储**: `mev-rs` 或自建向量索引（基于 `hnsw` 算法）
- **持久化存储**: SQLite (符合协议文档中的 `WorkspaceAuth` 存储模式)
- **序列化**: Protocol Buffers (与协议文档一致)
- **异步运行时**: Tokio (与 Forge 现有代码一致)

---

## 2. 系统架构设计

### 2.1 整体架构图

```
┌─────────────────────────────────────────────────────────────────────────┐
│                        Local Memory Management System                   │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│  ┌──────────────────────────────────────────────────────────────────┐  │
│  │                        API Layer (Rust Crate)                    │  │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐              │  │
│  │  │ MemoryAPI   │  │ SearchAPI   │  │ TrainAPI    │              │  │
│  │  └─────────────┘  └─────────────┘  └─────────────┘              │  │
│  └──────────────────────────────────────────────────────────────────┘  │
│                                    │                                    │
│                                    ▼                                    │
│  ┌──────────────────────────────────────────────────────────────────┐  │
│  │                     Core Engine Layer                            │  │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐              │  │
│  │  │ MemoryMgr   │  │ IndexMgr    │  │ TrainEngine │              │  │
│  │  └─────────────┘  └─────────────┘  └─────────────┘              │  │
│  └──────────────────────────────────────────────────────────────────┘  │
│                                    │                                    │
│                                    ▼                                    │
│  ┌──────────────────────────────────────────────────────────────────┐  │
│  │                   Data Lifecycle Layer                           │  │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐              │  │
│  │  │ HotStorage  │──│ ColdStorage │──│ ZombieStore │              │  │
│  │  │ (Memory)    │  │ (Disk)      │  │ (Archive)   │              │  │
│  │  └─────────────┘  └─────────────┘  └─────────────┘              │  │
│  └──────────────────────────────────────────────────────────────────┘  │
│                                                                         │
└─────────────────────────────────────────────────────────────────────────┘
```

### 2.2 模块划分

| 模块 | 职责 | 核心文件 |
|-----|------|---------|
| `memory-core` | 核心数据结构定义和 trait 定义 | `src/core/mod.rs` |
| `memory-storage` | 分层存储引擎实现 | `src/storage/mod.rs` |
| `memory-index` | 向量索引和全文索引管理 | `src/index/mod.rs` |
| `memory-train` | 本地训练引擎 | `src/train/mod.rs` |
| `memory-lifecycle` | 数据生命周期管理 | `src/lifecycle/mod.rs` |
| `memory-api` | 对外 API 接口 | `src/api/mod.rs` |

---

## 3. 核心数据结构设计

### 3.1 MemoryEntry

```rust
/// 记忆条目核心结构
pub struct MemoryEntry {
    pub id: Uuid,                      // 唯一标识
    pub content: String,               // 原始内容
    pub embedding: Option<Vec<f32>>,   // 向量嵌入 (可选)
    pub metadata: MemoryMetadata,      // 元数据
    pub state: MemoryState,            // 当前状态
    pub created_at: DateTime<Utc>,     // 创建时间
    pub updated_at: DateTime<Utc>,     // 更新时间
    pub last_accessed: DateTime<Utc>,  // 最后访问时间
    pub access_count: u64,             // 访问次数
    pub importance: f32,               // 重要性评分 (0.0-1.0)
}

pub struct MemoryMetadata {
    pub source_type: SourceType,       // 来源类型 (File/Code/Doc/User)
    pub source_path: Option<String>,   // 原始文件路径
    pub tags: Vec<String>,             // 标签
    pub language: Option<String>,      // 编程语言
    pub chunk_index: Option<u32>,      // 分块索引 (用于大文件)
    pub total_chunks: Option<u32>,     // 总分块数
}

#[derive(Clone, Debug, PartialEq)]
pub enum MemoryState {
    Hot,      // 热数据：内存中活跃
    Cold,     // 冷数据：磁盘存储
    Zombie,   // 僵尸数据：归档存储
    Deleting, // 待删除
}
```

### 3.2 访问统计与热度计算

```rust
/// 访问统计结构
pub struct AccessStats {
    pub total_accesses: u64,
    pub recent_accesses: Vec<DateTime<Utc>>,
    pub last_access_delta: Duration,
    pub access_frequency: f32,  // 访问频率 (次/小时)
}

/// 热度评分计算
impl MemoryEntry {
    pub fn calculate_heat_score(&self) -> f32 {
        let recency_score = self.calculate_recency_score();
        let frequency_score = self.calculate_frequency_score();
        let importance_score = self.importance;
        
        // 加权计算: 近期性 40%, 频率 30%, 重要性 30%
        recency_score * 0.4 + frequency_score * 0.3 + importance_score * 0.3
    }
    
    fn calculate_recency_score(&self) -> f32 {
        let hours_since_access = (Utc::now() - self.last_accessed).num_hours() as f32;
        // 指数衰减: 24小时内为1.0, 168小时(7天)后接近0
        (-hours_since_access / 48.0).exp()
    }
    
    fn calculate_frequency_score(&self) -> f32 {
        // 基于访问次数的对数评分
        (self.access_count as f32 + 1.0).log10() / 4.0
    }
}
```

### 3.3 索引结构

```rust
/// 向量索引元数据
pub struct VectorIndexMeta {
    pub id: Uuid,
    pub dimension: u32,
    pub entry_count: u64,
    pub created_at: DateTime<Utc>,
    pub last_rebuilt: DateTime<Utc>,
    pub index_type: IndexType,
}

pub enum IndexType {
    Hnsw,        // HNSW 近似最近邻
    Flat,        // 精确检索 (小规模)
    Composite,   // 组合索引
}

/// 全文索引元数据
pub struct FullTextIndexMeta {
    pub id: Uuid,
    pub tokenizer: TokenizerType,
    pub entry_count: u64,
    pub last_rebuilt: DateTime<Utc>,
}
```

---

## 4. 数据分层存储设计

### 4.1 分层架构

```
┌────────────────────────────────────────────────────────────┐
│                        Hot Storage                         │
│  ┌──────────────────────────────────────────────────────┐  │
│  │  LRU Cache (内存)                                     │  │
│  │  - 最大容量: 可配置 (默认 1GB)                        │  │
│  │  - 存储: 完整 MemoryEntry + 嵌入向量                  │  │
│  │  - 淘汰策略: 基于热度评分                            │  │
│  └──────────────────────────────────────────────────────┘  │
│                            │                                │
│                            ▼ 降级                           │
├────────────────────────────────────────────────────────────┤
│                        Cold Storage                        │
│  ┌──────────────────────────────────────────────────────┐  │
│  │  SQLite + 向量索引文件                                │  │
│  │  - 存储: 完整 MemoryEntry                            │  │
│  │  - 嵌入向量单独索引文件                               │  │
│  │  - 索引: 主键索引 + 时间索引 + 热度索引              │  │
│  └──────────────────────────────────────────────────────┘  │
│                            │                                │
│                            ▼ 归档                           │
├────────────────────────────────────────────────────────────┤
│                      Zombie Storage                        │
│  ┌──────────────────────────────────────────────────────┐  │
│  │  归档文件 (MessagePack/JSON 压缩)                    │  │
│  │  - 存储: MemoryEntry (不含向量)                      │  │
│  │  - 格式: 按月/按年分目录                             │  │
│  │  - 压缩: LZ4 或 Zstd                                 │  │
│  └──────────────────────────────────────────────────────┘  │
└────────────────────────────────────────────────────────────┘
```

### 4.2 存储配置

```rust
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct StorageConfig {
    /// 热数据存储配置
    pub hot: HotStorageConfig,
    /// 冷数据存储配置
    pub cold: ColdStorageConfig,
    /// 僵尸数据存储配置
    pub zombie: ZombieStorageConfig,
    /// 生命周期配置
    pub lifecycle: LifecycleConfig,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct HotStorageConfig {
    /// 最大内存使用 (字节)
    pub max_memory_bytes: u64,
    /// 最大条目数
    pub max_entries: u64,
    /// LRU 缓存初始容量
    pub initial_capacity: u64,
    /// 预加载条目数 (启动时从冷存储加载)
    pub preload_count: u64,
}

impl Default for HotStorageConfig {
    fn default() -> Self {
        Self {
            max_memory_bytes: 1024 * 1024 * 1024,  // 1GB
            max_entries: 100_000,
            initial_capacity: 10_000,
            preload_count: 1000,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ColdStorageConfig {
    /// SQLite 数据库路径
    pub db_path: PathBuf,
    /// 向量索引目录
    pub vector_index_dir: PathBuf,
    /// 批量写入大小
    pub batch_size: usize,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ZombieStorageConfig {
    /// 归档目录
    pub archive_dir: PathBuf,
    /// 归档格式
    pub format: ArchiveFormat,
    /// 压缩级别
    pub compression_level: i32,
    /// 每个归档文件最大条目数
    pub max_entries_per_archive: u64,
}
```

### 4.3 存储转换策略

```rust
/// 存储层转换管理器
pub struct StorageTransitionManager {
    hot_store: Arc<HotStorage>,
    cold_store: Arc<ColdStorage>,
    zombie_store: Arc<ZombieStorage>,
    config: StorageConfig,
}

impl StorageTransitionManager {
    /// 热数据降级到冷存储
    pub async fn demote_to_cold(&self, entry_id: Uuid) -> Result<()> {
        // 1. 从热存储获取
        let entry = self.hot_store.get(&entry_id).await?
            .ok_or(Error::EntryNotFound)?;
        
        // 2. 写入冷存储
        self.cold_store.put(entry.clone()).await?;
        
        // 3. 从热存储移除
        self.hot_store.remove(&entry_id).await?;
        
        // 4. 更新状态
        self.update_state(entry_id, MemoryState::Cold).await
    }
    
    /// 冷数据降级到僵尸存储
    pub async fn demote_to_zombie(&self, entry_id: Uuid) -> Result<()> {
        let entry = self.cold_store.get(&entry_id).await?
            .ok_or(Error::EntryNotFound)?;
        
        // 僵尸存储不保留向量数据
        let mut zombie_entry = entry;
        zombie_entry.embedding = None;
        
        self.zombie_store.archive(zombie_entry).await?;
        self.cold_store.remove(&entry_id).await?;
        
        self.update_state(entry_id, MemoryState::Zombie).await
    }
    
    /// 僵尸数据彻底删除
    pub async fn purge_zombie(&self, entry_id: Uuid) -> Result<()> {
        self.zombie_store.delete(&entry_id).await?;
        self.remove_from_index(entry_id).await
    }
}
```

---

## 5. API 设计

### 5.1 核心 API 结构

```rust
/// 记忆管理系统主入口
pub struct MemoryManager {
    storage: Arc<StorageLayer>,
    index: Arc<IndexManager>,
    trainer: Arc<TrainEngine>,
    lifecycle: Arc<LifecycleManager>,
}

impl MemoryManager {
    /// 创建新的记忆管理器实例
    pub fn new(config: MemoryManagerConfig) -> Result<Self>;
    
    /// 启动管理器 (加载索引、恢复状态)
    pub async fn start(&self) -> Result<()>;
    
    /// 停止管理器 (保存状态、关闭存储)
    pub async fn shutdown(&self) -> Result<()>;
}
```

### 5.2 记忆 CRUD API

```rust
/// 记忆管理 API
pub trait MemoryApi: Send + Sync {
    /// 添加新记忆
    async fn add(&self, entry: MemoryEntry) -> Result<MemoryId>;
    
    /// 批量添加记忆
    async fn add_batch(&self, entries: Vec<MemoryEntry>) -> Result<Vec<MemoryId>>;
    
    /// 获取记忆
    async fn get(&self, id: MemoryId) -> Result<Option<MemoryEntry>>;
    
    /// 更新记忆
    async fn update(&self, id: MemoryId, update: MemoryUpdate) -> Result<()>;
    
    /// 删除记忆
    async fn delete(&self, id: MemoryId) -> Result<()>;
    
    /// 批量删除
    async fn delete_batch(&self, ids: Vec<MemoryId>) -> Result<()>;
    
    /// 列出记忆 (支持分页和过滤)
    async fn list(&self, filter: MemoryFilter) -> Result<Vec<MemoryEntry>>;
}

#[derive(Clone, Debug, Default)]
pub struct MemoryFilter {
    pub state: Option<MemoryState>,
    pub source_type: Option<SourceType>,
    pub tags: Option<Vec<String>>,
    pub from_date: Option<DateTime<Utc>>,
    pub to_date: Option<DateTime<Utc>>,
    pub min_importance: Option<f32>,
    pub offset: u64,
    pub limit: u64,
}

pub struct MemoryUpdate {
    pub content: Option<String>,
    pub metadata: Option<MemoryMetadata>,
    pub importance: Option<f32>,
}
```

### 5.3 搜索 API

```rust
/// 搜索 API
pub trait SearchApi: Send + Sync {
    /// 语义向量搜索
    async fn semantic_search(&self, query: &str, options: SearchOptions) 
        -> Result<Vec<SearchResult>>;
    
    /// 全文搜索
    async fn fulltext_search(&self, query: &str, options: SearchOptions) 
        -> Result<Vec<SearchResult>>;
    
    /// 混合搜索 (语义 + 全文)
    async fn hybrid_search(&self, query: &str, options: SearchOptions) 
        -> Result<Vec<SearchResult>>;
    
    /// 精确匹配搜索
    async fn exact_search(&self, query: &str) -> Result<Vec<SearchResult>>;
}

#[derive(Clone, Debug)]
pub struct SearchOptions {
    pub limit: u32,                    // 返回结果数
    pub threshold: Option<f32>,        // 相似度阈值
    pub filters: Option<MemoryFilter>, // 过滤条件
    pub include_vector: bool,          // 是否包含向量数据
    pub rerank: bool,                  // 是否重排序
}

#[derive(Clone, Debug)]
pub struct SearchResult {
    pub entry: MemoryEntry,
    pub score: f32,
    pub rank: u32,
}
```

### 5.4 训练 API

```rust
/// 训练 API
pub trait TrainApi: Send + Sync {
    /// 触发增量训练
    async fn train_incremental(&self, config: TrainConfig) -> Result<TrainResult>;
    
    /// 获取训练状态
    async fn get_training_status(&self) -> Result<TrainingStatus>;
    
    /// 添加训练样本
    async fn add_training_sample(&self, sample: TrainingSample) -> Result<()>;
    
    /// 批量添加训练样本
    async fn add_training_samples(&self, samples: Vec<TrainingSample>) -> Result<()>;
    
    /// 导出训练数据
    async fn export_training_data(&self, path: &Path) -> Result<()>;
    
    /// 导入预训练模型
    async fn import_model(&self, path: &Path) -> Result<()>;
    
    /// 导出微调模型
    async fn export_model(&self, path: &Path) -> Result<()>;
}

#[derive(Clone, Debug)]
pub struct TrainConfig {
    pub epochs: u32,
    pub batch_size: u32,
    pub learning_rate: f32,
    pub data_source: TrainDataSource,
    pub model_type: ModelType,
}

#[derive(Clone, Debug)]
pub enum TrainDataSource {
    All,                    // 使用所有可用数据
    Recent(u64),            // 最近 N 条
    ByImportance(f32),      // 重要性 >= 指定值
    Custom(Vec<MemoryId>),  // 指定记忆 ID
}

#[derive(Clone, Debug)]
pub enum ModelType {
    Embedding,   // 嵌入模型
    Reranker,   // 重排序模型
    Both,       // 两者
}

#[derive(Clone, Debug)]
pub struct TrainingStatus {
    pub is_training: bool,
    pub progress: f32,
    pub current_epoch: u32,
    pub total_epochs: u32,
    pub samples_processed: u64,
    pub loss: Option<f32>,
}
```

### 5.5 管理 API

```rust
/// 管理 API
pub trait AdminApi: Send + Sync {
    /// 重建索引
    async fn rebuild_index(&self, config: RebuildIndexConfig) -> Result<()>;
    
    /// 获取存储统计
    async fn get_storage_stats(&self) -> Result<StorageStats>;
    
    /// 获取生命周期状态
    async fn get_lifecycle_status(&self) -> Result<LifecycleStatus>;
    
    /// 手动触发数据降级
    async fn trigger_demotion(&self, target_state: MemoryState) -> Result<u64>;
    
    /// 手动触发清理
    async fn trigger_cleanup(&self, older_than: Duration) -> Result<u64>;
    
    /// 获取配置
    async fn get_config(&self) -> Result<MemoryManagerConfig>;
    
    /// 更新配置
    async fn update_config(&self, config: MemoryManagerConfig) -> Result<()>;
}

#[derive(Clone, Debug)]
pub struct StorageStats {
    pub hot_entries: u64,
    pub hot_memory_bytes: u64,
    pub cold_entries: u64,
    pub cold_disk_bytes: u64,
    pub zombie_entries: u64,
    pub zombie_disk_bytes: u64,
    pub total_vectors: u64,
}

#[derive(Clone, Debug)]
pub struct RebuildIndexConfig {
    pub index_type: IndexType,
    pub force: bool,
    pub batch_size: Option<usize>,
}
```

---

## 6. 数据生命周期管理

### 6.1 生命周期状态机

```
                    ┌─────────────┐
                    │   Created   │
                    └──────┬──────┘
                           │
                           ▼
                    ┌─────────────┐
          ┌────────│    Hot      │────────┐
          │        └──────┬──────┘        │
          │               │               │
          │               │ 访问          │ 降级条件
          │               ▼               │ 满足
          │        ┌─────────────┐        │
          │        │    Hot      │────────┴──────┐
          │        │ (Active)    │               │
          │        └──────┬──────┘               │
          │               │                      │
          │ 长期不访问    │ 访问频率低           │ 归档条件
          │ (30天)       ▼                       │ 满足
          │        ┌─────────────┐        ┌──────┴──────┐
          │        │    Cold     │────────│   Zombie    │
          │        └──────┬──────┘        │  (Archived) │
          │               │               └──────┬──────┘
          │               │                      │
          │ 恢复访问      │ 归档超过180天        │ 超时清理
          │               ▼                      │ (365天)
          │        ┌─────────────┐        ┌──────┴──────┐
          │        │    Hot      │        │   Purged    │
          └───────▶│ (Restored)  │        │  (Deleted)  │
                   └─────────────┘        └─────────────┘
```

### 6.2 生命周期配置

```rust
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct LifecycleConfig {
    /// 热数据最大存活时间 (小时)
    pub hot_max_age_hours: u64,
    /// 冷数据最大存活时间 (小时)
    pub cold_max_age_hours: u64,
    /// 僵尸数据最大存活时间 (小时)
    pub zombie_max_age_hours: u64,
    /// 热数据降级阈值 (热度评分)
    pub hot_demote_threshold: f32,
    /// 冷数据降级阈值 (热度评分)
    pub cold_demote_threshold: f32,
    /// 触发降级的最小访问间隔 (小时)
    pub min_access_interval_hours: u64,
    /// 自动降级检查间隔 (分钟)
    pub demotion_check_interval_mins: u64,
    /// 自动清理检查间隔 (小时)
    pub cleanup_check_interval_hours: u64,
    /// 每次降级处理的最大条目数
    pub max_demotion_per_batch: u64,
    /// 每次清理的最大条目数
    pub max_cleanup_per_batch: u64,
}

impl Default for LifecycleConfig {
    fn default() -> Self {
        Self {
            hot_max_age_hours: 24 * 7,        // 7天
            cold_max_age_hours: 24 * 30,      // 30天
            zombie_max_age_hours: 24 * 365,   // 365天
            hot_demote_threshold: 0.2,
            cold_demote_threshold: 0.05,
            min_access_interval_hours: 24,
            demotion_check_interval_mins: 15,
            cleanup_check_interval_hours: 1,
            max_demotion_per_batch: 1000,
            max_cleanup_per_batch: 500,
        }
    }
}
```

### 6.3 生命周期管理器

```rust
pub struct LifecycleManager {
    config: LifecycleConfig,
    storage: Arc<StorageLayer>,
    index: Arc<IndexManager>,
    scheduler: Arc<Scheduler>,
}

impl LifecycleManager {
    /// 初始化生命周期管理器
    pub async fn new(config: LifecycleConfig, ...) -> Result<Self>;
    
    /// 启动定时任务
    pub async fn start(&self) -> Result<()>;
    
    /// 停止定时任务
    pub async fn stop(&self) -> Result<()>;
    
    /// 评估并执行数据降级
    pub async fn evaluate_demotions(&self) -> Result<DemotionReport>;
    
    /// 评估并执行数据清理
    pub async fn evaluate_cleanup(&self) -> Result<CleanupReport>;
    
    /// 更新记忆的访问信息
    pub async fn record_access(&self, id: MemoryId) -> Result<()>;
}

pub struct DemotionReport {
    pub hot_to_cold: u64,
    pub cold_to_zombie: u64,
    pub errors: Vec<Error>,
}

pub struct CleanupReport {
    pub purged_count: u64,
    pub freed_bytes: u64,
    pub errors: Vec<Error>,
}
```

---

## 7. 索引管理策略

### 7.1 索引架构

```rust
/// 索引管理器
pub struct IndexManager {
    vector_index: Arc<dyn VectorIndex>,
    fulltext_index: Arc<dyn FullTextIndex>,
    metadata_index: Arc<MetadataIndex>,
    config: IndexConfig,
}

pub struct IndexConfig {
    /// 向量维度
    pub vector_dimension: u32,
    /// 向量索引类型
    pub vector_index_type: IndexType,
    /// HNSW 参数
    pub hnsw_params: HnswParams,
    /// 全文索引类型
    pub fulltext_index_type: FullTextIndexType,
    /// 自动重建间隔 (小时)
    pub auto_rebuild_interval_hours: u64,
    /// 增量索引更新批次大小
    pub incremental_batch_size: usize,
    /// 重建阈值 (条目变化百分比)
    pub rebuild_threshold_percent: f32,
}

#[derive(Clone, Debug)]
pub struct HnswParams {
    pub ef_construction: u32,
    pub m: u32,
    pub ef_search: u32,
}

impl Default for HnswParams {
    fn default() -> Self {
        Self {
            ef_construction: 200,
            m: 16,
            ef_search: 50,
        }
    }
}
```

### 7.2 索引操作

```rust
impl IndexManager {
    /// 添加到向量索引
    pub async fn add_to_vector_index(&self, id: MemoryId, vector: &[f32]) -> Result<()>;
    
    /// 批量添加到向量索引
    pub async fn add_batch_to_vector_index(&self, entries: Vec<(MemoryId, Vec<f32>)>) -> Result<()>;
    
    /// 从向量索引删除
    pub async fn remove_from_vector_index(&self, id: &MemoryId) -> Result<()>;
    
    /// 向量搜索
    pub async fn vector_search(&self, query: &[f32], limit: u32) -> Result<Vec<(MemoryId, f32)>>;
    
    /// 添加到全文索引
    pub async fn add_to_fulltext_index(&self, id: MemoryId, content: &str) -> Result<()>;
    
    /// 全文搜索
    pub async fn fulltext_search(&self, query: &str, limit: u32) -> Result<Vec<(MemoryId, f32)>>;
    
    /// 重建向量索引
    pub async fn rebuild_vector_index(&self, config: RebuildConfig) -> Result<()>;
    
    /// 重建全文索引
    pub async fn rebuild_fulltext_index(&self, config: RebuildConfig) -> Result<()>;
    
    /// 获取索引统计
    pub async fn get_index_stats(&self) -> Result<IndexStats>;
}
```

### 7.3 定时索引重建

```rust
/// 索引重建调度器
pub struct IndexRebuildScheduler {
    index_manager: Arc<IndexManager>,
    config: IndexConfig,
    timer: Interval,
}

impl IndexRebuildScheduler {
    /// 定时检查是否需要重建
    pub async fn check_and_rebuild(&self) -> Result<RebuildDecision> {
        let stats = self.index_manager.get_index_stats().await?;
        
        // 检查是否达到重建条件
        let needs_rebuild = self.should_rebuild(&stats).await;
        
        if needs_rebuild {
            // 增量重建
            if self.can_incremental_rebuild(&stats) {
                self.index_manager.incremental_rebuild().await?;
                return Ok(RebuildDecision::Incremental);
            } else {
                // 全量重建
                self.index_manager.full_rebuild().await?;
                return Ok(RebuildDecision::Full);
            }
        }
        
        Ok(RebuildDecision::None)
    }
    
    fn should_rebuild(&self, stats: &IndexStats) -> bool {
        let change_percent = (stats.changed_entries as f32 / stats.total_entries as f32) * 100.0;
        change_percent > self.config.rebuild_threshold_percent
    }
}
```

---

## 8. 训练模型集成方案

### 8.1 训练引擎架构

```
┌─────────────────────────────────────────────────────────────────┐
│                      Train Engine                               │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  ┌─────────────┐    ┌─────────────┐    ┌─────────────┐        │
│  │ DataLoader  │───▶│  Trainer    │───▶│  Model      │        │
│  │             │    │             │    │  Manager    │        │
│  └─────────────┘    └─────────────┘    └─────────────┘        │
│         │                  │                   │               │
│         ▼                  ▼                   ▼               │
│  ┌─────────────┐    ┌─────────────┐    ┌─────────────┐        │
│  │ Sample      │    │ Loss        │    │ Checkpoint  │        │
│  │ Augmentation│    │ Compute     │    │ Manager     │        │
│  └─────────────┘    └─────────────┘    └─────────────┘        │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

### 8.2 训练数据准备

```rust
/// 训练数据准备器
pub struct TrainingDataPreparer {
    storage: Arc<StorageLayer>,
    config: TrainDataConfig,
}

impl TrainingDataPreparer {
    /// 准备训练数据
    pub async fn prepare(&self, config: &TrainDataSource) -> Result<TrainingDataset> {
        match config {
            TrainDataSource::All => self.prepare_all().await,
            TrainDataSource::Recent(n) => self.prepare_recent(*n).await,
            TrainDataSource::ByImportance(threshold) => {
                self.prepare_by_importance(*threshold).await
            }
            TrainDataSource::Custom(ids) => self.prepare_custom(ids).await,
        }
    }
    
    /// 数据增强
    pub async fn augment(&self, samples: Vec<TrainingSample>) -> Vec<TrainingSample> {
        let mut augmented = Vec::with_capacity(samples.len() * 3);
        
        for sample in samples {
            // 原始样本
            augmented.push(sample.clone());
            
            // 同义词替换
            if let Some(replaced) = self.synonym_replace(&sample) {
                augmented.push(replaced);
            }
            
            // 随机删除
            if let Some(deleted) = self.random_delete(&sample) {
                augmented.push(deleted);
            }
            
            // 随机交换
            if let Some(swapped) = self.random_swap(&sample) {
                augmented.push(swapped);
            }
        }
        
        augmented
    }
}

#[derive(Clone, Debug)]
pub struct TrainingSample {
    pub input: String,
    pub output: String,
    pub sample_type: SampleType,
    pub metadata: TrainingSampleMetadata,
}

#[derive(Clone, Debug)]
pub enum SampleType {
    Contrastive,    // 对比学习样本
    Classification, // 分类样本
    Generation,     // 生成样本
}
```

### 8.3 本地训练执行

```rust
/// 训练引擎
pub struct TrainEngine {
    model_manager: Arc<ModelManager>,
    data_preparer: Arc<TrainingDataPreparer>,
    config: TrainEngineConfig,
    status: Arc<RwLock<TrainingStatus>>,
}

impl TrainEngine {
    /// 执行增量训练
    pub async fn train_incremental(&self, config: TrainConfig) -> Result<TrainResult> {
        // 1. 准备训练数据
        let dataset = self.data_preparer.prepare(&config.data_source).await?;
        
        // 2. 数据增强
        let augmented = self.data_preparer.augment(dataset.samples).await;
        
        // 3. 数据分批
        let batches = self.create_batches(augmented, config.batch_size);
        
        // 4. 训练循环
        let mut total_loss = 0.0;
        for epoch in 0..config.epochs {
            self.update_status(|s| {
                s.current_epoch = epoch;
                s.is_training = true;
            }).await;
            
            for (batch_idx, batch) in batches.iter().enumerate() {
                // 前向传播
                let loss = self.model_manager.forward(batch).await?;
                
                // 反向传播
                self.model_manager.backward(loss).await?;
                
                // 更新状态
                self.update_status(|s| {
                    s.samples_processed += batch.len() as u64;
                    s.loss = Some(loss);
                }).await;
                
                // 定期保存检查点
                if batch_idx % 100 == 0 {
                    self.model_manager.save_checkpoint().await?;
                }
            }
            
            total_loss /= config.epochs as f32;
        }
        
        // 5. 保存最终模型
        self.model_manager.save_final_model().await?;
        
        // 6. 更新状态
        self.update_status(|s| {
            s.is_training = false;
            s.progress = 1.0;
        }).await;
        
        Ok(TrainResult {
            final_loss: total_loss,
            epochs_completed: config.epochs,
            samples_used: dataset.samples.len() as u64,
        })
    }
    
    /// 边学边练模式 (在线学习)
    pub async fn learn_on_the_go(&self, sample: TrainingSample) -> Result<()> {
        // 快速单样本训练
        let batch = vec![sample];
        let loss = self.model_manager.forward(&batch).await?;
        self.model_manager.backward(loss).await?;
        
        // 更新模型
        self.model_manager.apply_updates().await?;
        
        Ok(())
    }
}
```

### 8.4 模型管理

```rust
/// 模型管理器
pub struct ModelManager {
    embedding_model: Arc<RwLock<Option<EmbeddingModel>>>,
    reranker_model: Arc<RwLock<Option<RerankerModel>>>,
    checkpoint_dir: PathBuf,
    model_cache: Arc<ModelCache>,
}

impl ModelManager {
    /// 加载预训练模型
    pub async fn load_pretrained(&self, model_path: &Path) -> Result<()>;
    
    /// 导出微调模型
    pub async fn export_finetuned(&self, output_path: &Path) -> Result<()>;
    
    /// 生成嵌入向量
    pub async fn generate_embedding(&self, text: &str) -> Result<Vec<f32>>;
    
    /// 批量生成嵌入向量
    pub async fn generate_embeddings(&self, texts: &[String]) -> Result<Vec<Vec<f32>>>;
    
    /// 重排序
    pub async fn rerank(&self, query: &str, candidates: &[MemoryEntry]) -> Result<Vec<ScoredEntry>>;
}
```

---

## 9. 关键流程设计

### 9.1 数据写入流程

```
┌─────────────────────────────────────────────────────────────────┐
│                     Data Write Flow                              │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  User ──▶ API.add()                                            │
│           │                                                     │
│           ▼                                                     │
│  ┌─────────────────────────────────────────┐                   │
│  │ 1. 验证输入 (Validation)                 │                   │
│  │    - 内容非空                            │                   │
│  │    - 元数据有效                          │                   │
│  └──────────────────┬──────────────────────┘                   │
│                     │                                            │
│                     ▼                                            │
│  ┌─────────────────────────────────────────┐                   │
│  │ 2. 生成 ID 和嵌入向量                    │                   │
│  │    - UUID 生成                           │                   │
│  │    - 调用 embedding model                │                   │
│  └──────────────────┬──────────────────────┘                   │
│                     │                                            │
│                     ▼                                            │
│  ┌─────────────────────────────────────────┐                   │
│  │ 3. 存储到热存储 (Hot Storage)            │                   │
│  │    - 写入 LRU cache                      │                   │
│  │    - 更新内存统计                        │                   │
│  └──────────────────┬──────────────────────┘                   │
│                     │                                            │
│                     ▼                                            │
│  ┌─────────────────────────────────────────┐                   │
│  │ 4. 索引 (Indexing)                       │                   │
│  │    - 添加到向量索引                      │                   │
│  │    - 添加到全文索引                      │                   │
│  │    - 更新元数据索引                      │                   │
│  └──────────────────┬──────────────────────┘                   │
│                     │                                            │
│                     ▼                                            │
│  ┌─────────────────────────────────────────┐                   │
│  │ 5. 异步持久化 (Async Persist)            │                   │
│  │    - 写入冷存储 (SQLite)                 │                   │
│  │    - 保存向量索引文件                    │                   │
│  └──────────────────┬──────────────────────┘                   │
│                     │                                            │
│                     ▼                                            │
│  ┌─────────────────────────────────────────┐                   │
│  │ 6. 返回结果                              │                   │
│  │    - MemoryId                            │                   │
│  │    - 创建时间戳                          │                   │
│  └─────────────────────────────────────────┘                   │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

### 9.2 数据检索流程

```
┌─────────────────────────────────────────────────────────────────┐
│                     Data Search Flow                             │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  User ──▶ API.semantic_search(query)                           │
│           │                                                     │
│           ▼                                                     │
│  ┌─────────────────────────────────────────┐                   │
│  │ 1. 文本预处理                           │                   │
│  │    - 分词                                │                   │
│  │    - 标准化                              │                   │
│  └──────────────────┬──────────────────────┘                   │
│                     │                                            │
│                     ▼                                            │
│  ┌─────────────────────────────────────────┐                   │
│  │ 2. 生成查询向量                         │                   │
│  │    - 调用 embedding model               │                   │
│  └──────────────────┬──────────────────────┘                   │
│                     │                                            │
│                     ▼                                            │
│  ┌─────────────────────────────────────────┐                   │
│  │ 3. 并行搜索 (Parallel)                   │                   │
│  │    ┌─────────┐  ┌─────────┐  ┌────────┐ │                   │
│  │    │Vector   │  │Fulltext │  │Exact   │ │                   │
│  │    │Search   │  │Search   │  │Match   │ │                   │
│  │    └────┬────┘  └────┬────┘  └───┬────┘ │                   │
│  │         │            │           │      │                   │
│  │         └────────────┼───────────┘      │                   │
│  │                      ▼                  │                   │
│  │              ┌─────────────┐            │                   │
│  │              │Result Merge │            │                   │
│  │              │& Rerank     │            │                   │
│  │              └──────┬──────┘            │                   │
│  └─────────────────────┼───────────────────┘                   │
│                        │                                         │
│                        ▼                                         │
│  ┌─────────────────────────────────────────┐                   │
│  │ 4. 结果组装                              │                   │
│  │    - 加载完整 MemoryEntry               │                   │
│  │    - 热存储优先，否则从冷存储加载       │                   │
│  │    - 记录访问 (更新热度)                │                   │
│  └──────────────────┬──────────────────────┘                   │
│                     │                                            │
│                     ▼                                            │
│  ┌─────────────────────────────────────────┐                   │
│  │ 5. 返回结果                              │                   │
│  └─────────────────────────────────────────┘                   │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

### 9.3 生命周期管理流程

```
┌─────────────────────────────────────────────────────────────────┐
│                  Lifecycle Management Flow                       │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  Timer ──▶ LifecycleManager.check()                            │
│            │                                                    │
│            ▼                                                    │
│  ┌─────────────────────────────────────────┐                   │
│  │ 1. 评估降级 (Evaluation)                 │                   │
│  │    - 遍历所有热数据                      │                   │
│  │    - 计算热度评分                        │                   │
│  │    - 识别需要降级的条目                  │                   │
│  └──────────────────┬──────────────────────┘                   │
│                     │                                            │
│                     ▼                                            │
│  ┌─────────────────────────────────────────┐                   │
│  │ 2. 执行降级 (Demotion)                   │                   │
│  │    ┌──────────────┐  ┌───────────────┐  │                   │
│  │    │Hot → Cold    │  │Cold → Zombie  │  │                   │
│  │    │- 写入磁盘    │  │- 归档到文件   │  │                   │
│  │    │- 释放内存   │  │- 删除向量数据 │  │                   │
│  │    │- 更新索引   │  │- 更新索引     │  │                   │
│  │    └──────────────┘  └───────────────┘  │                   │
│  └──────────────────┬──────────────────────┘                   │
│                     │                                            │
│                     ▼                                            │
│  ┌─────────────────────────────────────────┐                   │
│  │ 3. 评估清理 (Cleanup)                    │                   │
│  │    - 检查僵尸数据存活时间                │                   │
│  │    - 识别需要清理的条目                  │                   │
│  └──────────────────┬──────────────────────┘                   │
│                     │                                            │
│                     ▼                                            │
│  ┌─────────────────────────────────────────┐                   │
│  │ 4. 执行清理 (Purge)                      │                   │
│  │    - 删除归档文件中的记录                │                   │
│  │    - 更新元数据                          │                   │
│  │    - 释放磁盘空间                        │                   │
│  └──────────────────┬──────────────────────┘                   │
│                     │                                            │
│                     ▼                                            │
│  ┌─────────────────────────────────────────┐                   │
│  │ 5. 报告 (Report)                         │                   │
│  │    - 记录操作统计                        │                   │
│  │    - 发送通知 (可选)                     │                   │
│  └─────────────────────────────────────────┘                   │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

---

## 10. 错误处理与容错

### 10.1 错误类型定义

```rust
#[derive(Error, Debug)]
pub enum MemoryError {
    #[error("Entry not found: {0}")]
    EntryNotFound(Uuid),
    
    #[error("Storage error: {0}")]
    StorageError(String),
    
    #[error("Index error: {0}")]
    IndexError(String),
    
    #[error("Model error: {0}")]
    ModelError(String),
    
    #[error("Configuration error: {0}")]
    ConfigError(String),
    
    #[error("Lifecycle error: {0}")]
    LifecycleError(String),
    
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    
    #[error("Serialization error: {0}")]
    SerializationError(String),
}
```

### 10.2 容错策略

| 场景 | 策略 |
|-----|------|
| 写入失败 | 重试 3 次，失败后写入错误队列 |
| 索引失败 | 标记条目为"索引待重建"，后台重试 |
| 模型推理失败 | 使用备选模型或返回空结果 |
| 磁盘空间不足 | 触发紧急清理，警告用户 |
| 内存不足 | 强制降级热数据到冷存储 |

---

## 11. 配置示例

```rust
// memory_manager.yaml
memory_manager:
  # 存储配置
  storage:
    hot:
      max_memory_bytes: 1073741824  # 1GB
      max_entries: 100000
      preload_count: 1000
    
    cold:
      db_path: "./data/memory.db"
      vector_index_dir: "./data/vectors"
      batch_size: 100
    
    zombie:
      archive_dir: "./data/archive"
      format: "msgpack"
      compression_level: 3
      max_entries_per_archive: 10000
  
  # 生命周期配置
  lifecycle:
    hot_max_age_hours: 168        # 7天
    cold_max_age_hours: 720       # 30天
    zombie_max_age_hours: 8760    # 365天
    hot_demote_threshold: 0.2
    cold_demote_threshold: 0.05
    demotion_check_interval_mins: 15
    cleanup_check_interval_hours: 1
  
  # 索引配置
  index:
    vector_dimension: 384
    vector_index_type: "hnsw"
    hnsw_params:
      ef_construction: 200
      m: 16
      ef_search: 50
    auto_rebuild_interval_hours: 24
    rebuild_threshold_percent: 20.0
  
  # 训练配置
  train:
    model_type: "embedding"
    default_epochs: 10
    default_batch_size: 32
    learning_rate: 0.001
    checkpoint_interval: 100
```

---

## 12. 模块依赖关系

```
┌────────────────────────────────────────────────────────────────────────┐
│                           Module Dependencies                           │
├────────────────────────────────────────────────────────────────────────┤
│                                                                        │
│  memory-api (顶层 API)                                                 │
│       │                                                               │
│       ├──▶ memory-core (核心类型和 Trait)                              │
│       │       │                                                        │
│       │       ├──▶ memory-storage (存储层)                            │
│       │       │       │                                                │
│       │       │       ├──▶ hot (内存存储)                             │
│       │       │       ├──▶ cold (磁盘存储)                            │
│       │       │       └──▶ zombie (归档存储)                          │
│       │       │                                                        │
│       │       ├──▶ memory-index (索引层)                              │
│       │       │       │                                                │
│       │       │       ├──▶ vector-index (向量索引)                    │
│       │       │       ├──▶ fulltext-index (全文索引)                  │
│       │       │       └──▶ metadata-index (元数据索引)                │
│       │       │                                                        │
│       │       └──▶ memory-lifecycle (生命周期)                        │
│       │               │                                                │
│       │               ├──▶ scheduler (调度器)                         │
│       │               └──▶ policy (策略)                              │
│       │                                                                │
│       └──▶ memory-train (训练层)                                      │
│               │                                                        │
│               ├──▶ data-preparer (数据准备)                           │
│               ├──▶ trainer (训练器)                                   │
│               └──▶ model-manager (模型管理)                           │
│                                                                        │
└────────────────────────────────────────────────────────────────────────┘
```

---

## 13. 总结

本方案提供了一个完整的本地记忆管理系统架构设计，涵盖了:

1. **分层存储**: 热数据内存缓存 + 冷数据磁盘存储 + 僵尸数据归档
2. **生命周期管理**: 自动降级和清理机制，基于热度评分
3. **索引管理**: 向量索引 + 全文索引，支持定时重建
4. **训练能力**: 增量训练和边学边练模式
5. **API 设计**: 清晰的 Rust 包方法 API 接口
6. **容错机制**: 完善的错误处理和重试策略

该架构设计遵循协议文档中的技术栈和设计理念，可以与 Forge 生态无缝集成。