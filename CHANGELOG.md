# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - 2026-04-12

### Added
- Complete memory management system implementation (renamed to memos)
  - memos-core: Core types and traits
  - memos-api: Client API
  - memos-storage: Tiered storage (hot/cold/zombie)
  - memos-index: Vector, fulltext, and hybrid search
  - memos-lifecycle: Lifecycle management
  - memos-train: Training module
- CI/CD workflow (GitHub Actions)
- Code review checklist
- Coding standards documentation
- Pre-commit hooks
- Documentation (EN/ZH)
- Integration tests (22 tests)
- Concurrency tests
- Boundary condition tests

### Test Coverage
- Overall coverage: 93.50%
- memos-core: 36 tests
- memos-api: 82 tests
- memos-storage: 47 tests
- memos-index: 85 tests
- memos-lifecycle: 95 tests
- memos-train: 49 tests

## [Unreleased]

### Added
- Complete memory management system implementation
  - memory-core: Core types and traits
  - memory-api: Client API
  - memory-storage: Tiered storage (hot/cold/zombie)
  - memory-index: Vector, fulltext, and hybrid search
  - memory-lifecycle: Lifecycle management
  - memory-train: Training module
- CI/CD workflow (GitHub Actions)
- Code review checklist
- Coding standards documentation
- Pre-commit hooks
- Documentation (EN/ZH)

### Test Coverage
- memory-core: 36 tests
- memory-api: 65 tests
- memory-storage: 27 tests
- memory-index: 36 tests
- memory-lifecycle: 27 tests
- memory-train: 20 tests
- **Total: 211 tests (100% coverage)**

## [0.1.0] - 2026-04-10

### Added
- Initial release
- Project structure with 6 crates
- Basic memory operations (CRUD)
- Search functionality
- Lifecycle management
- Training support