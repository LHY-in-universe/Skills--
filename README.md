# Skills 探索

AI 编排平台：Rust 后端 + Vue 前端 + 技能系统 + 语音交互。

## 项目结构

```text
Skills探索/
├── rust-backend/           # Rust 主后端（axum + tokio）
│   ├── config/             # 模型配置（models.json）
│   ├── models/             # 语音 ONNX 模型（gitignore）
│   └── src/                # Rust 源码
├── webapp/
│   └── frontend/           # Vue 3 + Vite 前端
├── skills/                 # 技能目录（SKILL.md / scripts/）
├── siliconflow/            # CLI 编排模块 + 配置 + 数据
│   ├── config/             # 环境变量、路由、provider 等配置
│   └── data/               # 运行时数据（conversations / token_usage 等）
└── docs/                   # 架构、迁移、API 文档
```

## 快速启动

### 1) 环境变量

在 `siliconflow/config/.env` 中配置 API Key：

```bash
SILICONFLOW_API_KEY=your_api_key
SILICONFLOW_API_URL=https://api.siliconflow.cn/v1/chat/completions

# 可选
DEEPSEEK_API_KEY=your_deepseek_key
DEEPSEEK_API_URL=https://api.deepseek.com/v1/chat/completions
```

### 2) 启动后端（Rust）

```bash
cd rust-backend
cargo run --release
```

默认监听：`http://127.0.0.1:18000`

### 3) 启动前端（Vite）

```bash
cd webapp/frontend
npm install
npm run dev
```

默认地址：`http://127.0.0.1:5173`

## 核心功能

- **多模型聊天**：SiliconFlow / DeepSeek / NVIDIA / Kimi，支持自动 failover
- **工具调用**：安全工具自动执行，危险工具走权限确认
- **技能系统**：Markdown 技能 + Native 脚本技能
- **语音交互**：KWS 唤醒词 + VAD 端点检测 + Paraformer ASR + Edge TTS，全部 Rust 进程内
- **会话管理**：SQLite 持久化，支持多会话切换
- **可观测性**：token 统计、执行日志、failover 记录

## 可选组件

### SiliconFlow CLI

```bash
python3 siliconflow/scripts/chat.py
```

详见 [siliconflow/README.md](./siliconflow/README.md)。
