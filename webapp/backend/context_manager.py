import json
import uuid
import time
import requests
from pathlib import Path
from typing import List, Dict, Any, Optional, Tuple

COMPRESSION_THRESHOLD = 20
COMPRESSION_WINDOW    = 10
KEEP_RECENT           = 6
RAG_TOP_K             = 3
SIMILARITY_FLOOR      = 0.3
EMBEDDING_MODEL       = "BAAI/bge-m3"

STANDARD_ROLES = {"system", "user", "assistant", "tool"}
STRIP_KEYS     = {"reasoning_content", "is_summary"}


class ContextManager:
    def __init__(
        self,
        api_key: str,
        api_base_url: str,
        embeddings_path: Path,
        chat_model: str,
        memory_manager=None,
    ):
        self.api_key        = api_key
        self.api_base_url   = api_base_url.rstrip("/")
        self.embeddings_path = embeddings_path
        self.chat_model     = chat_model
        self.memory_manager = memory_manager

    # ── Embedding API ────────────────────────────────────────────

    def _get_embedding(self, text: str) -> Optional[List[float]]:
        try:
            resp = requests.post(
                f"{self.api_base_url}/embeddings",
                headers={
                    "Authorization": f"Bearer {self.api_key}",
                    "Content-Type": "application/json",
                },
                json={"model": EMBEDDING_MODEL, "input": text},
                timeout=30,
            )
            resp.raise_for_status()
            data = resp.json()
            if data.get("usage") and getattr(self, "token_tracker", None):
                u = data["usage"]
                self.token_tracker.record(
                    "embedding_ctx", EMBEDDING_MODEL,
                    u.get("prompt_tokens", u.get("total_tokens", 0)), 0
                )
            return data["data"][0]["embedding"]
        except Exception:
            return None

    def _cosine_similarity(self, a: List[float], b: List[float]) -> float:
        try:
            import numpy as np
            va, vb = np.array(a, dtype=float), np.array(b, dtype=float)
            denom = np.linalg.norm(va) * np.linalg.norm(vb)
            if denom == 0:
                return 0.0
            return float(np.dot(va, vb) / denom)
        except ImportError:
            return 0.0

    # ── Persistence ──────────────────────────────────────────────

    def _load_index(self) -> Dict[str, Any]:
        try:
            if self.embeddings_path.exists():
                return json.loads(self.embeddings_path.read_text(encoding="utf-8"))
        except Exception:
            pass
        return {"version": 1, "conversations": {}}

    def _save_index(self, index: Dict[str, Any]) -> None:
        try:
            self.embeddings_path.write_text(
                json.dumps(index, ensure_ascii=False, indent=2),
                encoding="utf-8",
            )
        except Exception:
            pass

    # ── Tool-call pair grouping ───────────────────────────────────

    def _group_into_pairs(self, messages: List[Dict]) -> List[List[Dict]]:
        """Group messages into atomic units, keeping tool-call pairs together."""
        groups: List[List[Dict]] = []
        i = 0
        while i < len(messages):
            msg = messages[i]
            if msg.get("role") == "assistant" and msg.get("tool_calls"):
                # Collect this assistant msg + all following tool result msgs
                group = [msg]
                i += 1
                while i < len(messages) and messages[i].get("role") == "tool":
                    group.append(messages[i])
                    i += 1
                groups.append(group)
            else:
                groups.append([msg])
                i += 1
        return groups

    # ── Text rendering for summarisation ─────────────────────────

    def _render_window_as_text(self, groups: List[List[Dict]]) -> str:
        lines: List[str] = []
        for group in groups:
            for msg in group:
                role = msg.get("role", "")
                content = msg.get("content") or ""
                if role == "user":
                    lines.append(f"用户: {content}")
                elif role == "assistant":
                    if msg.get("tool_calls"):
                        for tc in msg["tool_calls"]:
                            fn = tc.get("function", {})
                            lines.append(f"助手调用工具 {fn.get('name', '?')}: {fn.get('arguments', '')}")
                        if content:
                            lines.append(f"助手: {content}")
                    else:
                        lines.append(f"助手: {content}")
                elif role == "tool":
                    lines.append(f"工具结果: {content[:300]}")
        return "\n".join(lines)

    # ── Summary rules ─────────────────────────────────────────────

    def _load_summary_rules(self) -> str:
        """Load summary rules from siliconflow/summary_rules.json and format for prompt injection."""
        rules_path = self.embeddings_path.parent / "summary_rules.json"
        try:
            data = json.loads(rules_path.read_text(encoding="utf-8"))
            rules = data.get("rules", [])
            if rules:
                return "\n\n注意事项（严格遵守）：\n" + "\n".join(f"- {r}" for r in rules)
        except Exception:
            pass
        return ""

    # ── Compression ──────────────────────────────────────────────

    def _compress_window(
        self,
        conv_id: str,
        groups: List[List[Dict]],
        chat_model: str,
        source_start: int,
        source_end: int,
    ) -> Optional[Dict]:
        window_text = self._render_window_as_text(groups)
        try:
            resp = requests.post(
                f"{self.api_base_url}/chat/completions",
                headers={
                    "Authorization": f"Bearer {self.api_key}",
                    "Content-Type": "application/json",
                },
                json={
                    "model": chat_model,
                    "messages": [
                        {
                            "role": "system",
                            "content": (
                                "你是一个对话摘要助手。"
                                "请将下面的对话历史压缩为一段简洁的中文摘要，"
                                "保留关键信息、工具调用结果和重要决策。"
                                + self._load_summary_rules()
                            ),
                        },
                        {"role": "user", "content": window_text},
                    ],
                },
                timeout=60,
            )
            resp.raise_for_status()
            data = resp.json()
            summary_text = data["choices"][0]["message"]["content"]
            if data.get("usage") and getattr(self, "token_tracker", None):
                u = data["usage"]
                self.token_tracker.record(
                    "compress", chat_model,
                    u.get("prompt_tokens", 0), u.get("completion_tokens", 0)
                )
        except Exception:
            return None

        # Build summary message
        summary_msg = {
            "role": "assistant",
            "content": f"[对话历史摘要]\n{summary_text}",
            "is_summary": True,
        }

        # Embed and store in RAG index
        embedding = self._get_embedding(summary_text)
        if embedding is not None:
            index = self._load_index()
            convs = index.setdefault("conversations", {})
            chunks = convs.setdefault(conv_id, {}).setdefault("chunks", [])
            chunks.append({
                "chunk_id": uuid.uuid4().hex[:8],
                "created_at": time.time(),
                "text": summary_text,
                "source_message_range": [source_start, source_end],
                "embedding": embedding,
            })
            self._save_index(index)

        return summary_msg

    def maybe_compress(
        self,
        conv_id: str,
        messages: List[Dict],
        chat_model: str,
    ) -> List[Dict]:
        non_system = messages[1:]
        if len(non_system) <= COMPRESSION_THRESHOLD:
            return messages

        # Keep tail verbatim; compress from the front of the compressible window
        compressible = non_system[:-KEEP_RECENT] if KEEP_RECENT > 0 else non_system
        tail = non_system[-KEEP_RECENT:] if KEEP_RECENT > 0 else []

        groups = self._group_into_pairs(compressible)

        # Accumulate groups up to COMPRESSION_WINDOW raw messages
        window_groups: List[List[Dict]] = []
        count = 0
        for g in groups:
            window_groups.append(g)
            count += len(g)
            if count >= COMPRESSION_WINDOW:
                break

        # source_start/end are 1-based indices in the original messages list
        source_start = 1
        source_end   = source_start + count - 1

        summary_msg = self._compress_window(
            conv_id, window_groups, chat_model, source_start, source_end
        )
        if summary_msg is None:
            return messages  # compression failed — keep original

        remaining_compressible = compressible[count:]
        return [messages[0], summary_msg] + remaining_compressible + tail

    # ── RAG retrieval ────────────────────────────────────────────

    def retrieve_context(
        self,
        conv_id: str,
        query: str,
        top_k: int = RAG_TOP_K,
    ) -> str:
        if not query:
            return ""
        index = self._load_index()
        chunks = index.get("conversations", {}).get(conv_id, {}).get("chunks", [])
        if not chunks:
            return ""

        query_vec = self._get_embedding(query)
        if query_vec is None:
            return ""

        scored = [
            (self._cosine_similarity(query_vec, c["embedding"]), c["text"])
            for c in chunks
            if c.get("embedding")
        ]
        scored.sort(key=lambda x: x[0], reverse=True)

        relevant = [(score, text) for score, text in scored[:top_k] if score >= SIMILARITY_FLOOR]
        if not relevant:
            return ""

        return "\n\n".join(
            f"[历史摘要片段 {i + 1}]\n{text}"
            for i, (_, text) in enumerate(relevant)
        )

    # ── Message cleaning ─────────────────────────────────────────

    def _clean_message(self, msg: Dict) -> Dict:
        """Strip non-standard fields before sending to API."""
        return {k: v for k, v in msg.items() if k not in STRIP_KEYS}

    # ── Public entry point ────────────────────────────────────────

    def prepare_messages(
        self,
        conv_id: str,
        messages: List[Dict],
        user_query: str,
        chat_model: str,
    ) -> Tuple[List[Dict], List[Dict]]:
        # 1. Compress if needed
        store_msgs = self.maybe_compress(conv_id, messages, chat_model)

        # 2. Retrieve relevant RAG context (conversation summaries)
        rag_block = self.retrieve_context(conv_id, user_query)

        # 3. Build API messages
        api_msgs = [self._clean_message(m) for m in store_msgs]

        # 4. Inject long-term memory into system prompt (per-query, freshly retrieved)
        if self.memory_manager and api_msgs:
            memory_block = self.memory_manager.search(user_query)
            if memory_block:
                api_msgs[0] = {
                    **api_msgs[0],
                    "content": api_msgs[0]["content"] + f"\n\n[相关长期记忆]\n{memory_block}",
                }

        # 5. Inject conversation RAG before last user msg
        if rag_block:
            rag_msg = {
                "role": "user",
                "content": f"[相关历史上下文]\n{rag_block}",
            }
            api_msgs = api_msgs[:-1] + [rag_msg] + api_msgs[-1:]

        return api_msgs, store_msgs

    # ── Cleanup ───────────────────────────────────────────────────

    def drop_conversation(self, conv_id: str) -> None:
        index = self._load_index()
        index.get("conversations", {}).pop(conv_id, None)
        self._save_index(index)
