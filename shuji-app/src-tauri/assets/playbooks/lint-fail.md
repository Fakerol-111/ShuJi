# Lint 失败排查

## 常见问题

### Rust (clippy)

- `clippy::unwrap_used` → 用 `?` 或 `match` 替代 `.unwrap()`
- `clippy::missing_docs` → 为 pub 项添加 `///` 文档注释
- `clippy::dead_code` → 删除未使用的变量/函数

### Python (ruff)

- `ANN` 规则 → 添加类型注解
- `D` 规则 → 添加 docstring
- `I` 规则 → 调整导入顺序

### Node (eslint)

- `no-explicit-any` → 使用 `unknown` 或具体类型
- 格式化 → 运行 `npx prettier --write`
