# Rust 后端配置说明

Rust 后端当前仍复用现有项目的配置目录：

```text
siliconflow/config/
webapp/backend/models.json
siliconflow/data/runtime.db
```

## 当前已读取的配置

### `webapp/backend/models.json`

用途：

- 定义模型列表
- 定义 provider
- 定义模型是否启用
- 定义能力与依赖

### `siliconflow/config/runtime_config.json`

用途：

- 定义 embedding 策略
- fast_first_token
- 自纠错
- planner
- retry policy
- vision model map

### `siliconflow/config/.env`

Rust 当前还会读取这些运行时入口变量：

- `NVIDIA_API_KEY`

说明：

- 语音桥已经不再代理 Python 的 WebSocket 路由
- 当前由 Rust 直接启动 `webapp/backend/voice_worker.py`
- Python 解释器优先使用 `webapp/backend/venv/bin/python`

## 后续会继续接管的配置

### `siliconflow/config/providers.json`

- provider 定义
- base url
- auth key 规则

### `siliconflow/config/auth_profiles.json`

- provider 多 key 轮换

### `siliconflow/config/skill_settings.json`

- skill 开关
- skill secret
- skill 环境变量

### `siliconflow/config/permission_settings.json`

用途：

- 存储危险工具的长期授权白名单

当前结构：

```json
{
  "always_allow_tools": [
    "run_terminal"
  ]
}
```

说明：

- 当前粒度是“按工具名全局放行”
- 前端在权限弹窗里勾选 `always_allow` 后，会写入这里
- Rust 后端在判断是否需要再次弹审批时会读取这个文件

## 当前新增运行时数据库

### `siliconflow/data/runtime.db`

用途：

- 存储 Rust 后端接管后的运行时数据
- 当前已存储 conversations 和 conversation_messages

后续会继续纳入：

- token usage
- execution events
- runtime state
- memory entries / embeddings
