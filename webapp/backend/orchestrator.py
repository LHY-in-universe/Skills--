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
        
        self.env = self._load_env()
        self.api_key = self.env.get("SILICONFLOW_API_KEY")
        self.api_url = self.env.get("SILICONFLOW_API_URL", "https://api.siliconflow.cn/v1/chat/completions")
        
        self.base_system_prompt = "你是一个基于 Python 的全能助手。支持图片分析和本地工具调用。"
        self.models = {
            "1": "Qwen/Qwen3.5-27B",
            "2": "deepseek-ai/DeepSeek-V3.2",
            "3": "Pro/zai-org/GLM-4.7"
        }
        
        self.current_model = self.models["1"]
        self.messages = [] # Will be initialized in clear_history
        self.tools = self._initialize_tools()
        self.active_skills = {t["function"]["name"]: True for t in self.tools}
        self.granted_permissions = set()
        self.clear_history()

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
                result = subprocess.check_output(["python3", str(self.skills_root / "weather/scripts/get_weather.py"), city], text=True)
                return result.strip()
            
            if name == "get_system_info":
                result = subprocess.check_output(["python3", str(self.skills_root / "system_monitor/scripts/get_sys_info.py")], text=True)
                return result.strip()
            
            if name == "get_current_time":
                result = subprocess.check_output(["python3", str(self.skills_root / "clock/scripts/get_time.py")], text=True)
                return result.strip()
                
            if name == "file_editor":
                op, folder = args.get("op", "list"), args.get("folder", "")
                cmd = ["python3", str(self.skills_root / "file_editor/scripts/edit_file.py"), "--op", op, "--folder", folder]
                if args.get("file"): cmd += ["--file", args["file"]]
                if args.get("content"): cmd += ["--content", args["content"]]
                if args.get("old"): cmd += ["--old", args["old"]]
                if args.get("new"): cmd += ["--new", args["new"]]
                proc = subprocess.run(cmd, text=True, capture_output=True)
                return proc.stdout.strip() or proc.stderr.strip()

            if name == "run_terminal":
                command = args.get("command", "")
                result = subprocess.check_output(["python3", str(self.skills_root / "terminal/scripts/run_command.py"), command], text=True)
                return result.strip()

            if name == "write_python":
                folder, file, content = args.get("folder", ""), args.get("file", ""), args.get("content", "")
                proc = subprocess.run(["python3", str(self.skills_root / "python_writer/scripts/write_python.py"), f"--folder={folder}", f"--file={file}", f"--content={content}"], text=True, capture_output=True)
                return proc.stdout.strip() or proc.stderr.strip()

            if name == "open_terminal":
                command, directory = args.get("command", ""), args.get("directory", "")
                cmd_args = ["python3", str(self.skills_root / "macos_terminal/scripts/open_terminal.py"), command]
                if directory: cmd_args.append(directory)
                result = subprocess.check_output(cmd_args, text=True)
                return result.strip()

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
                            result = subprocess.check_output(["python3", str(script_path), f"--args={json.dumps(args, ensure_ascii=False)}"], text=True)
                            return result.strip()
                except Exception: pass

        except Exception as e:
            return f"Error executing tool: {str(e)}"
        return "Unknown tool"

    async def chat(self, user_input: str) -> Dict[str, Any]:
        self.messages.append({"role": "user", "content": user_input})
        
        while True:
            payload = {
                "model": self.current_model,
                "messages": self.messages,
                "tools": self.get_enabled_tools(),
                "tool_choice": "auto"
            }
            
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
                        result = await self.handle_tool_call(tool_call)
                        self.messages.append({
                            "role": "tool",
                            "tool_call_id": tool_call["id"],
                            "content": result
                        })
                    continue # Continue the loop to get the next response from LLM
                
                return {"role": "assistant", "content": assistant_msg.get("content", "")}
                
            except Exception as e:
                return {"role": "assistant", "content": f"Error: {str(e)}"}
