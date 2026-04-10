# Code Review Checklist

## 1. Code Format (自动化检查)
- [ ] `cargo fmt --check` 通过
- [ ] `cargo clippy -- -D warnings` 无警告
- [ ] 无多余空行或无用注释

## 2. Documentation (文档对齐)
- [ ] 所有 public API 都有文档注释 (`///` 或 `//!`)
- [ ] README.md 存在且与代码功能对齐
- [ ] 复杂逻辑有内联注释说明
- [ ] CHANGELOG.md 已更新（如果需要）

## 3. Test Coverage (测试覆盖)
- [ ] 单元测试覆盖率 100%
- [ ] 所有 public 方法有对应测试
- [ ] 边界条件和错误处理已覆盖
- [ ] `cargo test --workspace` 全部通过

## 4. Black Box Testing (黑盒测试)
- [ ] 功能测试：验证 API 行为符合预期
- [ ] 集成测试：多模块协作正常
- [ ] 压力测试：大数据量场景正常

## 5. White Box Testing (白盒测试)
- [ ] 单元测试：每个函数/方法有测试
- [ ] 路径覆盖：关键代码路径已覆盖
- [ ] 异常处理：所有 error 分支已测试

## 6. Code Style (编码规范)
- [ ] 命名规范： snake_case (变量/函数), CamelCase (类型)
- [ ] 错误处理：使用 `Result` 而非 `unwrap()` (除测试外)
- [ ] 生命周期：所有借用正确标注
- [ ] 依赖：无不必要依赖

## 7. Security (安全)
- [ ] 无硬编码密钥或凭证
- [ ] 输入验证完整
- [ ] `cargo audit` 无漏洞

## 8. Performance (性能)
- [ ] 无明显性能问题
- [ ] 资源使用合理（内存/线程）

## 提交前检查命令
```bash
# 1. 格式检查
cargo fmt --check
cargo clippy --workspace -- -D warnings

# 2. 测试
cargo test --workspace

# 3. 文档
cargo doc --workspace --no-deps --document-private-items

# 4. 安全审计
cargo audit

# 5. 覆盖率
cargo llvm-cov --workspace --summary-only
```