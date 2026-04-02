---
name: File Editor (Python)
description: 在指定文件夹内列出、读取、写入、追加或替换文件内容。
---

# 文件编辑器技能 (File Editor Skill)

本技能允许 AI 在指定文件夹内安全地操作文件，支持列目录、读取、写入、追加、文本替换五种操作。所有操作强制限定在 `/Users/lhy/Desktop/Skills探索` 目录内，任何试图访问该目录外的请求都会被拒绝。

## 操作权限申请 (Permission Required)

**`write`、`append`、`replace` 三种操作会修改或创建文件，执行前必须向用户申请权限。**（`list` 和 `read` 为只读操作，无需申请。）

流程如下：

1. **说明意图**：告知用户将对哪个文件执行什么操作（写入/追加/替换）。
2. **解释原因**：说明为什么需要修改该文件，以及写入的内容或替换的文本是什么。
3. **等待确认**：明确询问用户是否同意，**不得在用户回复之前执行脚本**。
4. **获得同意后**：用户明确表示同意（如"同意"、"可以"、"yes"、"继续"）后，才可执行。
5. **遭到拒绝时**：立即停止，不得用其他操作变通实现相同效果。

**申请格式示例：**
> 我需要对文件执行以下操作：
> - **操作类型**：`write` / `append` / `replace`
> - **目标文件**：`<文件完整路径>`
> - **内容预览**：<写入或替换的具体内容（较长内容可截取前几行）>
>
> **原因**：<详细说明为什么需要做这个修改>
> 请问您是否同意？

---

## 指令说明 (Instructions)

统一调用方式：

```
python3 scripts/edit_file.py --folder <文件夹路径> --op <操作> [其他参数]
```

### 操作列表

| `--op` | 说明 | 必填参数 |
|--------|------|----------|
| `list` | 列出文件夹内所有文件和子目录 | `--folder` |
| `read` | 读取文件内容 | `--folder` `--file` |
| `write` | 写入文件（覆盖或新建） | `--folder` `--file` `--content` |
| `append` | 追加内容到文件末尾 | `--folder` `--file` `--content` |
| `replace` | 替换文件中的指定文本 | `--folder` `--file` `--old` `--new` |

## 使用示例

> 任务：列出 ~/Documents 下的所有文件。
>
> 1. 执行 `python3 scripts/edit_file.py --folder ~/Documents --op list`

> 任务：读取 ~/Documents/notes.txt。
>
> 1. 执行 `python3 scripts/edit_file.py --folder ~/Documents --op read --file notes.txt`

> 任务：新建 ~/Documents/hello.txt，内容为"你好，世界"。
>
> 1. 执行 `python3 scripts/edit_file.py --folder ~/Documents --op write --file hello.txt --content "你好，世界"`

> 任务：将 ~/Documents/notes.txt 中所有"旧内容"替换为"新内容"。
>
> 1. 执行 `python3 scripts/edit_file.py --folder ~/Documents --op replace --file notes.txt --old "旧内容" --new "新内容"`
