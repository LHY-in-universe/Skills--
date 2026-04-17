---
name: SiliconFlow AI Assistant (Orchestrator)
description: 交互式聊天，智能调用本地技能（天气、时间、系统监控、文件编辑、终端控制、数学计算、视觉分析等）。
---

# SiliconFlow 技能编排器

核心编排技能：与远程大模型对话，模型可主动调用本地技能脚本。

## 使用方式

```bash
# 启动 CLI 对话
python3 siliconflow/scripts/chat.py

# 对话中切换模型
/model
```

## 当前可用技能

| 工具名 | 技能目录 | 说明 |
|--------|----------|------|
| `get_weather` | weather | 查询指定城市实时天气和预报 |
| `get_current_time` | clock | 获取当前日期、时刻和星期 |
| `get_system_info` | system_monitor | 获取 CPU、内存、磁盘、运行时间 |
| `file_editor` | file_editor | 在项目目录内列出、读取、写入、替换文件 |
| `run_terminal` | terminal | 在 test/ 沙箱内执行白名单终端命令 |
| `write_python` | python_writer | 安全写入 Python 文件（路径 + 语法 + 安全校验） |
| `monte_carlo_integration` | monte_carlo | 蒙特卡洛定积分估算，支持五种采样策略 |
| `summary_rules` | summary_rules | 规则摘要生成 |
| `pip_venv` | pip_venv | 虚拟环境内 pip 包管理 |
| `vision_analyze` | vision_analyze | 图像视觉分析 |
| `video_frames` | video-frames | 视频帧提取（ffmpeg） |
| `skill_manager` | skill_manager | 自动扫描 + 注册新技能 |

## 扩展新技能

1. 在 `skills/` 下新建目录，添加 `SKILL.md`（描述）和 `scripts/`（执行脚本）
2. `skill_manager` 会在启动时自动扫描并注册到 `siliconflow/data/skill_registry.json`
3. 在 Rust 后端中，通过 `tool_service.rs` 注册工具定义

## 核心流程

```
用户输入 → 模型推理 → tool_calls → 本地脚本执行 → 结果回传 → 模型总结 → 输出
```

- 安全工具（天气、时间等）自动执行
- 危险工具（终端、文件写入）需要用户确认
