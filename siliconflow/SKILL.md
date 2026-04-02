---
name: SiliconFlow AI Assistant (Orchestrator)
description: 交互式聊天，智能调用本地技能（天气、时间、系统监控、文件编辑、终端控制）。
---

# SiliconFlow 技能编排器 (SiliconFlow Orchestrator)

这是核心编排技能，让你与远程大模型对话，并让模型**主动调用**本地 Python 技能脚本。

## 指令说明 (Instructions)

1. **启动对话**：
   - 执行 `python3 scripts/chat.py`。
   - 输入 `/model` 可切换模型。

2. **扩展新技能**：
   - 在 `skills/` 下新建技能目录，添加 `SKILL.md` 和 `scripts/`。
   - 在 `chat.py` 的 `TOOLS` 列表和 `handle_tool_call` 中注册。

## 当前可用技能

| 工具名 | 技能 | 说明 |
|--------|------|------|
| `get_weather` | weather | 查询指定城市实时天气 |
| `get_current_time` | clock | 获取当前日期、时刻和星期 |
| `get_system_info` | system_monitor | 获取 CPU、内存、磁盘、运行时间 |
| `file_editor` | file_editor | 在 Skills探索/ 内列出、读取、写入、替换文件 |
| `run_terminal` | terminal | 在 test/ 沙箱内执行白名单终端命令 |
| `open_terminal` | macos_terminal | 打开 Terminal.app 在可见窗口执行命令 |

## 核心原理

- **发现**：模型在 `TOOLS` 定义中发现可用技能。
- **调用**：模型返回 `tool_calls`。
- **执行**：`chat.py` 的 `handle_tool_call` 调用对应 `.py` 脚本。

## 使用示例

> 任务：查询上海天气。
> 1. 执行 `python3 scripts/chat.py`，输入"上海天气怎么样？"。
