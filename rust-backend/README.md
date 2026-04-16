# Rust Backend 重构骨架

这个目录是 `refactor/rust-backend-v1` 分支上的 Rust 主后端起点。

当前状态：

- 已建立可编译的 `axum + tokio` 服务骨架
- 已接管一批基础配置接口：
  - `GET /health`
  - `GET /api/config`
  - `POST /api/config`
  - `GET /api/models`
  - `POST /api/models`
  - `PATCH /api/models/:name`
  - `DELETE /api/models/:name`
  - `GET /api/providers/catalog`
  - `GET /api/runtime-settings`
  - `POST /api/runtime-settings`
  - `GET /api/doctor`
  - `POST /api/doctor/fix`
  - `GET /api/security-audit`
  - `GET /api/auth-profiles`
  - `GET /api/runtime-health`
  - `GET /api/failover/recent`
  - `GET /api/observability/summary`
  - `GET /api/observability/events`
  - `GET /api/routing`
  - `POST /api/routing`
  - `GET /api/lark/config`
  - `POST /api/lark/config`
  - `GET /api/skills`
  - `POST /api/skills/toggle`
  - `PATCH /api/skills/:name`
  - `GET /api/token-usage`
- 已接管会话基础接口：
  - `GET /api/conversations`
  - `POST /api/conversations`
  - `POST /api/conversations/:conv_id/activate`
  - `PATCH /api/conversations/:conv_id`
  - `DELETE /api/conversations/:conv_id`
  - `GET /api/history`
  - `POST /api/history/clear`
- 已接管第一版聊天链路：
  - `POST /api/chat`
  - `POST /api/chat/abort`
  - `POST /api/chat/resume`
- 已支持把当前激活模型写回现有 `webapp/backend/models.json`
- 已支持把 routing / runtime / skill / lark 配置写回现有配置文件
- 已建立第一版 SQLite 数据层，并支持从旧 `siliconflow/data/conversations.json` 自动导入
- 已支持第一版 tool-calls 循环：
  - 解析上游 `tool_calls`
  - 自动执行安全工具
  - 对危险工具发出 `permission_required`
  - 用户同意后通过 `/api/chat/resume` 继续执行
  - 追加 `assistant(tool_calls)` / `tool` / `assistant(final)` 三段消息到 SQLite
- 已支持第一版危险工具持久化授权：
  - 前端勾选 `always_allow` 后，会写入 `siliconflow/config/permission_settings.json`
  - 当前已接入 `run_terminal`
- 已支持第一版轻量路由与 failover：
  - 读取 `siliconflow/config/routing_config.json`
  - 短问题按 `easy` tier 路由
  - 中长复杂问题按启发式分到 `medium / hard`
  - 路由模型请求失败时，按候选链自动回退
  - 最近 failover 事件可通过 `/api/failover/recent` 查看
- 已支持第一版 planner / audit：
  - 复杂请求会先发 `plan`
  - 执行中发 `step_start / step_done`
  - 自然回答完成后发 `audit`
- 已支持第一版 Rust 语音桥入口：
  - `WS /api/voice/bridge`
  - Rust 直接接管 WebSocket 入口
  - 底层 ASR / VAD / TTS 仍通过 `webapp/backend/voice_worker.py` 调用 Python 运行时
  - 语音桥支持显式携带 `conv_id`
  - TTS 改为句子级 flush，再由 worker 按音频块回传 `audio_stream`
- 已建立第一版模块分层：
  - `api`
  - `app`
  - `domain`
  - `infra`
- 已完成 P0 主链路收口：
  - `api/handlers.rs` 按路由聚类拆成 `api/handlers/` 目录（chat / voice / config / conversations / skills / doctor / observability / health / shared）
  - `chat_service.rs` 的 planner / failover / permission / tool_loop / usage 下沉到 `app/services/chat/` 子模块
  - 会话级运行态收口到 `app/run_registry.rs` 的 `RunRegistry`，替代原先四张散落的 `HashMap/Vec`
  - 引入 `domain/run.rs` 的 `ConvId` / `RunStatus` / `RunError` 作为会话运行态的强类型基础

## 启动方式

```bash
cd rust-backend
cargo run
```

默认地址：

```text
http://127.0.0.1:18000
```

## 当前设计原则

