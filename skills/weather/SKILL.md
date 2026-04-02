---
name: Weather Explorer (Python)
description: 获取指定城市的实时天气和预报。
---

# 天气查看技能 (Weather Explorer Skill)

本技能用于快速获取全球任何城市的当前天气状况及未来预报。

## 指令说明 (Instructions)

1.  **获取天气数据**：
    -   使用 `python3 scripts/get_weather.py`。
    -   提取输出中的温度、状态等信息。

2.  **可视化验证**：
    -   如果需要视觉反馈，可以调用 `browser_subagent` 访问相关天气网站。

## 使用示例

> 任务：查看上海的天气。
> 1. 执行 `python3 scripts/get_weather.py "上海"`。
