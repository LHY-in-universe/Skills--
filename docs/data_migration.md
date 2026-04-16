# 数据迁移：Python JSON → Rust SQLite

本文档记录 Rust 后端（`rust-backend/`）如何接管原 Python 侧（`webapp/backend/`、`siliconflow/data/`）基于 JSON 的持久化，并给出一次性导入、校验与回滚路径。

## 当前接管情况

| 数据域           | 源 JSON                                      | SQLite 表            | 接管方式                               |
| ---------------- | -------------------------------------------- | -------------------- | -------------------------------------- |
| 权限 always_allow | `siliconflow/config/permission_settings.json` | `permission_grants`  | 首启自动导入 + 运行时双写               |
| Token 用量       | `siliconflow/data/token_usage.json`          | `token_usage`        | 首启自动导入 + 运行时双写               |
| 执行事件（工具） | 历史散落在 stdout / `logs/*.jsonl`           | `execution_events`   | 运行时写入；旧 logs 目录已不存在，跳过  |
| 用户画像 / 备忘   | `siliconflow/data/memory.json`               | `memories`           | 首启自动导入（`legacy_profile` kind）   |
| 会话对话         | `siliconflow/data/conversations.json`        | `conversations` / `conversation_messages` | 首启自动导入 |

`memories` 表 schema：

```sql
CREATE TABLE memories (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    conv_id TEXT,
    kind TEXT NOT NULL,
    content_json TEXT NOT NULL,
    created_at TEXT NOT NULL
);
CREATE INDEX idx_memories_conv ON memories(conv_id);
CREATE INDEX idx_memories_kind ON memories(kind);
```

## 导入命令

正常启动 `cargo run` / `cargo run --release` 时，各 `*_store::bootstrap` 会在表空时自动导入一次。若需要独立确认/跑一遍，用：

```bash
cd rust-backend
cargo run --bin import_legacy --release
```

输出形如：

```
[import_legacy] project_root = /path/to/repo
[import_legacy] runtime.db = /path/to/repo/siliconflow/data/runtime.db
[import_legacy] counts after bootstrap:
  permission_grants  = 3
  token_usage        = 27
  execution_events   = 104
  memories           = 6
```

该工具幂等：表非空时不会重复导入，重跑只刷新报告。

## 校验

```bash
sqlite3 siliconflow/data/runtime.db "SELECT kind, COUNT(*) FROM memories GROUP BY kind"
sqlite3 siliconflow/data/runtime.db "SELECT tool_name, always_allow FROM permission_grants"
sqlite3 siliconflow/data/runtime.db "SELECT date(created_at, 'unixepoch'), SUM(total_tokens) FROM token_usage GROUP BY 1 ORDER BY 1 DESC LIMIT 7"
```

## 回滚

1. 停止 Rust 后端进程。
2. 备份并删除 `siliconflow/data/runtime.db`（或恢复备份副本）。
3. 源 JSON 文件未被修改；Python 端可继续读旧文件。

下次启动 Rust 后端时会再次自动导入，恢复至幂等状态。