- 现有 Python 后端是行为参考，不再是长期架构目标
- Rust 服务先接管“配置读取 + 基础 API + 文档化模型”
- 后续再逐步迁移：
  - 会话执行器
  - SSE 聊天流
  - tool / skills
  - memory / compression
  - voice bridge
  - doctor / observability

## 当前已接管的数据

- 配置快照：继续读取现有 JSON 配置
- 会话数据：启动时将旧 `conversations.json` 导入到 `siliconflow/data/runtime.db`

说明：

- 当前导入是“首次建库自动导入”
- 一旦 SQLite 中已有数据，不会重复覆盖
- 这保证了迁移期可以平滑试运行，而不破坏旧文件
- 聊天消息会继续写入 SQLite 的 `conversation_messages` 表

## 目录说明

```text
rust-backend/
├── src/
│   ├── api/
│   │   ├── handlers/           # 按路由聚类拆分的 HTTP handler
│   │   │   ├── shared.rs       # 共享 DTO + error helper + sse_json
│   │   │   ├── health.rs
│   │   │   ├── config.rs       # 模型 / runtime / lark / terminal 等配置接口
│   │   │   ├── skills.rs       # skills / routing / token-usage
│   │   │   ├── conversations.rs
│   │   │   ├── chat.rs         # chat / abort / resume + 工具循环
│   │   │   ├── voice.rs        # 语音桥 WebSocket
│   │   │   ├── doctor.rs
│   │   │   └── observability.rs
│   │   └── router.rs           # 路由表
│   ├── app/
│   │   ├── run_registry.rs     # 会话级运行态（abort / run lock / pending permission / failover log）
│   │   ├── state.rs            # AppState 装配
│   │   └── services/
│   │       ├── chat_service.rs # 聊天服务门面
│   │       ├── chat/           # chat_service 的算法子模块
│   │       │   ├── planner.rs
│   │       │   ├── failover.rs
│   │       │   ├── permission.rs
│   │       │   ├── tool_loop.rs
│   │       │   └── usage.rs
│   │       ├── config_service.rs
│   │       ├── conversation_service.rs
│   │       └── tool_service.rs
│   ├── domain/
│   │   ├── run.rs              # ConvId / RunStatus / RunError
│   │   ├── models.rs
│   │   ├── conversation.rs
│   │   └── doctor.rs
│   └── infra/
│       ├── conversation_store.rs
│       ├── config_loader.rs
│       └── sqlite.rs
└── README.md
```

## Rust 重构待办

下面这份清单是后续实现的主线，不再以对话内容为准。

### P0：先做主链路收口（已完成）

1. ✅ 拆分 `handlers.rs` → `api/handlers/` 目录，按路由聚类分文件
2. ✅ 压缩 `chat_service.rs`，planner / failover / permission / tool_loop / usage 下沉到 `app/services/chat/` 子模块
3. ✅ 建立会话级 `RunRegistry`，`abort / resume / permission_required` 共享同一 `RunHandle`

### P1：继续替代 Python 数据层

1. ✅ 把 token usage 持久化统一迁入 SQLite（第一批 #7）
2. ✅ 把 execution events 持久化统一迁入 SQLite（第一批 #8）
3. ✅ 把 permission state 持久化统一迁入 SQLite（第一批 #5）
4. N/A 把 voice session state 持久化统一迁入 SQLite — `voice_session_state` 是 WS 瞬时信令（listening/processing/speaking 相位切换），无需入库
5. ✅ 为这些表补中文注释和迁移说明 — 见 `docs/data_migration.md`（P1 第二批 #6）

### P1：继续整理 provider / failover 架构

1. ✅ 抽出明确的 `ProviderDriver` trait（第一批 #4）
2. ✅ 抽出 `FailoverPolicy`（第一批 #4）
3. ✅ 抽出 `AuthProfileResolver`（P1 第二批 #2）— 新增 `src/infra/auth_profile.rs` + `ConfigAuthResolver` 默认实现；`ChatService` 持有 `Arc<dyn AuthProfileResolver>`，`chat/failover.rs::build_fallback_chain` 接受 `&dyn AuthProfileResolver`；`ConfigService::resolve_api_key / default_api_url` 保留给 `config_view` 内部用
4. ✅ 让错误分类驱动 failover，而不是散落在 handler 中（第一批 #3）
5. 保留 trait 扩展点：NVIDIA / DeepSeek / SiliconFlow / Kimi 目前都 OpenAI 兼容，统一走 `OpenAiCompatDriver`；未来若真出现差异再拆子驱动

