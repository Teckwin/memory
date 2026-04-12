# Code Coverage Gap Report

## Overall Coverage: ~75% (varies by crate)

---

## Crate-by-Crate Analysis

### 1. memory-api (84% covered)
**Uncovered:**
- Lines 55-56: `config()` getter
- Line 169: Text search filtering in `list()`
- Line 176: Tags filtering in `list()`
- Lines 314-317: `batch_add()` with storage disabled
- Line 360: `stats()` method

### 2. memory-core (100% covered) ✅

### 3. memory-index (79% covered)
**Critical gaps:**
- `HybridIndex::remove()` and `batch_remove()`
- Complex search scoring logic
- Metadata filtering by date_range
- Error `From` implementations

### 4. memory-lifecycle (51% covered)
**Critical gaps:**
- ALL error conversions (0% tested)
- `LifecycleManager::with_default_policy()`
- `LifecycleScheduler::run_cycle()`
- Policy evaluation methods

### 5. memory-storage (76% covered)
**Critical gaps:**
- ALL error conversions (0% tested)
- `HotStorage` LRU eviction logic
- Cross-tier migration in `UnifiedStorage::migrate()`
- `ZombieStorage` file operations

### 6. memory-train (57% covered)
**Critical gaps:**
- `ModelManager` core operations (refresh_cache, load/save/delete)
- `TrainService` trait implementations
- Text extraction in DataPreparer

---

## Priority Test Additions Needed

### P0 - Error Handling (must fix)
1. `memory-lifecycle/src/error.rs` - All From implementations
2. `memory-storage/src/error.rs` - All From implementations

### P1 - Core Functionality
3. `HybridIndex::remove()` and `batch_remove()`
4. `ModelManager` core operations
5. `LifecycleScheduler::run_cycle()`

### P2 - Storage Logic
6. `HotStorage` LRU eviction
7. `UnifiedStorage::migrate()`

### P3 - Edge Cases
8. Builder methods, batch operations, config getters