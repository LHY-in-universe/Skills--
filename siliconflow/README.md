# 🤖 SiliconFlow AI 编排器 (Python 版)

此模块集成了 SiliconFlow 的大模型能力，允许 AI 助手通过本地技能脚本调用天气、系统信息、时钟、文件编辑、终端控制等工具。

## 目录结构

- **[SKILL.md](SKILL.md)**: AI 指令文档（含完整工具列表）。
- **[scripts/chat.py](scripts/chat.py)**: 主编排脚本（Python）。
- **[.env](.env)**: 用于存储 API Key（请勿提交到 Git）。
- **[requirements.txt](requirements.txt)**: Python 依赖列表。

## 如何使用

1. **准备环境**：确保已安装 Python 3。
2. **安装依赖**：
   ```bash
   pip install -r siliconflow/requirements.txt
   ```
3. **配置 API Key**：在 `siliconflow/.env` 中填写：
   ```
   SILICONFLOW_API_KEY=your_key_here
   ```
4. **启动对话**：
   ```bash
   python3 siliconflow/scripts/chat.py
   ```
5. **切换模型**：对话中输入 `/model`，支持以下模型：
   - `1` Qwen/Qwen3.5-27B
   - `2` deepseek-ai/DeepSeek-V3.2
   - `3` Pro/zai-org/GLM-4.7

## 已注册技能

| 工具名 | 说明 | 限制范围 |
|--------|------|----------|
| `get_weather` | 查询城市实时天气 | 无 |
| `get_current_time` | 获取当前时间与星期 | 无 |
| `get_system_info` | 查看 CPU、内存、磁盘状态 | 无 |
| `file_editor` | 列出、读取、写入、追加、替换文件 | `Skills探索/` |
| `run_terminal` | 执行白名单终端命令（静默） | `Skills探索/test/` |
| `open_terminal` | 打开 Terminal.app 可见窗口执行命令 | 无 |

---
*Created by Deepmind Antigravity*
