import re
import json
import time
import hashlib
import asyncio
import requests
import httpx
from pathlib import Path
from typing import Dict, Any, Tuple, Optional

CLASSIFY_PROMPT = """\
判断下面问题的难度，只输出一个单词，不得有任何其他内容：
easy   → 闲聊/打招呼/简单事实/简单计算/单步指令/查询时间天气
medium → 代码调试/短函数编写/文本分析/技术问答/多步骤任务/解释概念
hard   → 完整项目/系统实现/科学计算/数值模拟/算法设计/长篇写作/复杂推理/跨领域分析/写一个完整程序/实现一个系统
判断标准：需要生成 50 行以上代码、或涉及多个模块、或需要专业领域知识 → hard
只输出：easy 或 medium 或 hard\
"""


class ModelRouter:
    DEFAULT_CONFIG: Dict[str, Any] = {
        "enabled": False,
        "router_model": "",
        "summary_model": "",
        "tiers": {
            "easy":   "",
            "medium": "",
            "hard":   "",
        },
    }

    CACHE_TTL = 300  # seconds
    TECH_KEYWORDS = (
        "代码", "算法", "设计", "实现", "分析", "证明", "调试",
        "函数", "架构", "系统", "数据库", "网络", "部署",
        "模拟", "计算", "程序", "项目", "写一个", "实现一个",
        "debug", "error", "bug", "api", "code", "script", "python", "java",
    )

    def __init__(self, api_key: str, api_base_url: str, config_path: Path) -> None:
        self.api_key       = api_key
        self.api_base_url  = api_base_url.rstrip("/")
        self.config_path   = config_path
        self._cache: Dict[str, Tuple[str, float]] = {}  # {sha256: (tier, timestamp)}

    # ── Config I/O ────────────────────────────────────────────

    def get_config(self) -> Dict[str, Any]:
        try:
            if self.config_path.exists():
                raw = json.loads(self.config_path.read_text(encoding="utf-8"))
                cfg = dict(self.DEFAULT_CONFIG)
                cfg.update(raw)
                cfg["tiers"] = {**self.DEFAULT_CONFIG["tiers"], **raw.get("tiers", {})}
                return cfg
        except Exception:
            pass
        return dict(self.DEFAULT_CONFIG)

    def save_config(self, config: Dict[str, Any]) -> None:
        self.config_path.write_text(
            json.dumps(config, ensure_ascii=False, indent=2),
            encoding="utf-8",
        )

    # ── Classification ────────────────────────────────────────

    async def classify_async(self, question: str, router_model: str) -> str:
        """Classify using streaming — reads content tokens only, skips thinking.
        Works with Qwen3 thinking models: closes stream as soon as content arrives.
        """
        payload = {
            "model": router_model,
            "messages": [
                {"role": "system", "content": CLASSIFY_PROMPT},
                {"role": "user",   "content": question},
            ],
            "max_tokens": 10,
            "temperature": 0.0,
            "stream": True,
            "stream_options": {"include_usage": True},
        }
        # enable_thinking=False only works on Qwen3 thinking models; other models return 400
        if "qwen3" in router_model.lower():
            payload["enable_thinking"] = False
        collected = ""
        _stream_usage = {}
        try:
            async with httpx.AsyncClient(timeout=60.0, trust_env=False) as client:
                async with client.stream(
                    "POST",
                    f"{self.api_base_url}/chat/completions",
                    headers={
                        "Authorization": f"Bearer {self.api_key}",
                        "Content-Type": "application/json",
                    },
                    json=payload,
                ) as resp:
                    resp.raise_for_status()
                    async for line in resp.aiter_lines():
                        if not line.startswith("data: "):
                            continue
                        raw = line[6:]
                        if raw == "[DONE]":
                            break
                        try:
                            chunk = json.loads(raw)
                        except Exception:
                            continue
                        if chunk.get("usage"):
                            _stream_usage = chunk["usage"]
                        if not chunk.get("choices"):
                            continue
                        delta = chunk["choices"][0].get("delta", {})
                        # Only read `content`, skip `reasoning_content` (thinking)
                        text = delta.get("content") or ""
                        if text:
                            collected += text
            
            if getattr(self, "token_tracker", None) and _stream_usage:
                pt = _stream_usage.get("prompt_tokens", 0)
                ct = _stream_usage.get("completion_tokens", 0)
                self.token_tracker.record("router", router_model, pt, ct)
            elif getattr(self, "token_tracker", None):
                # Fallback heuristic if usage wasn't received
                self.token_tracker.record("router", router_model, len(question)//2 + 50, len(collected)//2 + 1)
                
            tier = self._parse_tier(collected)
            print(f"[Router] {question!r} → raw={collected!r} → {tier}", flush=True)
            return tier
        except Exception as e:
            import traceback
            print(f"[Router] classify error ({type(e).__name__}): {e}", flush=True)
            traceback.print_exc()
            return "medium"

    @staticmethod
    def _parse_tier(content: str) -> str:
        """Extract tier from model output, robust against markdown wrapping."""
        content = content.strip()
        # Strip <think>...</think> blocks (Qwen3 thinking mode)
        content = re.sub(r"<think>[\s\S]*?</think>", "", content).strip()
        # Strip markdown code fences if present
        content = re.sub(r"^```(?:json)?\s*", "", content)
        content = re.sub(r"\s*```$", "", content)
        # Extract JSON object substring
        m = re.search(r"\{[^}]+\}", content)
        if m:
            try:
                data = json.loads(m.group())
                tier = str(data.get("tier", "medium")).lower()
                if tier in ("easy", "medium", "hard"):
                    return tier
            except Exception:
                pass
        # Keyword fallback
        cl = content.lower()
        if "easy" in cl:
            return "easy"
        if "hard" in cl:
            return "hard"
        return "medium"

    # ── Public entry point ────────────────────────────────────

    async def route(self, question: str) -> Tuple[str, str]:
        """Return (model_id, tier).

        Returns ("", "") when routing is disabled or misconfigured.
        Returns ("", tier) when tier is detected but no model is set for it
        (caller should fall back to current_model).
        """
        cfg = self.get_config()
        if not cfg.get("enabled"):
            return "", ""

        router_model = cfg.get("router_model", "").strip()
        if not router_model:
            return "", ""

        # ── Fast-path: short non-technical input → easy immediately ──
        q = question.strip()
        if len(q) <= 20 and not any(kw in q.lower() for kw in self.TECH_KEYWORDS):
            model = cfg["tiers"].get("easy", "").strip()
            print(f"[Router] fast-path easy: {q!r}", flush=True)
            return model, "easy"

        # ── Cache check ───────────────────────────────────────────
        key = hashlib.sha256(question.encode()).hexdigest()
        if key in self._cache:
            cached_tier, ts = self._cache[key]
            if time.time() - ts < self.CACHE_TTL:
                model = cfg["tiers"].get(cached_tier, "").strip()
                print(f"[Router] cache hit → {cached_tier}", flush=True)
                return model, cached_tier

        # ── LLM classification ────────────────────────────────────
        tier  = await self.classify_async(question, router_model)
        self._cache[key] = (tier, time.time())
        model = cfg["tiers"].get(tier, "").strip()
        return model, tier