### P1：继续整理 planner / tool 执行架构

1. ✅ planner 改成显式状态机（P1 第二批 #3）— 新增 `src/app/services/chat/run_phase.rs`，`RunPhase` 枚举覆盖 `text / usage / tool_start / tool_done / permission_required / failover_step / failover_exhausted / step_done / audit / aborted / done / error` 所有对外事件；`ChatExecutor::run_loop` 原先十余处内联 `tx.send(serde_json!(...))` 全部改走 `emit(RunPhase::X)`，SSE payload 构造收敛到一处
   - `plan`
   - `step_start`
   - `step_done`
   - `audit`
   - `retry`
2. ✅ tool loop 改成独立模块（`app/services/chat/tool_loop.rs`）
3. ✅ permission resume 改成显式 checkpoint 恢复（`PendingPermission` + `RunHandle::set_pending/take_pending`）
4. ✅ 把危险工具和安全工具的执行路径彻底统一（P1 第二批 #4）— 审查通过，无需代码改动：`ChatExecutor::run_loop` / `resume` 两处都走 `chat_service.requires_permission(&name)` 单一闸门，拒绝 → 追加 tool 拒绝消息，允许/安全 → 统一进 `execute_tool_tracked`，安全 / 放行 / 即席授权三条分支在工具执行层零分叉

### P2：继续整理语音链，但重点仍在 Rust 内核

1. ✅ 把语音桥控制层从 handlers 拆到独立 voice service（P1 第二批 #5）— `handlers/voice.rs` 只负责 WS upgrade / 消息编解码；相位信令 payload 构造与 worker stdin 写入统一收敛到 `src/app/services/voice_bridge.rs::voice_session_state_payload` / `write_worker_line`，handler 的 `send_voice_session_state` / `write_worker_json` 退化为薄封装
2. ✅ 把 `voice bridge -> chat -> tts` 变成明确的内部状态机（`VoiceBridge::run_chat` 接管事件流 + TTS 切句，handler 不再自打 HTTP）
3. ✅ 继续缩小 `webapp/backend/voice_worker.py` 的职责（P2a TTS 迁 Rust）— 新增 `src/app/services/edge_tts.rs`（~300 LOC），忠实移植 Python `edge_tts` 包的 WSS 协议（DRM token / SSML 构造 / 二进制帧解析 / 403 clock skew 重试）；`voice_bridge.rs` 的 `flush_tts_sentences` 不再写 `tts_stream` 到 worker stdin，改为通过 TTS channel 排队合成并直接经 WS 发送 `audio_stream` 给前端；`voice_worker.py` 删除 `tts_stream` 分支，`voice_engine.py` 删除 `generate_speech` / `get_streaming_tts` / `import edge_tts`；worker 退化为纯 ASR 进程
4. 优先迁移语音控制逻辑，再评估是否迁移推理层（ASR/KWS/VAD 依赖 sherpa-onnx C FFI + ONNX Runtime ~200MB，留待 P2b 评估）

### P2：导入与迁移补齐（P1 第二批 #6）

1. ✅ 从 Python 旧数据导入 — 新增 `src/infra/memory_store.rs`（`memories` 表 + `legacy_profile` 首启导入）；`src/bin/import_legacy.rs` 汇总 `permission_grants` / `token_usage` / `execution_events` / `memories` 行数报告，幂等重跑
   - token（`token_usage` 表，已在首启自动导入）
   - memory（`memories` 表，首启自动导入 `memory.json` 扁平 kv 为 `legacy_profile` kind）
   - execution logs（`execution_events` 表由运行时写入；旧 `siliconflow/data/logs/` 已不存在，跳过）
   - permission settings（`permission_grants` 表，已在首启自动导入）
2. ✅ 补齐迁移文档 — `docs/data_migration.md`（schema / 导入命令 / 校验 / 回滚）
3. ✅ 补齐回滚说明 — 见 `docs/data_migration.md` 末节

## 下一轮改动清单（P1 第一批）

下面这份清单是 P0 收口之后、马上要推进的 9 项具体改动。每一项都带有「当前状态 / 目标 / 主要落点 / 验证」，可以独立成一次提交。

推荐执行顺序：`1 → 2 → 3 → 6 → 5/7/8（可并行）→ 4 → 9`。

### ✅ 1. 把 `stream_upstream_chat` 从 handler 抽成 `ChatExecutor`

