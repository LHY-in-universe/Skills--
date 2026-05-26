# Rust 后端 API 说明

## 第一阶段已实现接口

### `GET /health`

用途：

- 服务存活检查

返回示例：

```json
{
  "ok": true,
  "service": "skills-rust-backend",
  "version": "0.1.0"
}
```

### `GET /api/config`

用途：

- 返回当前配置视图
- 供前端显示当前模型、provider 和 URL

### `POST /api/config`

用途：

- 更新当前激活模型
- 当前阶段只支持 `{ "model": "显示名" }` 这一种更新方式
- 会把结果写回 `webapp/backend/models.json`

### `GET /api/models`

用途：

- 返回模型清单
- 当前直接返回对象映射，结构与现有前端兼容：

```json
{
  "DeepSeek-Chat": {
    "id": "deepseek-chat",
    "provider": "deepseek",
    "api_url": null,
    "enabled": true,
    "capabilities": {
      "chat": true,
      "vision": false,
      "tools": true
    },
    "requires": []
  }
}
```

### `POST /api/models`

用途：

- 新增模型定义
- 当前会写回 `webapp/backend/models.json`

### `PATCH /api/models/:name`

用途：

- 更新模型的 `model_id / provider / api_url`

### `DELETE /api/models/:name`

用途：

- 删除模型定义
- 若删除后没有任何 `enabled=true` 的模型，会自动启用第一条模型

### `GET /api/providers/catalog`

用途：

- 返回 provider 目录
- 供前端新增模型时显示默认 URL 和所需环境变量

### `GET /api/runtime-settings`

用途：

- 返回运行时策略

### `POST /api/runtime-settings`

用途：

- 更新运行时策略
- 当前直接写回 `siliconflow/config/runtime_config.json`

### `GET /api/doctor`

用途：

- 返回基础静态预检结果

### `GET /api/conversations`

用途：

- 返回会话列表
- 当前已从 SQLite 读取，并包含 `active` 与 `message_count`

### `POST /api/conversations`

用途：

- 创建新会话
- 创建后自动设为 active

### `POST /api/conversations/:conv_id/activate`

用途：

- 切换当前活跃会话

### `PATCH /api/conversations/:conv_id`

用途：

- 修改会话名称

### `DELETE /api/conversations/:conv_id`

用途：

- 删除会话
- 如果删除的是活跃会话，自动激活最近一条会话

### `GET /api/history`

用途：

- 读取指定或当前 active 会话的历史消息
- 当前默认过滤 `system` 消息，对齐现有前端行为

### `POST /api/history/clear`

用途：

- 清空指定或当前 active 会话的消息历史
- 当前实现会直接删除 SQLite 中该会话的 `conversation_messages`

### `POST /api/chat`

用途：

- 执行第一版聊天请求
- 当前实现会直接调用上游兼容 OpenAI Chat Completions 的流式接口
- Rust 服务会把上游 `delta.content` 解析后转成前端可消费的 `text` 事件

当前限制：

- 还没有完整会话 actor 和 run registry
- 已支持第一版 tool-calls、权限中断和 resume
- 已支持第一版 planner、audit 和 failover 链，但还没有完整路由分类器
- 只覆盖了 OpenAI 风格 `data: { choices[0].delta.content }` 的主流兼容协议

SSE 事件类型：

- `start`
- `plan`
- `step_start`
- `step_done`
- `text`
- `usage`
- `audit`
- `tool_start`
- `tool_done`
- `permission_required`
- `failover_step`
- `failover_exhausted`
- `done`
- `error`
- `aborted`

`start` 事件当前还会携带这些路由字段：

- `_route`
- `_tier`
- `_model`
- `_model_id`
- `_provider`

当前已支持自动执行的工具：

- `get_current_time`
- `get_system_info`
- `monte_carlo_integration`
- `summary_rules`

当前已支持“审批后执行”的危险工具：

- `run_terminal`
- `file_editor`
- `write_python`
- `pip_venv`
- `vision_analyze`

当前工具调用落库方式：

- `assistant`（带 `tool_calls`）
- `tool`
- `assistant`（最终自然语言回答）

