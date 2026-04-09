# Skills探索 — 工业级 AI Agent 协同系统

本系统是一个基于 Python 的高性能 AI 编排器，通过飞书 (Lark) WebSocket 桥接和 Web 端双重入口，实现了“文档即技能”的动态扩展能力。

## 🌟 核心特性

- **🚀 飞书深度集成**：支持通过 WebSocket 长连接接入飞书，无需公网 IP 即可实现消息实时收发。具备 CardKit 2.0 流式打字机输出和工具执行实时状态显示。
- **📜 动态技能生态 (Markdown Driven)**：深度兼容 OpenClaw 规范。只需在目录下放入 `SKILL.md`，即可将文档直接转化为可执行的 AI 技能。
- **🛡️ 工业级并发安全**：采用“无状态”对话上下文隔离技术，支持 Web 端与飞书端多人同时并发操作，各会话轨迹互不干扰。
- **🧪 系统诊断 (Skill Doctor)**：启动时自动扫描环境依赖（ffmpeg, gh, python 等），确保所有技能处于 Ready 状态。
- **🔒 全方位安全加固**：
    - **Injection Defense**：使用 `shlex.quote` 转义技术防御针对 Shell 技能的恶意指令注入。
    - **Human-in-the-Loop**：高危操作（如 Shell 执行、文件改写）需用户在 Web 端手动审批。

## 🏗️ 项目架构

```
Skills探索/
├── webapp/
│   ├── backend/            # FastAPI 核心编排器
│   │   ├── main.py         # API 服务入口
│   │   ├── orchestrator.py # 并发消息引擎（双向通信总线）
│   │   ├── skill_loader.py # Markdown 及 Native 技能扫描器、健康诊断引擎
│   │   └── lark_bridge.py  # 飞书 WebSocket 桥接与 UI 渲染器
│   └── frontend/           # Vue 3 展示端（权限审批中心、管理后台）
├── skills/                 # 动态技能库
│   ├── github/             # [MD] GitHub 仓库管理 (SKILL.md)
│   ├── summarize/          # [MD] 万能网页总结 (SKILL.md)
│   ├── video-frames/       # [MD] 视频帧提取 (SKILL.md)
│   └── ...                 # 内置 Native 技能 (FileEditor, Terminal, etc.)
└── siliconflow/            # 配置与状态中心
    ├── .env                # 包含 LARK_APP_ID 等核心密钥
    └── memory.json         # RAG 长期记忆存储
```

## 🚀 快速开始

### 1. 安装依赖
```bash
cd webapp/backend
python3 -m venv venv
source venv/bin/activate
pip install -r requirements.txt
```

### 2. 配置环境变量
在 `siliconflow/.env` 中补充以下信息：
```bash
# 核心 API 配置
SILICONFLOW_API_KEY=your_api_key
SILICONFLOW_API_URL=https://api.siliconflow.cn/v1/chat/completions

# 飞书集成配置 (可选)
LARK_APP_ID=cli_xxxxxxxxxxxx
LARK_APP_SECRET=xxxxxxxxxxxxxxxxxxxxxxxx
```

### 3. 启动系统
```bash
# 后端启动（自动开启飞书桥接与健康诊断）
python main.py

# 前端启动（审批操作中心）
cd ../frontend && npm run dev
```

## 🛠️ 技能系统说明

### Markdown 技能 (OpenClaw Mode)
只需在 `skills/` 目录下创建一个文件夹并包含 `SKILL.md`：
- 系统会自动解析文档中的 YAML 元数据。
- AI 调用的参数会自动映射到文档中的 Bash 命令模板中。
- 所有动态参数均经过 `shlex` 安全转义。

### Native 技能 (Python Mode)
通过 `skill_manifest.json` 定义参数规范，由 `subprocess` 安全调用本地 Python 脚本。

## 🛡️ 安全机制说明

- **并发轨道**：每个飞书 `chat_id` 自动映射一个独特的内部会话 ID，防止多用户对话串线。
- **注入防御**：所有的 Bash 命令拼接均通过 `shlex.quote` 过滤特殊字符。
- **权限拦截**：AI 尝试执行包含高危指令 (如 `rm`) 的 Shell 脚本时，会先在 Web 端/飞书端触发挂起，等待人工确认。

---

## 开发与贡献

1. 运行 `verify_skills.py` 检查当前系统的技能就绪情况。
2. 每一个新增的技能都建议配有相应的文档说明和依赖安装脚本。
