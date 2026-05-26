#!/usr/bin/env python3
from pathlib import Path
import json
import os
import sys

ROOT = Path(__file__).resolve().parent


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


def main():
    payload = load_payload()
    focus = payload.get("question") or payload.get("topic") or ""
    text = (ROOT / "SKILL.md").read_text(encoding="utf-8")
    if focus:
        print(f"[Focus] {focus}\n")
    print(text[:8000])
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
