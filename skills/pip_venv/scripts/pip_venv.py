import sys
import json
import argparse
import subprocess
from pathlib import Path

"""
pip_venv 技能脚本
在指定虚拟环境中执行 pip 操作（install / uninstall / list / show）
"""

DEFAULT_VENV = Path(__file__).resolve().parents[3] / "webapp" / "backend" / "venv"

ALLOWED_ACTIONS = {"install", "uninstall", "list", "show"}

# pip flags that are allowed for install
ALLOWED_INSTALL_FLAGS = {"-r", "--requirement", "--upgrade", "-U", "--quiet", "-q",
                          "--no-cache-dir", "--index-url", "-i", "--extra-index-url"}

def abort(msg: str):
    print(json.dumps({"error": msg}, ensure_ascii=False))
    sys.exit(1)

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--args", default="{}", help="JSON 参数")
    parsed = parser.parse_args()

    try:
        args = json.loads(parsed.args)
    except Exception:
        abort("参数 JSON 解析失败")

    action = args.get("action", "install").strip().lower()
    if action not in ALLOWED_ACTIONS:
        abort(f"不支持的操作 '{action}'，允许: {', '.join(sorted(ALLOWED_ACTIONS))}")

    # Resolve venv
    venv_path_arg = args.get("venv_path", "").strip()
    if venv_path_arg:
        venv_dir = Path(venv_path_arg).expanduser().resolve()
    else:
        venv_dir = DEFAULT_VENV.resolve()

    pip_bin = venv_dir / "bin" / "pip"
    if not pip_bin.exists():
        abort(f"找不到 pip：{pip_bin}（请确认虚拟环境路径）")

    if action == "list":
        cmd = [str(pip_bin), "list", "--format=columns"]

    elif action == "show":
        package = args.get("package", "").strip()
        if not package:
            abort("show 操作需要提供 package 参数")
        cmd = [str(pip_bin), "show", package]

    elif action == "install":
        package = args.get("package", "").strip()
        if not package:
            abort("install 操作需要提供 package 参数")
        # Basic sanity check: no shell metacharacters
        for ch in (";", "&", "|", "`", "$", ">", "<"):
            if ch in package:
                abort(f"package 参数包含非法字符 '{ch}'")
        cmd = [str(pip_bin), "install"] + package.split()

    elif action == "uninstall":
        package = args.get("package", "").strip()
        if not package:
            abort("uninstall 操作需要提供 package 参数")
        for ch in (";", "&", "|", "`", "$", ">", "<"):
            if ch in package:
                abort(f"package 参数包含非法字符 '{ch}'")
        cmd = [str(pip_bin), "uninstall", "-y"] + package.split()

    try:
        result = subprocess.run(
            cmd,
            capture_output=True,
            text=True,
            timeout=120,
        )
        output = {
            "ok": result.returncode == 0,
            "action": action,
            "venv": str(venv_dir),
            "stdout": result.stdout.strip(),
            "stderr": result.stderr.strip(),
            "returncode": result.returncode,
        }
        print(json.dumps(output, ensure_ascii=False, indent=2))
    except subprocess.TimeoutExpired:
        abort("pip 操作超时（限制 120 秒）")

if __name__ == "__main__":
    main()
