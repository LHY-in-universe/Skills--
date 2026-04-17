# SiliconFlow CLI 模块

`siliconflow/` 是本仓库的命令行编排模块，提供独立于 Web 界面的终端对话能力。

## 目录结构

```text
siliconflow/
├── config/                 # 配置文件
│   ├── .env                # API Key 等环境变量（需自行创建，勿提交）
│   ├── providers.json      # Provider 定义
│   ├── routing_config.json # 路由策略
│   └── ...                 # 其他配置
├── data/                   # 运行时数据
│   ├── conversations.json  # 对话历史
│   ├── memory.json         # 记忆数据
│   └── token_usage.json    # Token 统计
├── scripts/
│   └── chat.py             # 命令行聊天入口
├── SKILL.md                # 系统提示词与工具说明
└── requirements.txt        # Python 依赖
```

## 快速开始

1. 安装依赖

```bash
pip install -r siliconflow/requirements.txt
pip install openai python-dotenv
```

2. 创建环境变量文件 `siliconflow/config/.env`

```bash
SILICONFLOW_API_KEY=your_key_here
SILICONFLOW_API_URL=https://api.siliconflow.cn/v1/chat/completions
```

3. 启动 CLI 对话

```bash
python3 siliconflow/scripts/chat.py
```

## 说明

- CLI 模块与 Rust 后端共享同一套配置目录（`siliconflow/config/`）和技能目录（`skills/`）
- 模型列表来自 `rust-backend/config/models.json`
- 运行时数据存储在 `siliconflow/data/`，Rust 后端额外使用 SQLite（`siliconflow/data/runtime.db`）
