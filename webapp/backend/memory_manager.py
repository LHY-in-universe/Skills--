import re
import json
import time
import uuid
import requests
import numpy as np
from datetime import datetime
from pathlib import Path
from typing import List, Dict, Any, Optional

EMBEDDING_MODEL  = "BAAI/bge-m3"
SIMILARITY_FLOOR = 0.25
MEMORY_TOP_K     = 3
DECAY_DAYS       = 90
COSINE_WEIGHT    = 0.85
DECAY_WEIGHT     = 0.15
MEMORY_MAX_CHUNKS = 200
INDEX_FILE  = "memory_index.json"
VECTOR_FILE = "memory_vectors.npy"


class MemoryManager:
    def __init__(self, memory_dir: Path, api_key: str, api_base_url: str) -> None:
        self.memory_dir    = memory_dir
        self.index_path    = memory_dir.parent / INDEX_FILE
        self.vector_path   = memory_dir.parent / VECTOR_FILE
        self.api_key       = api_key
        self.api_base_url  = api_base_url.rstrip("/")
        self.memory_dir.mkdir(parents=True, exist_ok=True)

    # ── Embedding ─────────────────────────────────────────────

    def _get_embedding(self, text: str, retries: int = 2) -> Optional[List[float]]:
        for attempt in range(retries + 1):
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
                        "embedding_mem", EMBEDDING_MODEL,
                        u.get("prompt_tokens", u.get("total_tokens", 0)), 0
                    )
                return data["data"][0]["embedding"]
            except Exception as e:
                if attempt < retries:
                    time.sleep(1)
                    continue
                print(f"[Memory] embedding error: {e}", flush=True)
                return None

    def _cosine_similarity(self, a: List[float], b: List[float]) -> float:
        va = np.array(a, dtype=float)
        vb = np.array(b, dtype=float)
        denom = np.linalg.norm(va) * np.linalg.norm(vb)
        if denom == 0:
            return 0.0
        return float(np.dot(va, vb) / denom)

    # ── Index I/O ─────────────────────────────────────────────

    def _load_index(self) -> Dict[str, Any]:
        try:
            if self.index_path.exists():
                return json.loads(self.index_path.read_text(encoding="utf-8"))
        except Exception:
            pass
        return {"version": 2, "chunks": []}

    def _save_index(self, index: Dict[str, Any]) -> None:
        try:
            self.index_path.write_text(
                json.dumps(index, ensure_ascii=False, indent=2),
                encoding="utf-8",
            )
        except Exception as e:
            print(f"[Memory] index save error: {e}", flush=True)

    # ── Vector I/O ────────────────────────────────────────────

    def _load_vectors(self) -> Optional[np.ndarray]:
        """Load vector matrix, shape=(N, 1024), dtype=float32."""
        try:
            if self.vector_path.exists():
                return np.load(str(self.vector_path)).astype(np.float32)
        except Exception:
            pass
        return None

    def _save_vectors(self, vecs: np.ndarray) -> None:
        """Save vector matrix as float16 binary (compact)."""
        try:
            np.save(str(self.vector_path), vecs.astype(np.float16))
        except Exception as e:
            print(f"[Memory] vector save error: {e}", flush=True)

    # ── MD parsing ────────────────────────────────────────────

    def _parse_chunks(self, md_text: str, filename: str) -> List[Dict[str, Any]]:
        """Split MD file on '## HH:MM' section headers; each section = one chunk."""
        pattern = re.compile(r'^## (\d{2}:\d{2})(.*?)$', re.MULTILINE)
        matches = list(pattern.finditer(md_text))
        if not matches:
            text = md_text.strip()
            if text:
                return [{"text": text, "filename": filename, "created_at": time.time()}]
            return []

        chunks = []
        for i, m in enumerate(matches):
            start = m.end()
            end = matches[i + 1].start() if i + 1 < len(matches) else len(md_text)
            body = md_text[start:end].strip()
            if body:
                chunks.append({
                    "text": body,
                    "filename": filename,
                    "header": m.group(0).strip(),
                    "created_at": time.time(),
                })
        return chunks

    # ── Eviction ──────────────────────────────────────────────

    def _evict(self, chunks: List[Dict[str, Any]], vecs: np.ndarray) -> tuple:
        """Keep the newest MEMORY_MAX_CHUNKS entries, drop oldest."""
        if len(chunks) <= MEMORY_MAX_CHUNKS:
            return chunks, vecs
        evicted = len(chunks) - MEMORY_MAX_CHUNKS
        print(f"[Memory] evicting {evicted} old chunk(s), keeping {MEMORY_MAX_CHUNKS}", flush=True)
        chunks = chunks[-MEMORY_MAX_CHUNKS:]
        vecs = vecs[-MEMORY_MAX_CHUNKS:]
        return chunks, vecs

    # ── Migration from old memory_embeddings.json ─────────────

    def _migrate_legacy(self) -> None:
        """One-time migration: read old memory_embeddings.json → new index + npy."""
        legacy_path = self.index_path.parent / "memory_embeddings.json"
        if not legacy_path.exists():
            return
        try:
            data = json.loads(legacy_path.read_text(encoding="utf-8"))
            old_chunks = data.get("chunks", [])
            if not old_chunks:
                legacy_path.rename(legacy_path.with_suffix(".json.bak"))
                return

            new_chunks = []
            vecs_list = []
            for c in old_chunks:
                emb = c.get("embedding")
                new_chunks.append({
                    "id":         c.get("id", uuid.uuid4().hex[:8]),
                    "file":       c.get("file", ""),
                    "text":       c.get("text", ""),
                    "created_at": c.get("created_at", time.time()),
                })
                if emb and isinstance(emb, list):
                    vecs_list.append(emb)
                else:
                    vecs_list.append([0.0] * 1024)

            index = {"version": 2, "chunks": new_chunks}
            vecs = np.array(vecs_list, dtype=np.float32)
            self._save_index(index)
            self._save_vectors(vecs)
            legacy_path.rename(legacy_path.with_suffix(".json.bak"))
            print(f"[Memory] migrated {len(new_chunks)} chunks from legacy format", flush=True)
        except Exception as e:
            print(f"[Memory] migration error: {e}", flush=True)

    # ── Write ─────────────────────────────────────────────────

    def append(self, text: str, source: str = "AI手动写入") -> None:
        """Append a memory entry to today's MD file and update the index."""
        if not text.strip():
            return
        today   = datetime.now().strftime("%Y-%m-%d")
        hm      = datetime.now().strftime("%H:%M")
        md_path = self.memory_dir / f"{today}.md"

        entry = f"\n## {hm} [{source}]\n\n{text.strip()}\n"
        with open(md_path, "a", encoding="utf-8") as f:
            f.write(entry)
        print(f"[Memory] appended to {md_path.name}: {text[:60]}...", flush=True)

        embedding = self._get_embedding(text)

        index = self._load_index()
        index["chunks"].append({
            "id":         uuid.uuid4().hex[:8],
            "file":       md_path.name,
            "text":       text.strip(),
            "created_at": time.time(),
        })

        vecs = self._load_vectors()
        new_vec = np.array([embedding if embedding else [0.0] * 1024], dtype=np.float32)
        vecs = new_vec if vecs is None else np.vstack([vecs, new_vec])

        index["chunks"], vecs = self._evict(index["chunks"], vecs)

        self._save_index(index)
        self._save_vectors(vecs)

    # ── Index rebuild ─────────────────────────────────────────

    def index_all(self) -> None:
        """Scan all .md files and rebuild the full index. Called once on startup."""
        # One-time migration from old format
        self._migrate_legacy()

        md_files = sorted(self.memory_dir.glob("*.md"))
        if not md_files:
            return

        index = self._load_index()
        existing_vecs = self._load_vectors()

        indexed_texts = {(c["file"], c["text"][:80]) for c in index["chunks"]}

        new_chunks = []
        new_vecs = []
        for md_path in md_files:
            try:
                md_text = md_path.read_text(encoding="utf-8")
            except Exception:
                continue
            raw_chunks = self._parse_chunks(md_text, md_path.name)
            for rc in raw_chunks:
                key = (rc["filename"], rc["text"][:80])
                if key not in indexed_texts:
                    embedding = self._get_embedding(rc["text"])
                    new_chunks.append({
                        "id":         uuid.uuid4().hex[:8],
                        "file":       rc["filename"],
                        "text":       rc["text"],
                        "created_at": rc["created_at"],
                    })
                    new_vecs.append(embedding if embedding else [0.0] * 1024)
                    indexed_texts.add(key)

        if new_chunks:
            index["chunks"].extend(new_chunks)
            new_arr = np.array(new_vecs, dtype=np.float32)
            all_vecs = new_arr if existing_vecs is None else np.vstack([existing_vecs, new_arr])
            index["chunks"], all_vecs = self._evict(index["chunks"], all_vecs)
            self._save_index(index)
            self._save_vectors(all_vecs)
            print(f"[Memory] indexed {len(new_chunks)} new chunk(s) from {len(md_files)} file(s)", flush=True)
        else:
            total = len(index["chunks"])
            print(f"[Memory] index up-to-date ({total} chunks)", flush=True)

    # ── Search ────────────────────────────────────────────────

    def search(self, query: str, top_k: int = MEMORY_TOP_K) -> str:
        """Vector search with time decay. Returns formatted string for system prompt injection."""
        if not query.strip():
            return ""
        index = self._load_index()
        chunks = index.get("chunks", [])
        vecs = self._load_vectors()
        if not chunks or vecs is None or len(vecs) == 0:
            return ""

        query_vec = self._get_embedding(query)
        if query_vec is None:
            return ""

        qv = np.array(query_vec, dtype=np.float32)
        # Batch cosine similarity via matrix multiplication
        norms = np.linalg.norm(vecs, axis=1) * np.linalg.norm(qv)
        norms = np.where(norms == 0, 1e-9, norms)
        cosines = (vecs @ qv) / norms

        now = time.time()
        scores = []
        for i, c in enumerate(chunks):
            if i >= len(cosines):
                break
            cosine    = float(cosines[i])
            days_old  = (now - c.get("created_at", now)) / 86400
            decay     = max(0.0, 1.0 - days_old / DECAY_DAYS)
            score     = cosine * COSINE_WEIGHT + decay * DECAY_WEIGHT
            scores.append((score, cosine, c))

        scores.sort(key=lambda x: x[0], reverse=True)
        relevant = [(s, c) for s, cos, c in scores[:top_k] if cos >= SIMILARITY_FLOOR]
        if not relevant:
            return ""

        return "\n\n".join(
            f"[长期记忆 {i + 1}]\n{c['text']}"
            for i, (_, c) in enumerate(relevant)
        )
