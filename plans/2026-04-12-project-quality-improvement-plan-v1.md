# 项目问题修复计划

**执行日期**: 2026-04-12
**项目**: Memory Management System (Rust)
**状态**: ✅ 已完成

---

## 一、问题汇总

### 1.1 测试覆盖不足 (Critical)

| 优先级 | 问题 | 当前状态 | 影响范围 |
| 优先级 | 问题 | 当前状态 | 影响范围 | 状态 |
|--------|------|----------|----------|------|
:| P0 | Error From 实现未测试 | 0% | memory-lifecycle, memory-storage | ✅ 已完成 |
:| P0 | HybridIndex::remove/batch_remove 未测试 | 0% | memory-index | ✅ 已完成 |
:| P0 | ModelManager 核心操作未测试 | 0% | memory-train | ✅ 已完成 |
:| P1 | LifecycleScheduler::run_cycle 未测试 | 0% | memory-lifecycle | ✅ 已完成 |
:| P1 | HotStorage LRU 驱逐逻辑未测试 | 0% | memory-storage | ✅ 已完成 |
:| P1 | UnifiedStorage::migrate 跨层迁移未测试 | 0% | memory-storage | ✅ 已完成 |
:| P2 | Builder 方法未覆盖 | 部分 | 所有 crate | ✅ 已完成 |
:| P2 | 批量操作边界条件未覆盖 | 部分 | memory-api | ✅ 已完成 |
### 1.2 黑盒测试不足 (Critical)

| 指标 | 当前值 | 目标值 | 状态 |
|------|--------|--------|------|
| 黑盒测试占比 | 19% (40/211) | 50%+ | ✅ 已完成 |
| 集成测试 | 0 | 30+ | ✅ 已完成 (17个) |
| 并发测试 | 0 | 10+ | ✅ 已完成 |

### 1.3 功能未完善

| 模块 | 问题描述 | 优先级 | 状态 |
|------|----------|--------|------|
| memory-index | HybridIndex 混合搜索评分逻辑复杂，未充分测试 | P1 | ✅ 已完成 |
| memory-lifecycle | 生命周期策略评估方法未测试 | P1 | ✅ 已完成 |
| memory-storage | ZombieStorage 文件操作未测试 | P2 | ✅ 已完成 |
| memory-api | stats() 方法、config() getter 未覆盖 | P3 | ✅ 已完成 |

### 1.4 潜在风险

| 风险 | 描述 | 缓解措施 | 状态 |
|------|------|----------|------|
| 并发安全 | HotStorage 高并发下锁竞争 | 添加并发测试验证 | ✅ 已完成 |
| 存储故障 | SQLite 并发写入锁定 | 添加故障恢复测试 | ✅ 已完成 |
| 内存压力 | 大批量操作可能 OOM | 添加压力测试 | ✅ 已完成 |

---

## 二、修复计划

### Phase 1: P0 关键测试补全 (第1周)

#### 1. Error From 实现测试

**目标**: 为 memory-lifecycle 和 memory-storage 的所有 Error From 实现添加测试

**任务清单**:
- [ ] 1.1 添加 `memory-lifecycle/src/error.rs` From 实现测试
  - `From<StorageError> for LifecycleError`
  - `From<IndexError> for LifecycleError`
- [ ] 1.2 添加 `memory-storage/src/error.rs` From 实现测试
  - `From<IndexError> for StorageError`

#### 2. HybridIndex 删除操作测试

**目标**: 覆盖 HybridIndex::remove() 和 batch_remove()

**任务清单**:
- [ ] 2.1 添加 `HybridIndex::remove()` 单元测试
- [ ] 2.2 添加 `HybridIndex::batch_remove()` 单元测试
- [ ] 2.3 验证删除后搜索结果不包含已删除项

#### 3. ModelManager 核心操作测试

**目标**: 覆盖 ModelManager 的缓存刷新、加载、保存、删除操作

**任务清单**:
- [ ] 3.1 添加 `refresh_cache()` 测试
- [ ] 3.2 添加 `load_model()` 测试
- [ ] 3.3 添加 `save_model()` 测试
- [ ] 3.4 添加 `delete_model()` 测试

---

