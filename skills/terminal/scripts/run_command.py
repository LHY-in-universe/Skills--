import sys
import json
import shlex
import subprocess
from pathlib import Path

"""
终端命令技能脚本
在 SANDBOX_DIR 内执行白名单命令，禁止路径穿越和危险操作
"""

SANDBOX_DIR = Path("/Users/lhy/Desktop/Skills探索/test").resolve()

# 允许的命令白名单
ALLOWED_COMMANDS = {
    "ls", "ll", "cat", "head", "tail", "wc", "grep", "find",
    "echo", "printf", "pwd",
    "mkdir", "touch", "cp", "mv", "rm",
    "sort", "uniq", "cut", "awk", "sed",
    "python3", 
}

# 禁止出现的危险模式
BLOCKED_PATTERNS = [
    "$(", "`",          # 命令替换
    "&&", "||", ";",    # 命令链（防止绕过）
    ">", ">>", "<",     # 重定向（防止写到沙箱外）
    "|",                # 管道（允许后可组合危险命令）
    "../",              # 路径穿越
]

def abort(msg: str):
    print(json.dumps({"error": msg}, ensure_ascii=False))
    sys.exit(1)

def main():
    if len(sys.argv) < 2:
        abort("用法: python3 run_command.py '<命令>'")

    raw = sys.argv[1].strip()

    # 检查危险模式
    for pattern in BLOCKED_PATTERNS:
        if pattern in raw:
            abort(f"命令包含禁止字符或操作: '{pattern}'")

    # 解析命令
    try:
        parts = shlex.split(raw)
    except ValueError as e:
        abort(f"命令解析失败: {e}")

    if not parts:
        abort("命令为空")

    base_cmd = parts[0]
    if base_cmd not in ALLOWED_COMMANDS:
        abort(f"命令 '{base_cmd}' 不在允许列表中。允许的命令: {', '.join(sorted(ALLOWED_COMMANDS))}")

    # 检查参数中是否含绝对路径穿越（允许相对路径，但绝对路径必须在沙箱内）
    for arg in parts[1:]:
        p = Path(arg)
        if p.is_absolute():
            resolved = p.resolve()
            if not str(resolved).startswith(str(SANDBOX_DIR)):
                abort(f"绝对路径 '{arg}' 超出沙箱范围 ({SANDBOX_DIR})")

    # 执行命令，工作目录固定为 SANDBOX_DIR
    try:
        result = subprocess.run(
            parts,
            cwd=str(SANDBOX_DIR),
            capture_output=True,
            text=True,
            timeout=15
        )
        output = {
            "ok": result.returncode == 0,
            "command": raw,
            "stdout": result.stdout,
            "stderr": result.stderr,
            "returncode": result.returncode
        }
        print(json.dumps(output, ensure_ascii=False, indent=2))
    except subprocess.TimeoutExpired:
        abort("命令执行超时（限制 15 秒）")
    except FileNotFoundError:
        abort(f"命令未找到: {base_cmd}")

if __name__ == "__main__":
    main()
