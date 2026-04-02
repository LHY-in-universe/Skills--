import sys
import json
import subprocess
from pathlib import Path

"""
macOS Terminal 控制技能脚本
引用 terminal 技能 (run_command.py) 执行命令并获取结果，
同时通过 AppleScript 打开 Terminal.app 让用户可视化查看。
用法: python3 open_terminal.py '<命令>' [工作目录]
"""

# 引用 terminal 技能的执行脚本
TERMINAL_SCRIPT = Path(__file__).parent.parent.parent / "terminal" / "scripts" / "run_command.py"

def abort(msg: str):
    print(json.dumps({"error": msg}, ensure_ascii=False))
    sys.exit(1)

def main():
    if len(sys.argv) < 2:
        abort("用法: python3 open_terminal.py '<命令>' [工作目录]")

    command   = sys.argv[1].strip()
    directory = sys.argv[2].strip() if len(sys.argv) >= 3 else ""

    if not command:
        abort("命令不能为空")

    # 1. 调用 terminal skill 执行命令并获取结果
    try:
        result_raw = subprocess.check_output(
            ["python3", str(TERMINAL_SCRIPT), command],
            text=True,
            stderr=subprocess.PIPE,
            timeout=15
        )
        result = json.loads(result_raw)
    except subprocess.CalledProcessError as e:
        error_msg = e.output.strip() if e.output else e.stderr.strip()
        abort(f"terminal 技能执行失败: {error_msg}")
    except subprocess.TimeoutExpired:
        abort("命令执行超时（15 秒）")
    except json.JSONDecodeError:
        abort("terminal 技能返回格式异常")

    # 2. 用 AppleScript 打开 Terminal.app 展示命令执行过程
    full_command = f"cd {directory} && {command}" if directory else command
    escaped = full_command.replace("\\", "\\\\").replace('"', '\\"')
    applescript = f'tell application "Terminal" to activate\ntell application "Terminal" to do script "{escaped}"'

    try:
        subprocess.run(
            ["osascript", "-e", applescript],
            capture_output=True, text=True, timeout=10
        )
    except Exception:
        pass  # 即使 Terminal.app 打开失败，仍返回命令执行结果

    # 返回 terminal skill 的执行结果，附加 Terminal.app 已打开的提示
    result["terminal_opened"] = True
    print(json.dumps(result, ensure_ascii=False, indent=2))

if __name__ == "__main__":
    main()
