# SiliconFlow 模块说明

`siliconflow/` 是本仓库的命令行编排模块，提供模型调用、技能执行、对话与记忆数据管理。

## 目录

- [SKILL.md](./SKILL.md)：系统提示词与工具说明。
- [scripts/chat.py](./scripts/chat.py)：命令行聊天入口。
- [requirements.txt](./requirements.txt)：该模块基础依赖。
- `config/.env`：环境变量文件（需自行创建，勿提交）。
- `data/`：对话、记忆、token 统计等持久化数据。

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

# 可选
DEEPSEEK_API_KEY=your_deepseek_key
DEEPSEEK_API_URL=https://api.deepseek.com/v1/chat/completions
```

3. 启动 CLI 对话

```bash
python3 siliconflow/scripts/chat.py
```

## 常见说明

- `scripts/chat.py` 使用同一套技能目录（`skills/`）和数据目录（`siliconflow/data/`）。
- 模型列表默认来自 `webapp/backend/models.json`。
- 如果 API Key 未配置，启动后调用模型会失败。