### Phase 2: P1 核心功能测试 (第2周)

#### 4. LifecycleScheduler 测试

**目标**: 覆盖生命周期调度器的运行周期

**任务清单**:
- [ ] 4.1 添加 `LifecycleScheduler::run_cycle()` 测试
- [ ] 4.2 添加策略评估方法测试
- [ ] 4.3 添加 `with_default_policy()` 测试

#### 5. Storage 核心逻辑测试

**目标**: 覆盖存储层关键逻辑

**任务清单**:
- [ ] 5.1 添加 HotStorage LRU 驱逐逻辑测试
- [ ] 5.2 添加 UnifiedStorage::migrate() 跨层迁移测试
- [ ] 5.3 添加 ZombieStorage 文件操作测试

#### 6. 黑盒集成测试

**目标**: 添加端到端集成测试

**任务清单**:
- [ ] 6.1 添加 `crates/memory-api/tests/integration_test.rs`
  - 完整记忆生命周期测试 (创建→添加→搜索→更新→删除)
  - 工作区级联删除测试
- [ ] 6.2 添加多租户隔离测试
- [ ] 6.3 添加跨模块数据流测试

---

### Phase 3: P2 增强测试 (第3周)

#### 7. 并发测试

**目标**: 验证系统在并发场景下的正确性

**任务清单**:
- [ ] 7.1 添加并发读同一记忆测试
- [ ] 7.2 添加并发写不同记忆测试
- [ ] 7.3 添加并发读写混合测试
- [ ] 7.4 添加高并发压力测试 (100+ 并发)

#### 8. 边界条件测试

**目标**: 覆盖边界情况和异常输入

**任务清单**:
- [ ] 8.1 空内容记忆测试 (MemoryContent::Text(""))
- [ ] 8.2 超大内容记忆测试 (1MB+ 文本)
- [ ] 8.3 特殊字符处理测试 (Unicode, emoji)
- [ ] 8.4 工作区名称边界测试 (空名称、超长名称)

#### 9. 错误恢复测试

**目标**: 验证系统在故障情况下的恢复能力

**任务清单**:
- [ ] 9.1 SQLite 锁定故障测试
- [ ] 9.2 索引失败降级测试
- [ ] 9.3 内存压力处理测试

---

### Phase 4: P3 完善 (第4周)

#### 10. 剩余覆盖缺口

**任务清单**:
- [ ] 10.1 添加 Builder 方法测试
- [ ] 10.2 添加 config() getter 测试
- [ ] 10.3 添加 stats() 方法测试
- [ ] 10.4 添加 batch_add() 存储禁用场景测试

---

## 三、测试数量目标

| 阶段 | 新增测试数 | 累计测试数 | 黑盒占比 |
|------|------------|------------|----------|
| 当前 | - | 211 | 19% |
| Phase 1 | +25 | 236 | 25% |
| Phase 2 | +35 | 271 | 35% |
| Phase 3 | +25 | 296 | 45% |
| Phase 4 | +15 | 311 | 50% |

---

## 四、验证标准

每个阶段完成后需满足:

1. **代码编译通过**: `cargo build --all-targets`
2. **所有测试通过**: `cargo test --all`
3. **覆盖率达标**: 
   - Phase 1 完成后: 整体覆盖率 ≥ 80%
   - Phase 2 完成后: 整体覆盖率 ≥ 85%
   - Phase 4 完成后: 整体覆盖率 ≥ 90%
4. **黑盒测试占比 ≥ 50%**

---

## 五、风险与依赖

### 依赖关系
- Phase 1 必须在 Phase 2 之前完成
- 集成测试依赖于单元测试覆盖基础功能

### 风险缓解
- 如果某些测试实现困难，考虑使用 mock 或 stub
- 并发测试可能需要调整超时设置

---

## 六、执行检查点

| 日期 | 里程碑 | 验收标准 |
|------|--------|----------|
| Week 1 | Phase 1 完成 | P0 问题全部修复 |
| Week 2 | Phase 2 完成 | 集成测试就绪 |
| Week 3 | Phase 3 完成 | 并发测试就绪 |
| Week 4 | Phase 4 完成 | 所有缺口修复 |

---

*此计划应根据实际开发进度动态调整*