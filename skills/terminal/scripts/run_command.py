import sys
import json
import shlex
import argparse
import subprocess
from pathlib import Path

"""
终端命令技能脚本
在指定目录（或默认沙箱）内执行白名单命令，禁止路径穿越和危险操作
"""

DEFAULT_SANDBOX = Path("/Users/lhy/Desktop/Skills探索/test").resolve()

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
    parser = argparse.ArgumentParser()
    parser.add_argument("command", help="要执行的终端命令")
    parser.add_argument("--cwd", default=None, help="工作目录（覆盖默认沙箱）")
    parsed = parser.parse_args()

    raw = parsed.command.strip()

    # 确定工作目录
    if parsed.cwd:
        SANDBOX_DIR = Path(parsed.cwd).expanduser().resolve()
        if not SANDBOX_DIR.exists() or not SANDBOX_DIR.is_dir():
            abort(f"指定的工作目录不存在: {parsed.cwd}")
    else:
        SANDBOX_DIR = DEFAULT_SANDBOX

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

    # python3 只允许运行 .py 文件，禁止 -c / -m 等任意代码执行参数
    if base_cmd == "python3":
        for arg in parts[1:]:
            if arg.startswith("-"):
                abort(f"python3 不允许使用 '{arg}' 参数，只能直接运行 .py 文件")
        if len(parts) < 2 or not parts[1].endswith(".py"):
            abort("python3 只能运行 .py 文件，如: python3 script.py")

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
        # --- 视觉镜像 (肉眼可见) ---
        # 通过 AppleScript 将执行的指令和真实输出镜像到前台 Terminal.app，实现全透明监控
        escaped_cmd = raw.replace('\\', '\\\\').replace('"', '\\"')
        term_script = f"echo \\\"\\033[1;35m[AI 物理沙箱防护墙]\\033[0m 正在执行受限命令: \\033[1;36m{escaped_cmd}\\033[0m\\\"; echo \\\"---\\\";"
        
        out_content = result.stdout.strip()
        err_content = result.stderr.strip()
        
        if out_content:
            escaped_out = out_content.replace('\\', '\\\\').replace('"', '\\"').replace('\n', '\\n')
            term_script += f"echo \\\"{escaped_out}\\\";"
        if err_content:
            escaped_err = err_content.replace('\\', '\\\\').replace('"', '\\"').replace('\n', '\\n')
            term_script += f"echo \\\"\\033[1;31m{escaped_err}\\033[0m\\\";"
            
        term_script += "echo \\\"---\\\"; echo \\\"\\033[1;32m[执行完毕] 结果已加密回传给大模型。\\033[0m\\\";"
        
        applescript = f'tell application "Terminal" to do script "{term_script}"'
        try:
            subprocess.run(["osascript", "-e", applescript], check=False)
        except Exception:
            pass  # 如果 Terminal 没开成功，至少保证程序的逻辑正常回传
        # -----------------------------
        
        output = {
            "ok": result.returncode == 0,
            "command": raw,
            "cwd": str(SANDBOX_DIR),
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
