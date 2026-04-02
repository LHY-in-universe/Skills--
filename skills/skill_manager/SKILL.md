---
name: Skill Manager (Python)
description: 启动时自动扫描 skills/ 目录，对新技能脚本进行安全校验，通过后注册到 skill_registry.json。
---

# 技能管理器 (Skill Manager)

本技能由 `chat.py` 在启动时**自动调用**，无需手动触发。

## 工作流程

```
chat.py 启动
    └─ 调用 scan_skills.py
           ├─ 扫描 skills/*/skill_manifest.json
           ├─ 跳过已注册技能（对比 skill_registry.json）
           ├─ 对新技能的所有 .py 文件做安全校验
           │      ├─ AST 语法检查
           │      └─ 危险调用黑名单检查
           ├─ 校验通过 → 写入 skill_registry.json
           └─ 校验失败 → 打印警告，跳过
```

## 如何添加新技能（自动注册流程）

1. 在 `skills/` 下新建技能目录
2. 创建 `SKILL.md` 和 `scripts/<script>.py`
3. 创建 `skill_manifest.json`（见下方格式）
4. 脚本统一使用 `--args=<JSON>` 接收参数，输出 JSON
5. 下次启动 `chat.py` 时自动完成校验与注册

## skill_manifest.json 格式

```json
{
  "tool_name": "my_tool",
  "skill_dir": "my_skill",
  "script": "scripts/main.py",
  "description": "工具描述（模型可见）",
  "parameters": {
    "type": "object",
    "properties": {
      "param1": {"type": "string", "description": "参数说明"}
    },
    "required": ["param1"]
  }
}
```

## 安全校验规则

- AST 语法正确性
- 禁止调用：`eval` `exec` `os.system` `subprocess.run` `__import__` 等（与 python_writer 一致）

## 相关文件

- 注册表：`/Users/lhy/Desktop/Skills探索/siliconflow/skill_registry.json`
- 扫描脚本：`scripts/scan_skills.py`