- 当前状态：`src/api/handlers/chat.rs:70-454` 仍然塞着 330+ 行的 `stream_upstream_chat` 和 `resume_pending_permission`，handler 兼职执行层
- 目标：新建 `src/app/services/chat/executor.rs`，暴露 `ChatExecutor::stream_once(prepared, tx)` 和 `ChatExecutor::resume(pending, tx)`
- 主要落点：
  - 新建 `app/services/chat/executor.rs`
  - 把 handler 中的 SSE 组装、上游请求、tool_call 循环、failover 回退、audit retry 整段搬进去
  - `handlers/chat.rs` 瘦成只做 `prepare → acquire_run_slot → spawn(executor.stream_once) → 返回 SSE`
- 验证：重跑聊天 + 工具调用 + permission resume + failover，SSE 事件顺序完全不变
- **实际落点**：已迁入 `src/app/services/chat/executor.rs`（401 LOC）；`handlers/chat.rs` 从 519 → 141 LOC；`AppState` 新增 `chat_executor: ChatExecutor` 字段

### ✅ 2. 让 `RunStatus` 真正反映会话运行态

- 当前状态：`RunStatus::Running / Done / Aborted` 定义在 `domain/run.rs:28`，但执行路径从未调用；只有 `set_pending / take_pending` 会切 `AwaitingPermission / Running`（见 `app/run_registry.rs:105, 112`）
- 目标：`ChatExecutor` 在进入/退出每一阶段时显式推进 `RunStatus`，让 `/api/observability/*` 可以读到真实状态
- 主要落点：
  - `ChatExecutor::stream_once` 启动时 `handle.set_status(Running)`
  - 正常完成 → `Done`；abort 分支 → `Aborted`
  - 观测接口新增 `GET /api/runs/:conv_id`（或直接并进 `/api/runtime-health`）返回当前 `RunStatus`
- 验证：运行中 GET 能看到 `Running`；触发 permission 时能看到 `AwaitingPermission`；完成后回到 `Idle/Done`
- **实际落点**：`ChatExecutor::stream_once` 入口打 `Running`，出口按 abort / permission / 正常分支打 `Aborted / AwaitingPermission / Done`；`RunRegistry::active_runs` + `ChatService::active_runs` 暴露非 Idle 会话；`/api/runtime-health` 响应新增 `active_runs: [{conv_id, status}]` 数组

### ✅ 3. 让 `RunError` 真正驱动错误流

- 当前状态：`RunError` 在 `domain/run.rs:93` 定义，但整个 crate 没有第二处使用；错误仍然靠 `anyhow::Error` + 字符串 `error_class` 传递
- 目标：`ChatExecutor::stream_once` 返回 `Result<(), RunError>`，由调用方按变体分发 SSE（`aborted / upstream / tool / permission_denied / invalid_state`）
- 主要落点：
  - `app/services/chat/executor.rs` 签名改用 `RunError`
  - `app/services/chat/failover.rs::classify_upstream_error` 直接返回 `RunError::Upstream { class }` 而不是字符串
  - handler 只做「`RunError` → SSE error payload」的平铺翻译
- 验证：前端 `error_class` 字段保持与改造前完全一致（`aborted / upstream_error / rate_limited / auth_error / tool_error / …`）
- **实际落点**：`stream_once / resume` 返回 `Result<(), RunError>`；内部 `to_run_error` 把 anyhow 错误按 `classify_upstream_error` 结果归并到 `RunError::Upstream(class)`；`handlers/chat.rs::run_error_to_sse` 统一把变体平铺到 `{type, content, error_class}` payload

### ✅ 4. 抽出 `ProviderDriver` + `FailoverPolicy` trait

- 当前状态：
  - `ChatService::send_stream_request` 硬写 OpenAI 风格 payload（`messages / tools / tool_choice / stream / stream_options`）
  - NVIDIA / DeepSeek / SiliconFlow / Kimi 的差异目前靠 `provider` 字段在 `build_fallback_chain` 里临时分支
  - `classify_upstream_error` 在 `chat/failover.rs` 里是自由函数
- 目标：
  ```rust
  trait ProviderDriver {
      fn build_request(&self, prepared: &PreparedChatRun, msgs: &[Value]) -> RequestBuilder;
      fn parse_chunk(&self, raw: &str) -> Option<ChatChunk>;
  }
  trait FailoverPolicy {
      fn next_candidate(&self, current: &ModelRuntime, err: &RunError) -> Option<ModelRuntime>;
  }
  ```
