#!/usr/bin/env python3
import json
import sys
from pathlib import Path

import requests


PROJECT_ROOT = Path(__file__).resolve().parents[2]
ENV_PATH = PROJECT_ROOT / "siliconflow" / "config" / ".env"


def load_env():
    env = {}
    if not ENV_PATH.exists():
        return env
    for line in ENV_PATH.read_text(encoding="utf-8").splitlines():
        line = line.strip()
        if not line or line.startswith("#") or "=" not in line:
            continue
        key, value = line.split("=", 1)
        env[key.strip()] = value.strip()
    return env


def check(name, url, api_key, model_id):
    payload = {
        "model": model_id,
        "messages": [{"role": "user", "content": "ping"}],
        "stream": False,
        "max_tokens": 8,
    }
    headers = {"Content-Type": "application/json"}
    if api_key:
        headers["Authorization"] = f"Bearer {api_key}"
    try:
        session = requests.Session()
        session.trust_env = False
        resp = session.post(url, headers=headers, json=payload, timeout=20)
        body = resp.text[:500]
        return {
            "name": name,
            "url": url,
            "model": model_id,
            "ok": resp.ok,
            "status": resp.status_code,
            "body": body,
        }
    except Exception as exc:
        return {
            "name": name,
            "url": url,
            "model": model_id,
            "ok": False,
            "status": None,
            "body": str(exc),
        }


def main():
    env = load_env()
    checks = [
        (
            "DeepSeek",
            "https://api.deepseek.com/v1/chat/completions",
            env.get("DEEPSEEK_API_KEY", ""),
            "deepseek-v4-pro",
        ),
        (
            "MiMo",
            "https://api.xiaomimimo.com/v1/chat/completions",
            env.get("MIMO_API_KEY", ""),
            "mimo-v2.5-pro",
        ),
        (
            "MiniMax",
            "https://api.minimaxi.com/v1/chat/completions",
            env.get("MINIMAX_API_KEY", ""),
            "MiniMax-M2.7",
        ),
    ]

    results = [check(*item) for item in checks]
    print(json.dumps({"items": results}, ensure_ascii=False, indent=2))

    if all(item["ok"] for item in results):
        return 0
    return 1


if __name__ == "__main__":
    raise SystemExit(main())
