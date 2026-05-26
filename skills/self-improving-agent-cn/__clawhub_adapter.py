#!/usr/bin/env python3
import json
import os
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent
MANIFEST = json.loads((ROOT / "skill_manifest.json").read_text(encoding="utf-8"))


def load_payload():
    raw = os.environ.get("SKILL_ARGS_JSON")
    if raw:
        try:
            return json.loads(raw)
        except Exception:
            return {}
    for arg in sys.argv[1:]:
        if arg.startswith("--args="):
            try:
                return json.loads(arg.split("=", 1)[1])
            except Exception:
                return {}
    return {}


def append_arg(cmd, key, value):
    flag = f"--{key.replace('_', '-')}"
    if isinstance(value, bool):
        if value:
            cmd.append(flag)
        return
    if isinstance(value, list):
        for item in value:
            append_arg(cmd, key, item)
        return
    if value is None:
        return
    cmd.extend([flag, str(value)])


def main():
    payload = load_payload()
    actions = MANIFEST.get("adapter_actions", {})
    action = payload.get("action")
    if not action:
        print(f"Missing action. Available: {', '.join(actions.keys())}", file=sys.stderr)
        return 1
    if action not in actions:
        print(f"Unknown action: {action}. Available: {', '.join(actions.keys())}", file=sys.stderr)
        return 1

    args_obj = payload.get("args")
    if not isinstance(args_obj, dict):
        args_obj = {k: v for k, v in payload.items() if k != "action"}

    script_path = ROOT / actions[action]
    cmd = ["python3" if script_path.suffix == ".py" else "bash", str(script_path)]
    for key, value in args_obj.items():
        append_arg(cmd, key, value)

    result = subprocess.run(cmd, capture_output=True, text=True)
    output = (result.stdout or "").strip() or (result.stderr or "").strip()
    if output:
        print(output)
    return result.returncode


if __name__ == "__main__":
    raise SystemExit(main())
