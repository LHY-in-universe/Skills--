import json
from datetime import datetime
from pathlib import Path
from typing import Dict, Any

CALL_TYPES = ("chat", "skill", "router", "compress", "embedding_ctx", "embedding_mem")


class TokenTracker:
    def __init__(self, path: Path):
        self.path = path
        self._data = self._load()

    def _load(self) -> Dict:
        try:
            if self.path.exists():
                return json.loads(self.path.read_text(encoding="utf-8"))
        except Exception:
            pass
        return {"version": 1, "global": self._empty_global(), "daily": {}, "by_model": {}}

    def _empty_global(self) -> Dict:
        return {
            "calls": 0, "prompt": 0, "completion": 0, "total": 0,
            "by_type": {
                t: {"calls": 0, "prompt": 0, "completion": 0, "total": 0}
                for t in CALL_TYPES
            }
        }

    def record(self, call_type: str, model: str, prompt_tokens: int, completion_tokens: int):
        total = prompt_tokens + completion_tokens
        today = datetime.now().strftime("%Y-%m-%d")

        g = self._data["global"]
        g["calls"]      += 1
        g["prompt"]     += prompt_tokens
        g["completion"] += completion_tokens
        g["total"]      += total
        t = g["by_type"].setdefault(call_type, {"calls": 0, "prompt": 0, "completion": 0, "total": 0})
        t["calls"]      += 1
        t["prompt"]     += prompt_tokens
        t["completion"] += completion_tokens
        t["total"]      += total

        d = self._data["daily"].setdefault(today, {
            "calls": 0, "prompt": 0, "completion": 0, "total": 0, "by_type": {}
        })
        d["calls"]      += 1
        d["prompt"]     += prompt_tokens
        d["completion"] += completion_tokens
        d["total"]      += total
        dt = d["by_type"].setdefault(call_type, {"calls": 0, "prompt": 0, "completion": 0, "total": 0})
        dt["calls"]      += 1
        dt["prompt"]     += prompt_tokens
        dt["completion"] += completion_tokens
        dt["total"]      += total

        m = self._data["by_model"].setdefault(model, {"calls": 0, "prompt": 0, "completion": 0, "total": 0})
        m["calls"]      += 1
        m["prompt"]     += prompt_tokens
        m["completion"] += completion_tokens
        m["total"]      += total

        self._save()

    def _save(self):
        try:
            self.path.write_text(
                json.dumps(self._data, ensure_ascii=False, indent=2),
                encoding="utf-8"
            )
        except Exception as e:
            print(f"[Token] save error: {e}", flush=True)

    def get_stats(self) -> Dict[str, Any]:
        return self._data
