# Coding Standards

## 1. Code Formatting

### Rust Formatting Rules
- Use `cargo fmt` for automatic formatting
- 4 spaces for indentation (no tabs)
- Maximum line length: 100 characters
- Use trailing commas in multi-line expressions

```rust
// Good
fn example_function(
    param1: Type1,
    param2: Type2,
) -> Result<Type3, Error> {
    // ...
}

// Avoid
fn example_function(param1: Type1, param2: Type2) -> Result<Type3, Error> {
    // ...
}
```

## 2. Naming Conventions

| Type | Convention | Example |
|------|------------|---------|
| Variables | snake_case | `let memory_id = ...` |
| Functions | snake_case | `fn get_memory(...)` |
| Structs | PascalCase | `struct MemoryEntry` |
| Enums | PascalCase | `enum MemoryStatus` |
| Enum Variants | PascalCase | `Active, Cooling, Cold` |
| Constants | SCREAMING_SNAKE_CASE | `const MAX_SIZE: usize = 1000` |
| Modules | snake_case | `mod memory_storage` |

## 3. Documentation

### Public API Documentation
All public items must have documentation:

```rust
/// Represents a memory entry in the system.
///
/// # Fields
/// - `id`: Unique identifier for the memory
/// - `workspace_id`: ID of the workspace containing this memory
/// - `content`: The actual memory content
///
/// # Example
/// ```
/// use memory_core::{MemoryEntry, MemoryContent, MemoryMetadata, MemorySource};
///
/// let entry = MemoryEntry::new(
///     workspace_id,
///     MemoryContent::Text("Hello".to_string()),
///     MemoryMetadata::new(MemorySource::UserQuery { query: "test".to_string() }),
/// );
/// ```
pub struct MemoryEntry { ... }
```

### Module Documentation
Use `//!` for module-level docs:

```rust
//! Memory storage module.
//!
//! Provides tiered storage for memories: hot, cold, and zombie.
//!
//! # Architecture
//! - Hot: In-memory cache for active memories
//! - Cold: SQLite for cooling/cold memories
//! - Zombie: File-based archive
```

## 4. Error Handling

### Use Result for Error Propagation
```rust
// Good
fn get_memory(&self, id: MemoryId) -> Result<MemoryEntry, MemoryError> {
    self.storage
        .get(id)
        .ok_or(MemoryError::NotFound(id.to_string()))
}

// Avoid
fn get_memory(&self, id: MemoryId) -> MemoryEntry {
    self.storage.get(id).unwrap() // Don't use unwrap in production
}
```

### Custom Error Types
```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum MemoryError {
    #[error("Memory not found: {0}")]
    NotFound(String),

    #[error("Storage error: {0}")]
    StorageError(String),

    #[error("Invalid operation: {0}")]
    InvalidOperation(String),
}
```

## 5. Testing

### Test Organization
```rust
#[cfg(test)]
mod tests {
    use super::*;

    // Unit tests
    #[test]
    fn test_memory_entry_new() {
        // ...
    }

    #[test]
    fn test_memory_entry_increment_access() {
        // ...
    }

    // Integration tests
    #[tokio::test]
    async fn test_storage_operations() {
        // ...
    }
}
```

### Test Naming Convention
- `test_<function_name>_<scenario>`
- `test_<struct_name>_<behavior>`

```rust
#[test]
fn test_memory_entry_new_creates_valid_id() { ... }

#[test]
fn test_hot_storage_eviction_when_full() { ... }
```

## 6. Imports and Dependencies

### Use Glob Imports Sparingly
```rust
// Good - specific imports
use memory_core::{MemoryEntry, MemoryId, WorkspaceId};

// Avoid - unless re-exporting
use memory_core::*; // Only in lib.rs
```

### Group Imports
```rust
// Standard library
use std::collections::HashMap;
use std::sync::Arc;

// External crates
use async_trait::async_trait;
use tokio::sync::RwLock;

// Internal modules
use crate::error::MemoryError;
use crate::types::*;
```

## 7. Async/Await

### Use async_trait for Trait Definitions
```rust
use async_trait::async_trait;

#[async_trait]
pub trait MemoryApi: Send + Sync {
    async fn get(&self, id: MemoryId) -> Result<MemoryEntry, MemoryError>;
}
```

### Avoid Blocking in Async Context
```rust
// Good - use spawn_blocking for CPU-intensive operations
let result = tokio::task::spawn_blocking(move || {
    blocking_io_operation()
}).await?;

// For database operations, use async drivers
```

## 8. Performance Considerations

### Use Appropriate Data Structures
- `HashMap` for O(1) lookups
- `BTreeMap` for ordered data
- `Vec` for sequential access

### Lazy Initialization
```rust
// Good - initialize on demand
fn get_index(&self) -> &VectorIndex {
    self.index.get_or_init(|| VectorIndex::new())
}
```

## 9. Security

### No Hardcoded Secrets
```rust
// Bad
let api_key = "sk-1234567890abcdef";

// Good - use environment variables
let api_key = std::env::var("API_KEY")
    .expect("API_KEY must be set");
```

### Input Validation
```rust
fn create_workspace(&self, name: String) -> Result<Workspace, MemoryError> {
    if name.is_empty() {
        return Err(MemoryError::InvalidOperation("Name cannot be empty".into()));
    }
    if name.len() > 255 {
        return Err(MemoryError::InvalidOperation("Name too long".into()));
    }
    // ...
}
```

## 10. Git Commit Messages

### Format
```
<type>(<scope>): <subject>

<body>

<footer>
```

### Types
- `feat`: New feature
- `fix`: Bug fix
- `docs`: Documentation
- `style`: Formatting
- `refactor`: Code refactoring
- `test`: Adding tests
- `chore`: Maintenance

### Example
```
feat(memory-storage): implement hot storage eviction policy

Add LRU eviction when hot storage exceeds max_entries.
Fixes #123

Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>
```

## Pre-commit Checklist

Run before committing:
```bash
cargo fmt
cargo clippy --workspace -- -D warnings
cargo test --workspace
cargo doc --workspace --no-deps --document-private-items
```