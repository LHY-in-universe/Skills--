---
name: System Monitor (Python)
description: 获取本地系统的运行状态，包括 CPU、内存、负载及操作系统信息。
---

# 系统计数器技能 (System Monitor Skill)

本技能允许 AI 实时感知本地主机的运行状况，帮助用户进行性能分析或环境诊断。

## 指令说明 (Instructions)

1.  **获取系统信息**：
    -   使用 `python3 scripts/get_sys_info.py`。
    -   脚本返回一个 JSON 对象，包含 `memory`, `cpu_count`, `uptime` 等。

2.  **场景建议**：
    -   当用户问“系统负载”或“剩余内存”时调用。

## 使用示例

> 任务：查看当前电脑的内存占用情况。
> 1. 执行 `python3 scripts/get_sys_info.py`。
> 2. 提取 `memory.usage_percent` 并告知用户。
