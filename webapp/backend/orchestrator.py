import os
import json
import subprocess
import requests
import asyncio
from pathlib import Path
from typing import List, Dict, Any, Optional

class ChatOrchestrator:
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
        self.messages = [] # Will be initialized in clear_history
        self.tools = self._initialize_tools()
        self.active_skills = {t["function"]["name"]: True for t in self.tools}
        self.granted_permissions = set()
        self._pending_permission = None
        self._aborted = False
        self.clear_history()

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

    def _permission_key(self, name: str, args: dict) -> str:
        if name == "file_editor":
            return f"file_editor:{args.get('op', '')}"
        return name

    def _needs_permission(self, name: str, args: dict) -> bool:
        if name in {"write_python", "run_terminal", "open_terminal"}:
            return True
        if name == "file_editor" and args.get("op") in {"write", "append", "replace"}:
            return True
        return False

    def _format_permission_detail(self, name: str, args: dict) -> str:
        if name == "run_terminal":
            return f"执行终端命令: `{args.get('command', '')}`"
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
                    if "=" in line:
                        key, value = line.strip().split("=", 1)
                        env[key] = value
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
            {"type": "function", "function": {"name": "run_terminal", "description": "在沙箱执行命令", "parameters": {"type": "object", "properties": {"command": {"type": "string"}}, "required": ["command"]}}},
            {"type": "function", "function": {"name": "open_terminal", "description": "在真实终端执行命令", "parameters": {"type": "object", "properties": {"command": {"type": "string"}, "directory": {"type": "string"}}, "required": ["command"]}}},
            {"type": "function", "function": {"name": "write_python", "description": "安全写入 Python 文件", "parameters": {"type": "object", "properties": {"folder": {"type": "string"}, "file": {"type": "string"}, "content": {"type": "string"}}, "required": ["folder", "file", "content"]}}},
            {"type": "function", "function": {"name": "memory_save", "description": "保存长期记忆", "parameters": {"type": "object", "properties": {"key": {"type": "string"}, "value": {"type": "string"}}, "required": ["key", "value"]}}},
            {"type": "function", "function": {"name": "memory_forget", "description": "删除长期记忆", "parameters": {"type": "object", "properties": {"key": {"type": "string"}}, "required": ["key"]}}}
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
        if api_url: self.api_url = api_url
        if api_key: self.api_key = api_key
        if model: self.current_model = model

    def get_enabled_tools(self) -> List[Dict[str, Any]]:
        return [t for t in self.tools if self.active_skills.get(t["function"]["name"], False)]

    def toggle_skill(self, name: str, enabled: bool):
        self.active_skills[name] = enabled

    def clear_history(self):
        self.messages = [{"role": "system", "content": self._build_system_prompt(self.load_memory())}]
        self.granted_permissions = set()

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
                proc = subprocess.run(["python3", str(self.skills_root / "terminal/scripts/run_command.py"), command], text=True, capture_output=True, encoding="utf-8")
                return proc.stdout.strip() or proc.stderr.strip()

            if name == "write_python":
                folder, file, content = args.get("folder", ""), args.get("file", ""), args.get("content", "")
                proc = subprocess.run(["python3", str(self.skills_root / "python_writer/scripts/write_python.py"), f"--folder={folder}", f"--file={file}", f"--content={content}"], text=True, capture_output=True, encoding="utf-8")
                return proc.stdout.strip() or proc.stderr.strip()

            if name == "open_terminal":
                command, directory = args.get("command", ""), args.get("directory", "")
                cmd_args = ["python3", str(self.skills_root / "macos_terminal/scripts/open_terminal.py"), command]
                if directory: cmd_args.append(directory)
                proc = subprocess.run(cmd_args, text=True, capture_output=True, encoding="utf-8")
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

    async def chat(self, user_input: str) -> Dict[str, Any]:
        self._aborted = False
        self.messages.append({"role": "user", "content": user_input})
        return await self._run_chat_loop()

    async def resume_after_permission(self, granted: bool) -> Dict[str, Any]:
        """Resume conversation after user responds to a permission request."""
        if not self._pending_permission:
            return {"role": "assistant", "content": "没有待处理的权限请求。"}

        tc = self._pending_permission["tool_call"]
        name = self._pending_permission["name"]
        args = self._pending_permission["args"]

        if granted:
            self.granted_permissions.add(self._permission_key(name, args))
            result = await self.handle_tool_call(tc)
        else:
            result = "用户拒绝了此操作，已取消执行。"

        self.messages.append({
            "role": "tool",
            "tool_call_id": tc["id"],
            "content": result
        })
        self._pending_permission = None
        return await self._run_chat_loop()

    async def _run_chat_loop(self) -> Dict[str, Any]:
        while True:
            if self._aborted:
                return {"role": "assistant", "content": "⏹️ 已停止。"}

            enabled_tools = self.get_enabled_tools()
            payload = {
                "model": self.current_model,
                "messages": self.messages
            }
            if enabled_tools:
                payload["tools"] = enabled_tools
                payload["tool_choice"] = "auto"

            try:
                response = requests.post(
                    self.api_url,
                    headers={"Authorization": f"Bearer {self.api_key}", "Content-Type": "application/json"},
                    json=payload,
                    timeout=60
                )
                response.raise_for_status()
                data = response.json()

                assistant_msg = data["choices"][0]["message"]
                self.messages.append(assistant_msg)

                if assistant_msg.get("tool_calls"):
                    for tool_call in assistant_msg["tool_calls"]:
                        if self._aborted:
                            return {"role": "assistant", "content": "⏹️ 已停止。"}

                        name = tool_call["function"]["name"]
                        
                        # Verify the tool is actually enabled before processing
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

                        # Permission check — pause and ask the user via API
                        if self._needs_permission(name, args):
                            self._pending_permission = {
                                "tool_call": tool_call,
                                "name": name,
                                "args": args
                            }
                            return {
                                "type": "permission_required",
                                "tool_name": name,
                                "args": args,
                                "description": self._format_permission_detail(name, args)
                            }

                        result = await self.handle_tool_call(tool_call)
                        self.messages.append({
                            "role": "tool",
                            "tool_call_id": tool_call["id"],
                            "content": result
                        })
                    continue  # all tool calls done, loop again to get next LLM response

                return {"role": "assistant", "content": assistant_msg.get("content", "")}

            except Exception as e:
                return {"role": "assistant", "content": f"Error: {str(e)}"}