- 主要落点：
  - 新建 `infra/providers/` 目录：`openai_compat.rs`（默认驱动）+ 各家差异化子模块
  - 新建 `app/services/chat/policy.rs` 实现 `FailoverPolicy`
  - `ChatExecutor` 只依赖两个 trait，不再看 `provider` 字符串
- 验证：四家 provider 的成功 / 失败路径各跑一次
- **实际落点**：新增 `src/infra/providers/{mod,openai_compat}.rs`，`ChatService` 持有 `Arc<dyn ProviderDriver>`，`send_stream_request` 走 `driver.build_payload`；新增 `src/app/services/chat/policy.rs`（`FailoverPolicy` + `DefaultFailoverPolicy`），`ChatExecutor` 的 fallback 判定统一走 `policy.next_candidate(RunError, chain, idx)`，不再在 executor 里内联 error_class 白名单

### ✅ 5. `permission_settings` 迁入 SQLite

- 当前状态：持久化仍落在 `siliconflow/config/permission_settings.json`（`chat_service.rs::maybe_persist_always_allow` 写的就是这个文件）
- 目标：新表 `permission_grants(tool_name TEXT PRIMARY KEY, always_allow INT, source TEXT, updated_at TEXT)`，首启时一次性从 JSON 导入，其后只写 SQLite
- 主要落点：
  - `infra/sqlite.rs` 加迁移 + `permission_store.rs`
  - `ToolService::is_always_allowed / grant_always_allow` 改读写 SQLite
  - 保留 JSON 作为只读 fallback，便于回滚
- 验证：勾选 `always_allow` 后重启 Rust，再次调用同一危险工具不再弹 `permission_required`
- **实际落点**：新增 `src/infra/permission_store.rs` + `permission_grants` 表；`ConfigService` 持有 `PermissionStore`，`is_tool_always_allowed` / `allow_tool_always` / `permission_settings` 改走 SQLite；首启时若表空则从 `permission_settings.json` 一次性导入

### ✅ 6. 语音桥去掉自打 HTTP 的回环

- 当前状态：`src/api/handlers/voice.rs:297`（`stream_chat_to_voice_ws`）和 `:436`（`abort_chat_via_http`）都在对 `http://127.0.0.1:18000/api/chat(/abort)` 发请求，相当于 Rust 自己绕 loopback
- 目标：`VoiceBridge` 直接调用 `ChatExecutor::stream_once`，拿到的 SSE 事件流内部转换成 WebSocket 帧；abort 直接走 `RunRegistry::abort`
- 主要落点：
  - 新建 `app/services/voice_bridge.rs`
  - `handlers/voice.rs` 只做 WebSocket upgrade + 消息编解码
  - 语音 TTS 句子切分保留在 voice bridge 内（继续通过 `voice_worker.py` 的 `tts_stream`）
- 验证：去掉回环后，确认 `127.0.0.1:18000` 出站连接数不再增加；ASR → chat → TTS 链路延迟与原版持平或更低
- 依赖：需要先做 #1（`ChatExecutor` 可被内部直接调用）
- **实际落点**：新增 `src/app/services/voice_bridge.rs`；`handlers/voice.rs` 去掉 `127.0.0.1:18000` 回环，ASR 文本直接驱动 `ChatExecutor::stream_once`，abort 改调 `ChatService::abort`；TTS 句切逻辑迁入 `voice_bridge`，continue 透传给 WS 客户端

### ✅ 7. `token_usage` 迁入 SQLite

- 当前状态：仍落在 `siliconflow/data/token_usage.json`
- 目标：表 `token_usage(conv_id, model, prompt, completion, total, created_at)` + 聚合视图；`/api/token-usage` 改读 SQLite
- 主要落点：`infra/token_store.rs` + `app/services/chat/usage.rs::UsageSnapshot::persist`
- 验证：SSE `done` 事件对应一行新纪录；`GET /api/token-usage` 总量与改造前一致
- **实际落点**：新增 `src/infra/token_store.rs` + `token_usage` 表（含 conv / model 索引）；`ChatService::finalize_chat` 完成即插一行；`ConfigService::token_usage` 用 `aggregate_total` + `aggregate_by_model` 覆盖 `global.calls/prompt/completion/total/by_type`，`/api/token-usage` 的其余字段仍与旧 JSON 同源

