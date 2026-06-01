# Rust Backend

基于 `axum + tokio` 的主后端服务，已完全替代原 Python FastAPI 后端。

## 启动

```bash
cd rust-backend
cargo run --release
```

默认监听：`http://127.0.0.1:18000`

## API 接口

### 基础

| 方法 | 路径 | 说明 |
|------|------|------|
| GET | `/health` | 健康检查 |
| GET | `/api/runtime-health` | 运行时状态 + 活跃会话 |

### 配置

| 方法 | 路径 | 说明 |
|------|------|------|
| GET/POST | `/api/config` | 当前配置 / 切换模型 |
| GET/POST/PATCH/DELETE | `/api/models` | 模型 CRUD |
| GET | `/api/providers/catalog` | Provider 列表 |
| GET/POST | `/api/runtime-settings` | 运行时参数 |
| GET/POST | `/api/routing` | 路由策略 |
| GET/POST | `/api/lark/config` | 飞书桥接配置 |
| GET | `/api/auth-profiles` | 认证配置 |

### 聊天

| 方法 | 路径 | 说明 |
|------|------|------|
| POST | `/api/chat` | SSE 流式聊天 |
| POST | `/api/chat/abort` | 中断当前对话 |
| POST | `/api/chat/resume` | 恢复等待权限的对话 |

### 会话

| 方法 | 路径 | 说明 |
|------|------|------|
| GET/POST | `/api/conversations` | 会话列表 / 新建 |
| POST | `/api/conversations/:id/activate` | 激活会话 |
| PATCH/DELETE | `/api/conversations/:id` | 修改 / 删除 |
| GET | `/api/history` | 消息历史 |
| POST | `/api/history/clear` | 清空历史 |

### 技能 & 工具

| 方法 | 路径 | 说明 |
|------|------|------|
| GET | `/api/skills` | 技能列表 |
| POST | `/api/skills/toggle` | 启用/禁用 |
| PATCH | `/api/skills/:name` | 修改技能参数 |
| GET | `/api/token-usage` | Token 统计 |

### 可观测性

| 方法 | 路径 | 说明 |
|------|------|------|
| GET | `/api/doctor` | 诊断报告 |
| POST | `/api/doctor/fix` | 自动修复 |
| GET | `/api/security-audit` | 安全审计 |
| GET | `/api/model-connectivity` | 当前模型与路由模型的真实连通性自检 |
| GET | `/api/failover/recent` | 最近 failover 事件 |
| GET | `/api/observability/summary` | 运行摘要 |
| GET | `/api/observability/events` | 执行事件日志 |

### 语音

| 方法 | 路径 | 说明 |
|------|------|------|
| WS | `/api/voice/bridge` | 语音桥 WebSocket |

## 语音链路

完整的 Rust 进程内语音闭环，不依赖任何 Python 子进程：

```
前端音频帧 → VoicePipeline (KWS → VAD → ASR) → 文本
                                                    ↓
                                            VoiceBridge.run_chat
                                                    ↓
                                          ChatExecutor 流式输出
                                                    ↓
                                    句切 → Edge TTS → audio_stream → 前端
```

- **KWS**：zipformer keyword spotter（sherpa-rs-sys FFI 流式解码）
- **VAD**：Silero VAD（sherpa-rs 流式端点检测）
- **ASR**：Paraformer 离线识别（sherpa-rs）
- **TTS**：Edge TTS WSS 协议（Rust 原生实现）

## 目录结构

```text
rust-backend/
├── config/
│   └── models.json             # 模型配置
├── models/                     # 语音 ONNX 模型（gitignore）
│   └── voice/
│       ├── sherpa-onnx-paraformer-zh-2023-09-14/
│       ├── sherpa-onnx-kws-zipformer-wenetspeech-3.3M-2024-01-01/
│       ├── silero_vad.onnx
│       └── keywords.txt
├── build.rs                    # macOS/Linux rpath 配置
├── Cargo.toml
└── src/
    ├── main.rs
    ├── lib.rs
    ├── api/
    │   ├── router.rs           # 路由表
    │   └── handlers/           # HTTP handler
    │       ├── health.rs
    │       ├── config.rs
    │       ├── chat.rs
    │       ├── conversations.rs
    │       ├── skills.rs
    │       ├── voice.rs
    │       ├── doctor.rs
    │       ├── observability.rs
    │       └── shared.rs
    ├── app/
    │   ├── state.rs            # AppState
    │   ├── run_registry.rs     # 会话运行态管理
    │   └── services/
    │       ├── chat_service.rs
    │       ├── chat/           # 聊天子模块
    │       │   ├── executor.rs     # ChatExecutor
    │       │   ├── planner.rs      # 计划生成
    │       │   ├── failover.rs     # 错误分类 + 回退
    │       │   ├── policy.rs       # FailoverPolicy
    │       │   ├── permission.rs   # 权限管理
    │       │   ├── tool_loop.rs    # 工具循环
    │       │   ├── usage.rs        # Token 统计
    │       │   └── run_phase.rs    # RunPhase 状态机
    │       ├── config_service.rs
    │       ├── config/         # 配置子模块
    │       │   ├── models.rs
    │       │   ├── routing.rs
    │       │   ├── runtime.rs
    │       │   ├── skills.rs
    │       │   ├── auth.rs
    │       │   └── observability.rs
    │       ├── conversation_service.rs
    │       ├── tool_service.rs
    │       ├── voice_bridge.rs     # 语音桥（聊天 + TTS 切句）
    │       ├── voice_pipeline.rs   # KWS / VAD / ASR 管线
    │       └── edge_tts.rs         # Edge TTS WSS 协议
    ├── domain/
    │   ├── run.rs              # ConvId / RunStatus / RunError
    │   ├── models.rs
    │   ├── conversation.rs
    │   └── doctor.rs
    └── infra/
        ├── sqlite.rs           # SQLite 连接 + 迁移
        ├── config_loader.rs    # 配置快照加载
        ├── conversation_store.rs
        ├── token_store.rs
        ├── execution_store.rs
        ├── permission_store.rs
        ├── memory_store.rs
        ├── auth_profile.rs
        └── providers/
            ├── mod.rs          # ProviderDriver trait
            └── openai_compat.rs
```

## 数据层

所有持久化数据使用 SQLite（`siliconflow/data/runtime.db`）：

| 表 | 说明 |
|----|------|
| `conversations` | 会话元信息 |
| `conversation_messages` | 聊天消息 |
| `token_usage` | Token 用量统计 |
| `execution_events` | 工具执行日志 |
| `permission_grants` | 持久化授权 |
| `memories` | 记忆数据 |

首次启动时自动从旧 JSON 文件导入。详见 [docs/data_migration.md](../docs/data_migration.md)。

## 工具系统

自动执行的安全工具：`get_current_time` / `get_system_info` / `monte_carlo_integration` / `summary_rules`

需要权限确认的危险工具：`run_terminal` / `file_editor` / `write_python` / `pip_venv` / `vision_analyze`

## 模型连通性排障

若需要验证本地 `.env` 中的 key 是否真的能打通当前模型配置，可使用：

```bash
python3 ../siliconflow/scripts/check_model_connectivity.py
```

或者在后端运行时请求：

```bash
curl http://127.0.0.1:18000/api/model-connectivity
```

这会对当前启用模型、`router_model`、`summary_model` 和 `easy/medium/hard` tier
中用到的模型发起最小非流式请求，并返回：

- `ok`
- `status`
- `diagnosis`
- `recommendation`
