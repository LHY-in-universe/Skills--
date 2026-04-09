import re
import json
import uuid
import time
import subprocess
import requests
import httpx
from pathlib import Path
from typing import List, Dict, Any, Optional, AsyncGenerator
from context_manager import ContextManager
from memory_manager import MemoryManager
from router import ModelRouter
from token_tracker import TokenTracker

class ChatOrchestrator:
    # Tools to always include regardless of query content
    _ALWAYS_TOOLS = {"get_current_time", "memory_save", "memory_forget", "write_memory"}

    # Keyword → tool name mapping for dynamic filtering (medium tier)
    _TOOL_KEYWORDS: Dict[str, List[str]] = {
        "get_weather":      ["天气", "weather", "温度", "下雨", "晴", "气温"],
        "get_system_info":  ["系统", "cpu", "内存", "memory", "磁盘", "进程", "system"],
        "run_terminal":     ["命令", "终端", "执行", "运行", "terminal", "bash", "shell",
                             "ls", "git", "pip", "brew", "install"],
        "write_python":     ["python", "代码", "脚本", "程序", "写一个", ".py", "函数",
                             "class", "import", "script"],
        "file_editor":      ["文件", "读取", "写入", "目录", "folder", "file", "路径",
                             "read", "write", "列出", "查看文件"],
        "pip_venv":         ["pip", "install", "安装", "包", "库", "依赖", "uninstall",
                             "卸载", "requirements", "venv", "虚拟环境"],
    }

    def __init__(self, project_root: Path):
        self.project_root = project_root
        self.skills_root = project_root / "skills"
        self.registry_path = project_root / "siliconflow" / "skill_registry.json"
        self.memory_path = project_root / "siliconflow" / "memory.json"
        self.env_path = project_root / "siliconflow" / ".env"
        self.models_path = project_root / "webapp" / "backend" / "models.json"
        
        self.env = self._load_env()
        self.api_key = self.env.get("SILICONFLOW_API_KEY")
        self.api_url = self.env.get("SILICONFLOW_API_URL", "https://api.siliconflow.cn/v1/chat/completions")
        
        self.base_system_prompt = "你是一个基于 Python 的全能助手。支持图片分析和本地工具调用。"
        self.models = self._load_models()
        self.current_model = list(self.models.values())[0] if self.models else "Qwen/Qwen3.5-27B"
        self.tools = self._initialize_tools()
        self.active_skills = {t["function"]["name"]: True for t in self.tools}
        self.granted_permissions = set()
        self._pending_permission = None
        self._aborted = False
        self.user_cwd: Optional[str] = None

        # Multi-conversation support
        self.conversations_path = project_root / "siliconflow" / "conversations.json"
        self._conversations: Dict[str, Any] = {}
        self.active_conversation_id: str = ""
        self._load_conversations()

        _base = self.api_url.rsplit("/chat/completions", 1)[0]
        self.memory_manager = MemoryManager(
            memory_dir=self.project_root / "siliconflow" / "memory",
            api_key=self.api_key,
            api_base_url=_base,
        )
        self.memory_manager.index_all()
        self.context_manager = ContextManager(
            api_key=self.api_key,
            api_base_url=_base,
            embeddings_path=self.project_root / "siliconflow" / "conversation_embeddings.json",
            chat_model=self.current_model,
            memory_manager=self.memory_manager,
        )
        self.router = ModelRouter(
            api_key=self.api_key,
            api_base_url=_base,
            config_path=self.project_root / "siliconflow" / "routing_config.json",
        )
        self.token_tracker = TokenTracker(project_root / "siliconflow" / "token_usage.json")
        self.router.token_tracker = self.token_tracker
        self.context_manager.token_tracker = self.token_tracker
        self.memory_manager.token_tracker = self.token_tracker

    def _load_models(self) -> Dict[str, str]:
        if not self.models_path.exists():
            return {
                "Qwen-27B": "Qwen/Qwen3.5-27B",
                "DeepSeek-V3": "deepseek-ai/DeepSeek-V3.2"
            }
        try:
            return json.loads(self.models_path.read_text(encoding="utf-8"))
        except Exception:
            return {}

    def _save_models(self):
        self.models_path.write_text(json.dumps(self.models, indent=4, ensure_ascii=False), encoding="utf-8")

    def add_model(self, name: str, model_id: str):
        self.models[name] = model_id
        self._save_models()

    def delete_model(self, name: str):
        if name in self.models:
            del self.models[name]
            self._save_models()

    # ── Conversation management ─────────────────────────────────

    @property
    def messages(self) -> List[Dict]:
        return self._conversations[self.active_conversation_id]["messages"]

    @messages.setter
    def messages(self, value: List[Dict]):
        self._conversations[self.active_conversation_id]["messages"] = value

    def _load_conversations(self):
        if self.conversations_path.exists():
            try:
                data = json.loads(self.conversations_path.read_text(encoding="utf-8"))
                self._conversations = data.get("conversations", {})
                self.active_conversation_id = data.get("active_id", "")
            except Exception:
                pass
        if not self._conversations or self.active_conversation_id not in self._conversations:
            self._new_conversation_internal()
        else:
            # Restore active conversation's preferred model
            conv_model = self._conversations[self.active_conversation_id].get("model", "")
            if conv_model:
                self.current_model = conv_model

    def _save_conversations(self):
        self.conversations_path.write_text(
            json.dumps({"active_id": self.active_conversation_id, "conversations": self._conversations},
                       ensure_ascii=False, indent=2),
            encoding="utf-8"
        )

    def _new_conversation_internal(self, name: str = "新对话") -> str:
        conv_id = str(uuid.uuid4())[:8]
        self._conversations[conv_id] = {
            "id": conv_id,
            "name": name,
            "created_at": time.time(),
            "model": self.current_model,
            "messages": [{"role": "system", "content": self._build_system_prompt(self.load_memory())}]
        }
        self.active_conversation_id = conv_id
        self._save_conversations()
        return conv_id

    def _conv_summary(self, conv_id: str) -> Dict:
        conv = self._conversations[conv_id]
        preview = next((m["content"][:50] for m in conv["messages"] if m["role"] == "user" and isinstance(m.get("content"), str)), "")
        return {
            "id": conv["id"],
            "name": conv["name"],
            "created_at": conv["created_at"],
            "preview": preview,
            "active": conv["id"] == self.active_conversation_id,
            "message_count": sum(1 for m in conv["messages"] if m["role"] in ("user", "assistant"))
        }

    def list_conversations(self) -> List[Dict]:
        result = [self._conv_summary(cid) for cid in self._conversations]
        result.sort(key=lambda x: x["created_at"], reverse=True)
        return result

    def create_conversation(self) -> Dict:
        conv_id = self._new_conversation_internal()
        return self._conv_summary(conv_id)

    def switch_conversation(self, conv_id: str):
        if conv_id not in self._conversations:
            raise ValueError(f"对话不存在: {conv_id}")
        self._aborted = True
        self._pending_permission = None
        self.active_conversation_id = conv_id
        # Restore this conversation's preferred model
        conv_model = self._conversations[conv_id].get("model", "")
        if conv_model:
            self.current_model = conv_model
            self.context_manager.chat_model = conv_model
        self._save_conversations()

    def delete_conversation(self, conv_id: str):
        if conv_id not in self._conversations:
            raise ValueError(f"对话不存在: {conv_id}")
        del self._conversations[conv_id]
        self.context_manager.drop_conversation(conv_id)
        if not self._conversations:
            self._new_conversation_internal()
        elif self.active_conversation_id == conv_id:
            self.active_conversation_id = next(iter(self._conversations))
        self._save_conversations()

    def rename_conversation(self, conv_id: str, name: str):
        if conv_id not in self._conversations:
            raise ValueError(f"对话不存在: {conv_id}")
        self._conversations[conv_id]["name"] = name.strip() or "新对话"
        self._save_conversations()

    def _permission_key(self, name: str, args: dict) -> str:
        if name == "file_editor":
            return f"file_editor:{args.get('op', '')}"
        return name

    def _needs_permission(self, name: str, args: dict) -> bool:
        perm_key = self._permission_key(name, args)
        
        # Check if this is a dangerous delete operation that MUST bypass "Always Allow" cache
        is_delete = False
        if name == "run_terminal":
            cmd = args.get("command", "")
            if re.search(r'\brm\b|\brmdir\b', cmd):
                is_delete = True
        
        # If not a delete operation, and user already selected "Always Allow", skip prompting
        if not is_delete and perm_key in self.granted_permissions:
            return False

        if name in {"write_python", "run_terminal"}:
            return True
        if name == "file_editor" and args.get("op") in {"write", "append", "replace"}:
            return True
        if name == "pip_venv" and args.get("action") in {"install", "uninstall"}:
            return True
        return False
    def _format_permission_detail(self, name: str, args: dict) -> str:
        if name == "run_terminal":
            cwd_info = self.user_cwd or "/Users/lhy/Desktop/Skills探索/test（默认沙箱）"
            return f"执行终端命令: `{args.get('command', '')}`\n工作目录: {cwd_info}"
        if name == "open_terminal":
            return f"在 Terminal.app 中执行: `{args.get('command', '')}`，工作目录: {args.get('directory', '默认沙箱')}"
        if name == "write_python":
            lines = args.get("content", "").splitlines()
            preview = "\n".join(lines[:10]) + (f"\n...（共 {len(lines)} 行）" if len(lines) > 10 else "")
            return f"写入 Python 文件: `{args.get('folder', '')}/{args.get('file', '')}`\n```\n{preview}\n```"
        if name == "file_editor":
            op = args.get("op", "")
            body = args.get("content") or args.get("new", "")
            preview = body[:200] + ("…" if len(body) > 200 else "")
            return f"文件 {op} 操作: `{args.get('folder', '')}/{args.get('file', '')}`，内容: {preview}"
        if name == "pip_venv":
            action = args.get("action", "install")
            package = args.get("package", "")
            venv = args.get("venv_path", "") or "webapp/backend/venv（默认）"
            return f"pip {action}: `{package}`\n虚拟环境: {venv}"
        return str(args)

    def abort(self):
        """Cancel the current in-progress generation."""
        self._aborted = True
        self._pending_permission = None
        # Remove the last assistant message if it is a bare tool_calls with no content
        if len(self.messages) > 1:
            last = self.messages[-1]
            if last.get("role") == "assistant" and last.get("tool_calls") and not last.get("content"):
                self.messages.pop()

    def _load_env(self) -> Dict[str, str]:
        env = {}
        if self.env_path.exists():
            with open(self.env_path, "r", encoding="utf-8") as f:
                for line in f:
                    line = line.strip()
                    if not line or line.startswith("#") or "=" not in line:
                        continue
                    key, value = line.split("=", 1)
                    env[key.strip()] = value.strip()
        return env

    def load_memory(self) -> Dict[str, Any]:
        if not self.memory_path.exists(): return {}
        try:
            return json.loads(self.memory_path.read_text(encoding="utf-8"))
        except Exception: return {}

    def _build_system_prompt(self, mem: Dict[str, Any]) -> str:
        if not mem: return self.base_system_prompt
        lines = "\n".join(f"- {k}: {v}" for k, v in mem.items())
        return f"{self.base_system_prompt}\n\n[长期记忆]\n{lines}"

    def _initialize_tools(self) -> List[Dict[str, Any]]:
        # Hardcoded core tools from chat.py
        tools = [
            {"type": "function", "function": {"name": "get_weather", "description": "获取实时天气信息", "parameters": {"type": "object", "properties": {"city": {"type": "string"}}, "required": ["city"]}}},
            {"type": "function", "function": {"name": "get_system_info", "description": "获取系统运行指标", "parameters": {"type": "object", "properties": {}}}},
            {"type": "function", "function": {"name": "get_current_time", "description": "获取当前北京时间", "parameters": {"type": "object", "properties": {}}}},
            {"type": "function", "function": {"name": "file_editor", "description": "文件编辑工具", "parameters": {"type": "object", "properties": {"op": {"type": "string", "enum": ["list", "read", "write", "append", "replace"]}, "folder": {"type": "string"}, "file": {"type": "string"}, "content": {"type": "string"}, "old": {"type": "string"}, "new": {"type": "string"}}, "required": ["op", "folder"]}}},
            {"type": "function", "function": {"name": "run_terminal", "description": "在当前工作目录执行终端命令", "parameters": {"type": "object", "properties": {"command": {"type": "string", "description": "要执行的命令"}}, "required": ["command"]}}},
            {"type": "function", "function": {"name": "write_python", "description": "安全写入 Python 文件", "parameters": {"type": "object", "properties": {"folder": {"type": "string"}, "file": {"type": "string"}, "content": {"type": "string"}}, "required": ["folder", "file", "content"]}}},
            {"type": "function", "function": {"name": "memory_save", "description": "保存结构化长期记忆（键值对，如用户名/偏好等）", "parameters": {"type": "object", "properties": {"key": {"type": "string"}, "value": {"type": "string"}}, "required": ["key", "value"]}}},
            {"type": "function", "function": {"name": "memory_forget", "description": "删除结构化长期记忆", "parameters": {"type": "object", "properties": {"key": {"type": "string"}}, "required": ["key"]}}},
            {"type": "function", "function": {"name": "write_memory", "description": "将重要信息永久写入长期记忆文件（Markdown），供未来对话语义检索。适合记录事件、决策、用户需求、项目背景等自由文本。", "parameters": {"type": "object", "properties": {"content": {"type": "string", "description": "要记录的内容，尽量完整具体"}, "tag": {"type": "string", "description": "标签，如「用户偏好」「项目信息」「重要决策」等（可选）"}}, "required": ["content"]}}}
        ]
        
        # Load registry
        if self.registry_path.exists():
            try:
                registry = json.loads(self.registry_path.read_text(encoding="utf-8"))
                for skill in registry:
                    if skill["tool_name"] not in {t["function"]["name"] for t in tools}:
                        tools.append({
                            "type": "function",
                            "function": {
                                "name": skill["tool_name"],
                                "description": skill["description"],
                                "parameters": skill["parameters"]
                            }
                        })
            except Exception: pass
        return tools

    def update_config(self, api_url: Optional[str] = None, api_key: Optional[str] = None, model: Optional[str] = None):
        if api_url:
            self.api_url = api_url
            self.context_manager.api_base_url = api_url
            self.router.api_base_url = api_url
            self.memory_manager.api_base_url = api_url
        if api_key:
            self.api_key = api_key
            self.context_manager.api_key = api_key
            self.router.api_key = api_key
            self.memory_manager.api_key = api_key
        if model:
            self.current_model = model
            self.context_manager.chat_model = model
            if self.active_conversation_id in self._conversations:
                self._conversations[self.active_conversation_id]["model"] = model
                self._save_conversations()

    def get_enabled_tools(self) -> List[Dict[str, Any]]:
        return [t for t in self.tools if self.active_skills.get(t["function"]["name"], False)]

    def _select_tools(self, query: str, tier: str) -> List[Dict[str, Any]]:
        """Return the tool subset to include in the API call.

        easy  → no tools (pure chat, no overhead)
        hard  → all enabled tools
        medium / unknown → keyword-filtered; falls back to all if nothing matches
        """
        all_enabled = self.get_enabled_tools()

        if tier == "easy":
            print(f"[Tools] easy tier — no tools sent", flush=True)
            return []

        if tier == "hard" or not tier:
            return all_enabled

        # medium: keyword filtering
        q = query.lower()
        selected: set = set(self._ALWAYS_TOOLS)
        for name, keywords in self._TOOL_KEYWORDS.items():
            if any(kw in q for kw in keywords):
                selected.add(name)

        filtered = [t for t in all_enabled if t["function"]["name"] in selected]

        # If only the always-include tools matched, fall back to all tools to be safe
        specific = [t for t in filtered if t["function"]["name"] not in self._ALWAYS_TOOLS]
        if not specific:
            return all_enabled

        print(f"[Tools] medium tier — sending {len(filtered)}/{len(all_enabled)} tools: "
              f"{[t['function']['name'] for t in filtered]}", flush=True)
        return filtered

    def toggle_skill(self, name: str, enabled: bool):
        self.active_skills[name] = enabled

    def clear_history(self):
        self.messages = [{"role": "system", "content": self._build_system_prompt(self.load_memory())}]
        self.granted_permissions = set()
        self._conversations[self.active_conversation_id]["name"] = "新对话"
        self._save_conversations()

    async def handle_tool_call(self, tool_call: Dict[str, Any]) -> str:
        name = tool_call["function"]["name"]
        try:
            args = json.loads(tool_call["function"]["arguments"])
        except Exception: args = {}

        try:
            if name == "get_weather":
                city = args.get("city", "上海")
                proc = subprocess.run(["python3", str(self.skills_root / "weather/scripts/get_weather.py"), city], text=True, capture_output=True, encoding="utf-8")
                return proc.stdout.strip() or proc.stderr.strip()
            
            if name == "get_system_info":
                proc = subprocess.run(["python3", str(self.skills_root / "system_monitor/scripts/get_sys_info.py")], text=True, capture_output=True, encoding="utf-8")
                return proc.stdout.strip() or proc.stderr.strip()
            
            if name == "get_current_time":
                proc = subprocess.run(["python3", str(self.skills_root / "clock/scripts/get_time.py")], text=True, capture_output=True, encoding="utf-8")
                return proc.stdout.strip() or proc.stderr.strip()
                
            if name == "file_editor":
                op, folder = args.get("op", "list"), args.get("folder", "")
                cmd = ["python3", str(self.skills_root / "file_editor/scripts/edit_file.py"), "--op", op, "--folder", folder]
                if args.get("file"): cmd += ["--file", args["file"]]
                if args.get("content"): cmd += ["--content", args["content"]]
                if args.get("old"): cmd += ["--old", args["old"]]
                if args.get("new"): cmd += ["--new", args["new"]]
                proc = subprocess.run(cmd, text=True, capture_output=True, encoding="utf-8")
                return proc.stdout.strip() or proc.stderr.strip()

            if name == "run_terminal":
                command = args.get("command", "")
                cmd = ["python3", str(self.skills_root / "terminal/scripts/run_command.py"), command]
                if self.user_cwd:
                    cmd += ["--cwd", self.user_cwd]
                proc = subprocess.run(cmd, text=True, capture_output=True, encoding="utf-8")
                return proc.stdout.strip() or proc.stderr.strip()

            if name == "write_python":
                folder, file, content = args.get("folder", ""), args.get("file", ""), args.get("content", "")
                proc = subprocess.run(["python3", str(self.skills_root / "python_writer/scripts/write_python.py"), f"--folder={folder}", f"--file={file}", f"--content={content}"], text=True, capture_output=True, encoding="utf-8")
                return proc.stdout.strip() or proc.stderr.strip()

            if name == "memory_save":
                key, value = args.get("key", ""), args.get("value", "")
                mem = self.load_memory()
                mem[key] = value
                self.memory_path.write_text(json.dumps(mem, ensure_ascii=False, indent=2), encoding="utf-8")
                self.messages[0]["content"] = self._build_system_prompt(mem)
                return json.dumps({"ok": True}, ensure_ascii=False)

            if name == "memory_forget":
                key = args.get("key", "")
                mem = self.load_memory()
                if key in mem:
                    del mem[key]
                    self.memory_path.write_text(json.dumps(mem, ensure_ascii=False, indent=2), encoding="utf-8")
                    self.messages[0]["content"] = self._build_system_prompt(mem)
                    return json.dumps({"ok": True}, ensure_ascii=False)
                return json.dumps({"error": "Key not found"}, ensure_ascii=False)

            if name == "write_memory":
                content = args.get("content", "").strip()
                tag = args.get("tag", "AI手动写入")
                if not content:
                    return json.dumps({"error": "content is empty"}, ensure_ascii=False)
                self.memory_manager.append(content, source=tag)
                return json.dumps({"ok": True, "message": f"已写入长期记忆（{tag}）"}, ensure_ascii=False)

            # Registry Skills
            if self.registry_path.exists():
                try:
                    registry = json.loads(self.registry_path.read_text(encoding="utf-8"))
                    for skill in registry:
                        if name == skill["tool_name"]:
                            script_path = self.skills_root / skill["skill_dir"] / skill["script"]
                            proc = subprocess.run(["python3", str(script_path), f"--args={json.dumps(args, ensure_ascii=False)}"], text=True, capture_output=True, encoding="utf-8")
                            return proc.stdout.strip() or proc.stderr.strip()
                except Exception: pass

        except Exception as e:
            return f"Error executing tool: {str(e)}"
        return "Unknown tool"

    async def stream_chat(self, user_input: str) -> AsyncGenerator[Dict[str, Any], None]:
        """Entry point for streaming chat. Yields SSE-style dicts."""
        self._aborted = False
        _original_model = self.current_model
        routed_model, tier = await self.router.route(user_input)
        if routed_model:
            self.current_model = routed_model

        conv = self._conversations[self.active_conversation_id]
        if not any(m["role"] == "user" for m in conv["messages"]):
            conv["name"] = user_input[:25] + ("…" if len(user_input) > 25 else "")
        # Remove stale user message left by a previously failed request
        if self.messages and self.messages[-1]["role"] == "user":
            self.messages.pop()
        self.messages.append({"role": "user", "content": user_input})

        # Tell the frontend which model will respond before any tokens arrive
        yield {
            "type": "start",
            "_model": routed_model if routed_model else _original_model,
            "_tier": tier or "",
        }

        last_type = None
        async for event in self._stream_chat_loop(tier=tier, query=user_input):
            last_type = event.get("type")
            yield event

        # Only restore if routing actually overrode the model and it wasn't
        # changed externally (e.g. user switched model mid-stream)
        if routed_model and self.current_model == routed_model:
            self.current_model = _original_model
        self._save_conversations()

        if last_type not in ("permission_required", "aborted", "error"):
            yield {
                "type": "done",
                "_model": routed_model if routed_model else _original_model,
                "_tier": tier or "",
            }

    async def stream_resume_after_permission(self, granted: bool, always_allow: bool = False) -> AsyncGenerator[Dict[str, Any], None]:
        """Resume after permission dialog. Yields SSE-style dicts."""
        if not self._pending_permission:
            yield {"type": "error", "content": "没有待处理的权限请求。"}
            return

        tc   = self._pending_permission["tool_call"]
        name = self._pending_permission["name"]
        args = self._pending_permission["args"]
        self._pending_permission = None

        if not granted:
            self.messages.append({
                "role": "tool",
                "tool_call_id": tc["id"],
                "content": "用户拒绝了此操作，已取消执行。"
            })
        else:
            if always_allow:
                self.granted_permissions.add(self._permission_key(name, args))
            yield {"type": "tool_start", "name": name}
            result = await self.handle_tool_call(tc)
            self.messages.append({
                "role": "tool",
                "tool_call_id": tc["id"],
                "content": result,
            })
            yield {"type": "tool_done", "name": name}

        last_type = None
        async for event in self._stream_chat_loop(tier="hard", query=""):
            last_type = event.get("type")
            yield event

        self._save_conversations()

        if last_type not in ("permission_required", "aborted", "error"):
            yield {"type": "done", "_model": self.current_model, "_tier": ""}

    async def _stream_chat_loop(self, tier: str = "", query: str = "") -> AsyncGenerator[Dict[str, Any], None]:
        """Async generator: streams LLM output as SSE-style dicts, handles tool calls."""
        _routing = self.router.get_config()
        _summary_model = _routing.get("summary_model", "").strip() or self.current_model

        _loop_count = 0
        _in_tool_loop = False  # True after first tool execution
        while _loop_count < 10:
            if self._aborted:
                yield {"type": "aborted"}
                return

            # Strip garbage assistant messages (< 5 chars, no tool_calls) left by interrupted streams
            self.messages = [
                m for m in self.messages
                if not (
                    m.get("role") == "assistant"
                    and not m.get("tool_calls")
                    and len((m.get("content") or "").strip()) < 5
                )
            ]

            enabled_tools = self._select_tools(query, tier)
            api_msgs, compressed_msgs = self.context_manager.prepare_messages(
                conv_id=self.active_conversation_id,
                messages=self.messages,
                user_query=next(
                    (m["content"] for m in reversed(self.messages)
                     if m["role"] == "user" and isinstance(m.get("content"), str)),
                    ""
                ),
                chat_model=_summary_model,
            )
            self.messages = compressed_msgs
            payload: Dict[str, Any] = {
                "model": self.current_model,
                "messages": api_msgs,
                "stream": True,
            }
            if enabled_tools:
                payload["tools"] = enabled_tools
                payload["tool_choice"] = "auto"

            full_content = ""
            tool_calls_acc: Dict[int, Dict] = {}
            _stream_usage: Dict = {}

            _api_retries = 2
            for _attempt in range(_api_retries + 1):
              try:
                _timeout = httpx.Timeout(connect=15.0, read=300.0, write=15.0, pool=15.0)
                async with httpx.AsyncClient(timeout=_timeout, trust_env=False) as client:
                    async with client.stream(
                        "POST",
                        self.api_url,
                        headers={
                            "Authorization": f"Bearer {self.api_key}",
                            "Content-Type": "application/json",
                        },
                        json=payload,
                    ) as response:
                        response.raise_for_status()
                        async for line in response.aiter_lines():
                            if self._aborted:
                                yield {"type": "aborted"}
                                return
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
                            choice = chunk["choices"][0]
                            delta = choice.get("delta", {})

                            text = delta.get("content") or ""
                            if text:
                                full_content += text
                                yield {"type": "text", "content": text}

                            for tc_d in delta.get("tool_calls", []):
                                idx = tc_d["index"]
                                if idx not in tool_calls_acc:
                                    tool_calls_acc[idx] = {
                                        "id": "", "type": "function",
                                        "function": {"name": "", "arguments": ""}
                                    }
                                tc = tool_calls_acc[idx]
                                if tc_d.get("id"):
                                    tc["id"] = tc_d["id"]
                                fn = tc_d.get("function", {})
                                tc["function"]["name"] += fn.get("name", "")
                                tc["function"]["arguments"] += fn.get("arguments", "")
                break  # success — exit retry loop

              except Exception as e:
                err_msg = str(e)
                is_interrupted = ("incomplete" in err_msg.lower() or "peer closed" in err_msg.lower() or "read timeout" in err_msg.lower() or "server disconnected" in err_msg.lower())
                # Retry if nothing was received yet and it's a connection-level failure
                if not full_content and not tool_calls_acc and is_interrupted and _attempt < _api_retries:
                    import asyncio
                    print(f"[Chat] connection dropped before response, retrying ({_attempt + 1}/{_api_retries})...", flush=True)
                    await asyncio.sleep(1)
                    continue
                # Partial meaningful content — save and fall through
                if full_content and len(full_content) >= 10 and is_interrupted:
                    print(f"[Chat] stream interrupted ({type(e).__name__}), saving partial content ({len(full_content)} chars)", flush=True)
                    break  # fall through to commit
                else:
                    yield {"type": "error", "content": f"Error: {err_msg}"}
                    return

            # Record and emit token usage if captured
            if _stream_usage:
                pt = _stream_usage.get("prompt_tokens", 0)
                ct = _stream_usage.get("completion_tokens", 0)
                _call_type = "skill" if _in_tool_loop else "chat"
                self.token_tracker.record(_call_type, self.current_model, pt, ct)
                yield {"type": "usage", "call_type": _call_type,
                       "prompt": pt, "completion": ct,
                       "total": _stream_usage.get("total_tokens", pt + ct),
                       "model": self.current_model}

            # Commit assistant message
            tool_calls = [tool_calls_acc[i] for i in sorted(tool_calls_acc)] if tool_calls_acc else None
            assistant_msg: Dict[str, Any] = {"role": "assistant", "content": full_content}
            if tool_calls:
                assistant_msg["tool_calls"] = tool_calls
            self.messages.append(assistant_msg)

            if not tool_calls:
                return  # natural completion

            # Execute tool calls
            for tool_call in tool_calls:
                if self._aborted:
                    yield {"type": "aborted"}
                    return

                name = tool_call["function"]["name"]

                if not self.active_skills.get(name, False):
                    self.messages.append({
                        "role": "tool",
                        "tool_call_id": tool_call["id"],
                        "content": f"Error: Tool '{name}' is explicitly disabled by the user and cannot be executed."
                    })
                    continue

                try:
                    args = json.loads(tool_call["function"]["arguments"])
                except Exception:
                    args = {}

                if self._needs_permission(name, args):
                    self._pending_permission = {
                        "tool_call": tool_call,
                        "name": name,
                        "args": args,
                    }
                    yield {
                        "type": "permission_required",
                        "tool_name": name,
                        "args": args,
                        "description": self._format_permission_detail(name, args),
                    }
                    return

                yield {"type": "tool_start", "name": name}
                result = await self.handle_tool_call(tool_call)
                self.messages.append({
                    "role": "tool",
                    "tool_call_id": tool_call["id"],
                    "content": result,
                })
                yield {"type": "tool_done", "name": name}
            # loop: fetch next LLM response after tool results
            _in_tool_loop = True
            _loop_count += 1
