---
name: macOS Terminal Controller (Python)
description: 引用 terminal 技能执行命令，同时打开 Terminal.app 让用户可视化查看运行过程。
---

# macOS 终端控制技能 (macOS Terminal Controller)

本技能**引用 [terminal 技能](../terminal/SKILL.md)**：命令执行逻辑由 `terminal/scripts/run_command.py` 完成（含沙箱限制与白名单校验），本技能在此基础上额外打开 macOS Terminal.app，让用户直观看到命令运行过程。

## 与 terminal 技能的关系

| | terminal | macos_terminal |
|---|---|---|
| 执行逻辑 | `run_command.py` | 引用 `run_command.py` |
| 用户可见 | 否（静默） | 是（Terminal.app 窗口） |
| 沙箱限制 | `test/` 目录 | 同上（继承） |
| 命令白名单 | 是 | 同上（继承） |

## 操作权限申请 (Permission Required)

**在执行任何命令之前，必须先向用户申请权限。** 流程如下：

1. **说明意图**：用自然语言告知用户你准备执行的命令是什么。
2. **解释原因**：说明为什么需要执行这条命令，它将产生什么效果（创建文件、删除内容、修改目录结构等）。
3. **等待确认**：明确询问用户是否同意，**不得在用户回复之前执行任何命令**。
4. **获得同意后**：用户明确表示同意（如"同意"、"可以"、"yes"、"继续"）后，才可执行。
5. **遭到拒绝时**：立即停止，不得寻找替代命令绕过限制。

**申请格式示例：**
> 我需要在 Terminal.app 中执行以下命令：
> ```
> <具体命令>
> ```
> **原因**：<详细说明此命令的目的和预期效果>
> 请问您是否同意执行？

---

## 指令说明 (Instructions)

```
python3 scripts/open_terminal.py '<命令>' [工作目录]
```

## 使用示例

> 任务：在 Terminal.app 中运行 ls -la。
> 1. 执行 `python3 scripts/open_terminal.py 'ls -la'`

> 任务：在 ~/Desktop 目录下运行 python3 test.py。
> 1. 执行 `python3 scripts/open_terminal.py 'python3 test.py' '~/Desktop'`