### ✅ 8. `execution_events` 首次入库

- 当前状态：`webapp/backend/execution_logger.py` 还在写文件 / 内存；Rust 侧完全没有持久化 tool 执行日志
- 目标：新表 `execution_events(id, conv_id, tool_name, status, request_json, response_json, elapsed_ms, created_at)`，每次 `tool_loop` 执行一次就落一行
- 主要落点：`infra/execution_store.rs` + `app/services/chat/tool_loop.rs`
- 验证：对同一 conv_id 调用 3 个工具，表里出现 3 行，`/api/observability/events` 读得到
- **实际落点**：新增 `src/infra/execution_store.rs` + `execution_events` 表（conv_id / created_at 索引）；`ChatService::execute_tool_tracked` 记录 status + 耗时 + request/response；`ChatExecutor` 正常/resume 两条工具路径都走 tracked；`/api/observability/events` 优先读 SQLite，不足数时回退旧 JSONL

### 9. 压缩 `config_service.rs`（746 LOC） ✅

- 当前状态：单文件塞了 models / skills / routing / runtime-settings / lark / terminal / observability / auth-profiles / doctor
- 目标：拆到 `app/services/config/{models,skills,runtime,routing,observability,auth}.rs`，`ConfigService` 退化成薄门面
- 主要落点：按职责搬家，保持对外 API 完全不变（handler 一行不改）
- 验证：每个 config handler 跑一次冒烟，返回结构字段逐一对齐
- **实际落点**：新建 `src/app/services/config/{mod,auth,models,observability,routing,runtime,skills}.rs`，每个子模块只追加 `impl ConfigService` 块；`config_service.rs` 瘦身到 ~200 LOC，仅保留 struct、构造、`snapshot/config_view/models_view/runtime_settings`、`resolve_api_key/default_api_url`、`doctor_report`、`reload` 和通用 `read_/write_json_file + read_/write_env_file` 工具；handler 层零改动

---

## 下一阶段

P0 已闭环，下一轮按上面的 **P1 第一批** 推进，主线是：

1. 先把执行层抽出来（#1 `ChatExecutor`），解开 handler 与会话执行的耦合
2. 再把 `RunStatus / RunError`（#2 #3）通到新执行层，让观测和错误真正强类型
3. 然后做 provider / failover 收口（#4），为多 provider 扩展铺路
4. 过程中穿插 SQLite 迁移（#5 #7 #8）和语音桥去回环（#6）

当前自动工具执行范围：

- `get_current_time`
- `get_weather`
- `get_system_info`
- `monte_carlo_integration`
- `summary_rules`

当前仍受限的工具：

- `file_editor`
- `write_python`
- `pip_venv`
- `vision_analyze`

当前已支持权限后执行的危险工具：

- `run_terminal`
- `file_editor`
- `write_python`

## P1 第二批 + P2 第一批 闭环（2026-04-16）

- #1 README 同步 P1 主清单 + 去重（删除重复段，P1 三个小节逐条 ✅ / N/A 标注）
- #2 `AuthProfileResolver` 抽出（`src/infra/auth_profile.rs` + `ConfigAuthResolver`；`ChatService` / `chat/failover.rs` 改走 trait）
- #3 `RunPhase` 显式状态机（`src/app/services/chat/run_phase.rs`；`ChatExecutor::run_loop` 十余处内联 `tx.send(serde_json!(...))` 全部收敛为 `emit(RunPhase::X)`）
- #4 工具路径统一审查通过（`execute_tool_tracked` 单闸门，无需代码改动）
- #5 voice handler 瘦身（`voice_session_state_payload` / `write_worker_line` 搬进 `voice_bridge.rs`；`handlers/voice.rs` 仅留 WS 编解码薄壳）
- #6 Python 旧数据导入（新增 `memories` 表 + `MemoryStore::bootstrap` 首启导入 + `cargo run --bin import_legacy` 校验工具；`docs/data_migration.md`）
- #7 回归：`cargo build --release` 通过；`cargo clippy` 余下告警均为既有问题（与本轮改动无关）；`GET /health` / `GET /api/runtime-health`（含 `active_runs: []`）/ `GET /api/observability/events` 返回预期结构；`import_legacy` 幂等，`memories=6` 与 `memory.json` kv 数一致
