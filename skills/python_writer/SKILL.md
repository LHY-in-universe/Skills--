---
name: Python Writer (Python)
description: 安全写入 Python 文件，对路径、语法、安全、结构规范施加严格限制，全部通过后才写入磁盘。
---

# Python 文件写入技能 (Python Writer Skill)

本技能专用于写入 `.py` 文件，在写入前进行四层校验。任何一项不通过均返回错误列表，**不写入文件**。

## 四层限制

### 1. 路径与范围
- 只允许写入 `/Users/lhy/Desktop/Skills探索/` 目录内
- 文件名必须以 `.py` 结尾
- **不允许覆盖已存在的文件**

### 2. 语法与格式
- 使用 `ast.parse` 校验 Python 语法
- 文件前两行必须包含 UTF-8 编码声明：
  ```python
  # -*- coding: utf-8 -*-
  ```

### 3. 安全限制（禁止以下调用）
`eval` · `exec` · `compile` · `os.system` · `os.popen` · `subprocess.Popen` · `subprocess.run` · `__import__` · `open` · `shutil.rmtree`

### 4. 结构规范
- 必须有**模块级 docstring**（文件第一个语句为三引号字符串）
- 必须有 `if __name__ == "__main__":` 入口守卫
- 函数名必须符合 `snake_case`
- 类名必须符合 `PascalCase`

## 操作权限申请 (Permission Required)

**写入任何 Python 文件之前，必须先向用户申请权限。** 流程如下：

1. **说明意图**：告知用户将要创建的文件名和目标路径。
2. **展示内容**：展示完整的代码内容（或较长代码的核心摘要），让用户清楚写入的是什么。
3. **解释原因**：说明为什么需要创建这个文件，它的用途是什么。
4. **等待确认**：明确询问用户是否同意，**不得在用户回复之前执行脚本**。
5. **获得同意后**：用户明确表示同意（如"同意"、"可以"、"yes"、"继续"）后，才可执行。
6. **遭到拒绝时**：立即停止，不得修改文件名或路径后再次尝试。

**申请格式示例：**
> 我需要创建以下 Python 文件：
> - **路径**：`<完整文件路径>`
> - **代码预览**：
>   ```python
>   <代码内容（前 20 行，超出部分注明"…共 N 行"）>
>   ```
>
> **用途**：<详细说明这个文件的功能和创建原因>
> 请问您是否同意写入？

---

## 指令说明 (Instructions)

```
python3 scripts/write_python.py --folder=<目录> --file=<文件名.py> --content=<代码>
```

代码中的换行用 `\n` 传入，制表符用 `\t` 传入。

## 使用示例

> 任务：在 test/ 目录下写入一个合规的 Python 文件 hello.py。
> 1. 执行：
>    ```
>    python3 scripts/write_python.py \
>      --folder=/Users/lhy/Desktop/Skills探索/test \
>      --file=hello.py \
>      --content="# -*- coding: utf-8 -*-\n\"\"\"示例模块\"\"\"\n\ndef say_hello():\n    print('Hello')\n\nif __name__ == '__main__':\n    say_hello()"
>    ```