### `POST /api/chat/abort`

用途：

- 中断当前聊天输出
- 当前支持显式传入 `conv_id`

请求示例：

```json
{
  "conv_id": "3949e0d8"
}
```

### `POST /api/chat/resume`

用途：

- 在收到 `permission_required` 后，恢复危险工具执行链路
- 当 `always_allow=true` 时，会把当前工具名写入 `siliconflow/config/permission_settings.json`

请求示例：

```json
{
  "granted": true,
  "always_allow": true,
  "conv_id": "3949e0d8"
}
```

行为说明：

- `granted=false`：向模型回灌“用户拒绝执行”
- `granted=true`：执行当前待审批工具，并继续 follow-up 轮次
- `always_allow=true`：后续同名危险工具默认不再弹审批

### `GET /api/skills`

用途：

- 返回技能列表
- 当前会合并 `skill_registry.json` 与 `skill_settings.json`

### `POST /api/skills/toggle`

用途：

- 切换单个技能的启用状态

### `PATCH /api/skills/:name`

用途：

- 更新单个技能配置
- 当前主要用于 `api_key_ref / env / enabled`

### `GET /api/routing`

用途：

- 返回模型路由配置

### `POST /api/routing`

用途：

- 保存模型路由配置

当前 Rust 路由策略：

- `enabled=false`：不路由，直接使用当前激活模型
- 短且非技术问题：`easy`
- 长文本或明显工程类请求：`hard`
- 其余默认：`medium`

说明：

- 当前仍是启发式分类，不是 Python 版的 Ollama/FunctionGemma 分类器
- tier 选中的模型请求失败时，会按候选模型链回退，并发出 `failover_step`

### `GET /api/token-usage`

用途：

- 返回当前 token 统计快照
- 当前直接读取 `siliconflow/data/token_usage.json`

### `GET /api/security-audit`

用途：

- 返回基础安全审计结果
- 当前主要检查 provider 是否缺少 API Key

### `GET /api/auth-profiles`

用途：

- 返回 auth profiles 配置

### `GET /api/runtime-health`

用途：

- 返回运行时概览
- 当前包含 active conversation、模型数、启用技能数

### `GET /api/failover/recent`

用途：

- 返回最近 failover 事件
- 当前已返回 Rust 运行期内记录的最近 failover 明细

### `GET /api/observability/summary`

用途：

- 返回错误数、failover 成功率、执行事件数等摘要

### `GET /api/observability/events`

用途：

- 返回执行事件列表
- 当前读取 `siliconflow/data/logs/*.jsonl`

### `GET /api/lark/config`

用途：

- 返回 Lark 当前配置
- 只暴露 `app_id` 与 `has_app_secret`

### `POST /api/lark/config`

用途：

- 保存 Lark 配置到 `siliconflow/config/.env`

### `GET /api/terminal/cwd`

用途：

- 返回当前终端工作目录配置

### `POST /api/terminal/cwd`

用途：

- 更新当前终端工作目录配置
- 当前会校验目录存在后写入 `siliconflow/config/terminal.json`

### `WS /api/voice/bridge`

用途：

- Rust 版语音桥入口
- Rust 直接接管前端 WebSocket 连接
- 底层通过 `webapp/backend/voice_worker.py` 调用 Python 语音运行时

当前支持：

- 二进制音频帧上传
- `debug_config` 设置调试模式与当前 `conv_id`
- `debug_inject_text` 直接注入一条文本，跳过麦克风与 ASR
- `end_utterance` 手动结束当前语音段
- `abort` 中断当前会话的聊天生成
- ASR 结果会继续进入 Rust `/api/chat`
- TTS 使用句子级 flush，worker 再以多个 `audio_stream` 音频块回传
- 会额外发 `voice_session_state`，前端可据此展示当前语音会话绑定的 `conv_id` 与阶段

示例：

```json
{
  "type": "debug_config",
  "bypass_wakeword": true,
  "conv_id": "3949e0d8"
}
```

语音状态事件示例：

```json
{
  "type": "voice_session_state",
  "conv_id": "3949e0d8",
  "phase": "processing",
  "source": "chat_start"
}
```
