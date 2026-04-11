# Git Pre-commit Hook

This directory contains git hooks. To enable the pre-commit hook:

```bash
# Copy the hook
cp .git/hooks/pre-commit.sample .git/hooks/pre-commit

# Or create a symlink
ln -s ../../pre-commit.sh .git/hooks/pre-commit
```

The pre-commit hook runs:
1. Code format check (`cargo fmt --check`)
2. Clippy linting (`cargo clippy`)
3. Unit tests (`cargo test`)
4. Documentation check (`cargo doc`)
5. Debug code detection (`println!`)
6. Unwrap detection
7. README existence check