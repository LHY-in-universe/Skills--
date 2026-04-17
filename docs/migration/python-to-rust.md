# Python 到 Rust 迁移说明

## 迁移原则

- Python 后端视为“行为参考实现”
- Rust 后端视为“新的主实现”
- 迁移过程优先保持前端核心体验不丢失

## 现有 Python 模块与 Rust 对应关系

### `webapp/backend/main.py`

Rust 对应：

- `rust-backend/src/main.rs`
- `rust-backend/src/api/router.rs`

### `webapp/backend/orchestrator.py`

Rust 不再保留单文件对应物，而是拆分到：

- `src/app`
- `src/domain`
- `src/infra`

### `webapp/backend/provider_registry.py`

Rust 后续对应：

- `src/domain`
- `src/infra/provider_*`

### `webapp/backend/doctor.py`

Rust 对应：

- `src/domain/doctor.rs`
- `src/app/services`

## 第一阶段已迁移内容

- 模型配置读取
- 运行时策略读取
- 基础 doctor 只读报告
- 基础 API 结构
- conversations SQLite 导入
- conversations/history 基础接口

## 还未迁移的核心能力

- `/api/chat`
- `/api/chat/resume`
- `/api/chat/abort`
- conversation 运行态执行器
- token usage
- execution logs
- voice bridge
- memory / compression
- skill 执行
