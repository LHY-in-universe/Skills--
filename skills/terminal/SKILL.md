---
name: Terminal Runner (Python)
description: 在 /Users/lhy/Desktop/Skills探索/test 沙箱目录内执行白名单终端命令。
---

# 终端命令技能 (Terminal Runner Skill)

本技能允许 AI 在 `/Users/lhy/Desktop/Skills探索/test` 目录内执行指定的终端命令。工作目录固定为该沙箱目录，禁止路径穿越、命令链、重定向等危险操作。

## 操作权限申请 (Permission Required)

**在执行任何命令之前，必须先向用户申请权限。** 流程如下：

1. **说明意图**：用自然语言告知用户你准备执行的命令是什么。
2. **解释原因**：说明为什么需要执行这条命令，它将产生什么效果（创建文件、删除内容、修改目录结构等）。
3. **等待确认**：明确询问用户是否同意，**不得在用户回复之前执行任何命令**。
4. **获得同意后**：用户明确表示同意（如"同意"、"可以"、"yes"、"继续"）后，才可执行。
5. **遭到拒绝时**：立即停止，不得寻找替代命令绕过限制。

**申请格式示例：**
> 我需要执行以下终端命令：
> ```
> <具体命令>
> ```
> **原因**：<详细说明此命令的目的和预期效果>
> 请问您是否同意执行？

---

## 指令说明 (Instructions)

```
python3 scripts/run_command.py '<命令>'
```

命令以字符串形式传入，输出为 JSON，包含 `stdout`、`stderr`、`returncode`。

## 允许的命令

`ls` `cat` `head` `tail` `wc` `grep` `find` `echo` `printf` `pwd`
`mkdir` `touch` `cp` `mv` `rm` `sort` `uniq` `cut` `awk` `sed` `python3`

## 禁止的操作

- 路径穿越（`../`）
- 命令链（`&&` `||` `;`）
- 重定向（`>` `>>` `<`）
- 管道（`|`）
- 命令替换（`$(` `` ` ``）
- 访问沙箱目录外的绝对路径

## 使用示例

> 任务：列出 test 目录下的所有文件。
> 1. 执行 `python3 scripts/run_command.py 'ls -la'`

> 任务：在 test 目录下新建一个 hello.txt 文件。
> 1. 执行 `python3 scripts/run_command.py 'touch hello.txt'`

> 任务：查看 test/notes.txt 的内容。
> 1. 执行 `python3 scripts/run_command.py 'cat notes.txt'`

> 任务：在 test 目录下新建子目录 output。
> 1. 执行 `python3 scripts/run_command.py 'mkdir output'`
