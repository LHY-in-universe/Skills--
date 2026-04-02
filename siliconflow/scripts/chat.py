import os
import sys
import json
import requests
import subprocess
from pathlib import Path

"""
SiliconFlow Python Robust Orchestrator
支持: 自动回退 (Auto-fallback)、多模型切换、多模态输入、本地工具调用、技能自动注册
"""

# 1. 基础配置
def load_env():
    env_path = Path(__file__).parent.parent / ".env"
    env = {}
    if env_path.exists():
        with open(env_path, "r", encoding="utf-8") as f:
            for line in f:
                if "=" in line:
                    key, value = line.strip().split("=", 1)
                    env[key] = value
    return env

env = load_env()
API_KEY = env.get("SILICONFLOW_API_KEY")
API_URL = env.get("SILICONFLOW_API_URL", "https://api.siliconflow.cn/v1/chat/completions")

# 路径常量（模块级，供各函数共用）
PROJECT_ROOT  = Path(__file__).parent.parent.parent
SKILLS_ROOT   = PROJECT_ROOT / "skills"
REGISTRY_PATH = PROJECT_ROOT / "siliconflow" / "skill_registry.json"
MEMORY_PATH   = PROJECT_ROOT / "siliconflow" / "memory.json"

BASE_SYSTEM_PROMPT = "你是一个基于 Python 的全能助手。支持图片分析和本地工具调用。"

def load_memory() -> dict:
    """从磁盘读取长期记忆，返回 key-value 字典"""
    if not MEMORY_PATH.exists():
        return {}
    try:
        return json.loads(MEMORY_PATH.read_text(encoding="utf-8"))
    except Exception:
        return {}

def _build_system_prompt(mem: dict) -> str:
    """根据记忆内容构建系统提示"""
    if not mem:
        return BASE_SYSTEM_PROMPT
    lines = "\n".join(f"- {k}: {v}" for k, v in mem.items())
    return f"{BASE_SYSTEM_PROMPT}\n\n[长期记忆]\n{lines}"

def _refresh_system_prompt(mem: dict):
    """记忆变更后同步更新 messages[0] 的系统提示"""
    messages[0]["content"] = _build_system_prompt(mem)

# 预设模型
MODELS = {
    "1": "Qwen/Qwen3.5-27B",
    "2": "deepseek-ai/DeepSeek-V3.2",
    "3": "Pro/zai-org/GLM-4.7"
}

current_model = MODELS["1"]

# 2. 定义本地工具 (OpenAI 格式) — 手动注册的核心技能
TOOLS = [
    {
        "type": "function",
        "function": {
            "name": "get_weather",
            "description": "获取实时天气信息（温度、状态等）",
            "parameters": {
                "type": "object",
                "properties": {"city": {"type": "string"}},
                "required": ["city"]
            }
        }
    },
    {
        "type": "function",
        "function": {
            "name": "get_system_info",
            "description": "获取系统运行指标（CPU、内存占用、已运行时间 uptime 等）",
            "parameters": {"type": "object", "properties": {}}
        }
    },
    {
        "type": "function",
        "function": {
            "name": "get_current_time",
            "description": "获取当前的精确日期、时刻（北京时间）和星期",
            "parameters": {"type": "object", "properties": {}}
        }
    },
    {
        "type": "function",
        "function": {
            "name": "file_editor",
            "description": "在 /Users/lhy/Desktop/Skills探索 目录内操作文件：列出文件(list)、读取(read)、写入/新建(write)、追加(append)、文本替换(replace)",
            "parameters": {
                "type": "object",
                "properties": {
                    "op":      {"type": "string", "enum": ["list", "read", "write", "append", "replace"], "description": "操作类型"},
                    "folder":  {"type": "string", "description": "目标文件夹路径（必须在 /Users/lhy/Desktop/Skills探索 内）"},
                    "file":    {"type": "string", "description": "文件名（list 操作可省略）"},
                    "content": {"type": "string", "description": "写入或追加的内容（write/append 操作）"},
                    "old":     {"type": "string", "description": "要被替换的原始文本（replace 操作）"},
                    "new":     {"type": "string", "description": "替换后的新文本（replace 操作）"}
                },
                "required": ["op", "folder"]
            }
        }
    },
    {
        "type": "function",
        "function": {
            "name": "run_terminal",
            "description": "在 /Users/lhy/Desktop/Skills探索/test 沙箱目录内执行终端命令（ls/cat/mkdir/touch/cp/mv/rm/grep/find/python3 等白名单命令）",
            "parameters": {
                "type": "object",
                "properties": {
                    "command": {"type": "string", "description": "要执行的终端命令，如 'ls -la' 或 'mkdir output'"}
                },
                "required": ["command"]
            }
        }
    },
    {
        "type": "function",
        "function": {
            "name": "open_terminal",
            "description": "打开 macOS Terminal.app 并在可见窗口中执行命令，用户可实时看到终端输出，适合交互式或长时间运行的任务",
            "parameters": {
                "type": "object",
                "properties": {
                    "command":   {"type": "string", "description": "要在终端中执行的命令"},
                    "directory": {"type": "string", "description": "工作目录（可选），执行前先 cd 到该路径"}
                },
                "required": ["command"]
            }
        }
    },
    {
        "type": "function",
        "function": {
            "name": "write_python",
            "description": "安全写入 Python 文件，写入前强制校验：路径范围（限 Skills探索/）、Python 语法、禁止危险调用（eval/exec/os.system等）、结构规范（docstring/main入口/命名规范）",
            "parameters": {
                "type": "object",
                "properties": {
                    "folder":  {"type": "string", "description": "目标文件夹路径"},
                    "file":    {"type": "string", "description": "文件名，必须以 .py 结尾"},
                    "content": {"type": "string", "description": "Python 代码内容，换行用 \\n，制表符用 \\t"}
                },
                "required": ["folder", "file", "content"]
            }
        }
    },
    {
        "type": "function",
        "function": {
            "name": "memory_save",
            "description": "保存一条长期记忆（持久化到磁盘，重启后仍然有效）。用于记住用户的名字、偏好、背景信息等。不要用于记录权限授予，权限由系统自动管理。",
            "parameters": {
                "type": "object",
                "properties": {
                    "key":   {"type": "string", "description": "记忆的键名，如 user_name、preferred_language"},
                    "value": {"type": "string", "description": "要记住的内容"}
                },
                "required": ["key", "value"]
            }
        }
    },
    {
        "type": "function",
        "function": {
            "name": "memory_forget",
            "description": "删除一条长期记忆",
            "parameters": {
                "type": "object",
                "properties": {
                    "key": {"type": "string", "description": "要删除的记忆键名"}
                },
                "required": ["key"]
            }
        }
    }
]

# 3. 注册表工具加载
def _load_registry() -> list:
    """读取 skill_registry.json"""
    if not REGISTRY_PATH.exists():
        return []
    try:
        with open(REGISTRY_PATH, encoding="utf-8") as f:
            return json.load(f)
    except Exception:
        return []

def _extend_tools_from_registry():
    """将注册表中的新技能追加到 TOOLS（去重）"""
    existing = {t["function"]["name"] for t in TOOLS}
    for skill in _load_registry():
        if skill["tool_name"] not in existing:
            TOOLS.append({
                "type": "function",
                "function": {
                    "name":        skill["tool_name"],
                    "description": skill["description"],
                    "parameters":  skill["parameters"]
                }
            })
            existing.add(skill["tool_name"])

# 模块加载时先读一次注册表
_extend_tools_from_registry()

# 4. 权限拦截器
# session 级授权记忆：key 已在集合中则自动放行，重启后清空
GRANTED_PERMISSIONS: set = set()

def _permission_key(name: str, args: dict) -> str:
    """生成权限记忆的 key：file_editor 细分到操作类型，其余用工具名"""
    if name == "file_editor":
        return f"file_editor:{args.get('op', '')}"
    return name

def _needs_permission(name: str, args: dict) -> bool:
    """判断此次工具调用是否需要用户授权"""
    if name in {"write_python", "run_terminal", "open_terminal"}:
        return True
    # file_editor 仅写操作需要授权，list/read 不需要
    if name == "file_editor" and args.get("op") in {"write", "append", "replace"}:
        return True
    return False

def _format_permission_detail(name: str, args: dict) -> str:
    """生成权限申请的详细说明文本"""
    if name == "run_terminal":
        return (
            f"  操作类型 : 执行终端命令\n"
            f"  命令内容 : {args.get('command', '')}"
        )
    if name == "open_terminal":
        return (
            f"  操作类型 : 在 Terminal.app 中执行命令\n"
            f"  命令内容 : {args.get('command', '')}\n"
            f"  工作目录 : {args.get('directory', '默认沙箱目录')}"
        )
    if name == "write_python":
        preview = args.get("content", "")
        lines   = preview.splitlines()
        summary = "\n    ".join(lines[:15])
        suffix  = f"\n    …（共 {len(lines)} 行）" if len(lines) > 15 else ""
        return (
            f"  操作类型 : 创建 Python 文件\n"
            f"  目标路径 : {args.get('folder', '')}/{args.get('file', '')}\n"
            f"  代码预览 :\n    {summary}{suffix}"
        )
    if name == "file_editor":
        op = args.get("op", "")
        content_or_new = args.get("content") or args.get("new", "")
        preview = content_or_new[:300] + ("…" if len(content_or_new) > 300 else "")
        return (
            f"  操作类型 : 文件 {op}\n"
            f"  目标文件 : {args.get('folder', '')}/{args.get('file', '')}\n"
            f"  写入内容 : {preview}"
        )
    return f"  参数: {args}"

def request_permission(name: str, args: dict) -> bool:
    """检查授权记忆，已授权则自动放行；否则弹出确认，同意后记住本次会话"""
    key = _permission_key(name, args)
    sep = "═" * 52

    # 已在记忆中，自动放行
    if key in GRANTED_PERMISSIONS:
        print(f"\n  ✅ [已记住授权，自动放行] {key}")
        return True

    # 首次询问
    print(f"\n{sep}")
    print(f"  ⚠️  [权限申请] 模型请求执行敏感操作")
    print(f"  工具名称 : {name}")
    print(_format_permission_detail(name, args))
    print(sep)
    answer = input("  请问您是否同意执行？(yes / no) > ").strip().lower()
    allowed = answer in {"yes", "y", "是", "同意", "可以", "继续", "ok", "好", "行"}
    if allowed:
        GRANTED_PERMISSIONS.add(key)
        print(f"  ✅ 已授权，本次会话内 [{key}] 不再询问。")
    else:
        print("  ❌ 已拒绝，操作已取消。")
    print(sep)
    return allowed

# 5. 本地工具执行器
def handle_tool_call(tool_call):
    name = tool_call["function"]["name"]
    try:
        args = json.loads(tool_call["function"]["arguments"])
    except json.JSONDecodeError:
        args = {}

    # ── 权限拦截：敏感操作须经用户确认（已记忆则自动放行）────────
    if _needs_permission(name, args):
        if not request_permission(name, args):
            return json.dumps(
                {"error": "用户拒绝了此操作，已取消执行。", "denied": True},
                ensure_ascii=False
            )

    try:
        # ── 手动注册技能 ──────────────────────────────────────────
        if name == "get_weather":
            city = args.get("city", "上海")
            print(f"\n[本地技能] 正在查询天气: {city}...")
            result = subprocess.check_output(
                ["python3", str(SKILLS_ROOT / "weather/scripts/get_weather.py"), city],
                text=True, stderr=subprocess.PIPE)
            return result.strip()

        if name == "get_system_info":
            print(f"\n[本地技能] 正在获取系统信息...")
            result = subprocess.check_output(
                ["python3", str(SKILLS_ROOT / "system_monitor/scripts/get_sys_info.py")],
                text=True, stderr=subprocess.PIPE)
            return result.strip()

        if name == "get_current_time":
            print(f"\n[本地技能] 正在查询当前时间...")
            result = subprocess.check_output(
                ["python3", str(SKILLS_ROOT / "clock/scripts/get_time.py")],
                text=True, stderr=subprocess.PIPE)
            return result.strip()

        if name == "file_editor":
            op     = args.get("op", "list")
            folder = args.get("folder", "")
            cmd = ["python3", str(SKILLS_ROOT / "file_editor/scripts/edit_file.py"),
                   "--op", op, "--folder", folder]
            if args.get("file"):    cmd += ["--file",    args["file"]]
            if args.get("content"): cmd += ["--content", args["content"]]
            if args.get("old"):     cmd += ["--old",     args["old"]]
            if args.get("new"):     cmd += ["--new",     args["new"]]
            print(f"\n[本地技能] 文件操作: {op} @ {folder}")
            proc = subprocess.run(cmd, text=True, capture_output=True)
            return proc.stdout.strip() or proc.stderr.strip()

        if name == "run_terminal":
            command = args.get("command", "")
            print(f"\n[本地技能] 执行终端命令: {command}")
            result = subprocess.check_output(
                ["python3", str(SKILLS_ROOT / "terminal/scripts/run_command.py"), command],
                text=True, stderr=subprocess.PIPE)
            try:
                data = json.loads(result)
                if data.get("stdout"):
                    print(f"\n[输出]\n{data['stdout'].rstrip()}")
                if data.get("stderr"):
                    print(f"\n[错误]\n{data['stderr'].rstrip()}")
            except Exception:
                print(f"\n[输出]\n{result.strip()}")
            return result.strip()

        if name == "write_python":
            folder  = args.get("folder", "")
            file    = args.get("file", "")
            content = args.get("content", "")
            print(f"\n[本地技能] 写入 Python 文件: {file}")
            proc = subprocess.run([
                "python3", str(SKILLS_ROOT / "python_writer/scripts/write_python.py"),
                f"--folder={folder}", f"--file={file}", f"--content={content}"
            ], text=True, capture_output=True)
            return proc.stdout.strip() or proc.stderr.strip()

        if name == "open_terminal":
            command   = args.get("command", "")
            directory = args.get("directory", "")
            print(f"\n[本地技能] 打开终端执行: {command}")
            cmd_args = ["python3", str(SKILLS_ROOT / "macos_terminal/scripts/open_terminal.py"), command]
            if directory:
                cmd_args.append(directory)
            result = subprocess.check_output(cmd_args, text=True, stderr=subprocess.PIPE)
            return result.strip()

        if name == "memory_save":
            key   = args.get("key", "")
            value = args.get("value", "")
            mem = load_memory()
            mem[key] = value
            MEMORY_PATH.write_text(json.dumps(mem, ensure_ascii=False, indent=2), encoding="utf-8")
            _refresh_system_prompt(mem)
            print(f"\n[记忆] 已保存: {key} = {value}")
            return json.dumps({"ok": True, "key": key, "value": value}, ensure_ascii=False)

        if name == "memory_forget":
            key = args.get("key", "")
            mem = load_memory()
            if key in mem:
                del mem[key]
                MEMORY_PATH.write_text(json.dumps(mem, ensure_ascii=False, indent=2), encoding="utf-8")
                _refresh_system_prompt(mem)
                print(f"\n[记忆] 已删除: {key}")
                return json.dumps({"ok": True, "deleted": key}, ensure_ascii=False)
            return json.dumps({"ok": False, "error": f"记忆中没有键 '{key}'"}, ensure_ascii=False)

        # ── 注册表动态技能通用 dispatch ───────────────────────────
        for skill in _load_registry():
            if name == skill["tool_name"]:
                script_path = SKILLS_ROOT / skill["skill_dir"] / skill["script"]
                print(f"\n[注册技能] 调用: {skill['tool_name']}")
                result = subprocess.check_output(
                    ["python3", str(script_path), f"--args={json.dumps(args, ensure_ascii=False)}"],
                    text=True, stderr=subprocess.PIPE)
                return result.strip()

    except subprocess.CalledProcessError as e:
        return f"执行本地技能失败: {e.stderr or str(e)}"
    except Exception as e:
        return f"执行本地技能失败: {str(e)}"
    return "未知工具"

# 6. 内容构建器 (支持图片 URL)
def build_content(user_input):
    import re
    img_regex = r"(https?://.*\.(?:png|jpg|jpeg|gif|webp))"
    match = re.search(img_regex, user_input, re.IGNORECASE)

    if match:
        image_url = match.group(1)
        text = user_input.replace(image_url, "").strip()
        print(f"\n[系统] 检测到图片输入: {image_url}")
        return [
            {"type": "text", "text": text or "请描述这张图片"},
            {"type": "image_url", "image_url": {"url": image_url}}
        ]
    return user_input

# 7. 核心对话引擎
messages = [
    {"role": "system", "content": BASE_SYSTEM_PROMPT}
]

def process_conversation(use_tools=True):
    global messages

    try:
        payload = {
            "model": current_model,
            "messages": messages
        }
        if use_tools:
            payload["tools"] = TOOLS
            payload["tool_choice"] = "auto"

        response = requests.post(
            API_URL,
            headers={"Authorization": f"Bearer {API_KEY}", "Content-Type": "application/json"},
            json=payload,
            timeout=60
        )

        # 智能回退: 如果模型不支持 tools，降级到纯文本
        if response.status_code == 400:
            error_data = response.json()
            if "tools" in str(error_data) or "choice" in str(error_data):
                print(f"\n[注意] 当前模型 {current_model} 不支持工具调用，将自动降级为纯文本模式...")
                return process_conversation(use_tools=False)

        response.raise_for_status()
        data = response.json()

        assistant_msg = data["choices"][0]["message"]
        messages.append(assistant_msg)

        # 处理工具调用
        if assistant_msg.get("tool_calls"):
            for tool_call in assistant_msg["tool_calls"]:
                func_name = tool_call["function"]["name"]
                func_args = tool_call["function"].get("arguments", "{}")

                # ── 调用前：打印技能名与参数 ──
                print(f"\n{'─'*50}")
                print(f"▶ [技能调用] {func_name}")
                try:
                    args_dict = json.loads(func_args)
                    for k, v in args_dict.items():
                        v_str = str(v)
                        print(f"  {k}: {v_str if len(v_str) <= 120 else v_str[:120] + '...'}")
                except Exception:
                    print(f"  {func_args}")

                result = handle_tool_call(tool_call)

                # ── 调用后：打印返回结果 ──
                print(f"◀ [返回结果]")
                try:
                    result_obj = json.loads(result)
                    print(json.dumps(result_obj, ensure_ascii=False, indent=2)
                          if len(result) <= 800 else
                          json.dumps(result_obj, ensure_ascii=False, indent=2)[:800] + "\n  ...(已截断)")
                except Exception:
                    print(result if len(result) <= 800 else result[:800] + "\n  ...(已截断)")
                print(f"{'─'*50}")

                messages.append({
                    "role": "tool",
                    "tool_call_id": tool_call["id"],
                    "content": result
                })
            return process_conversation(use_tools=use_tools)

        if assistant_msg.get("content"):
            print(f"\nSiliconFlow [Python 版] > {assistant_msg['content']}")

    except Exception as e:
        print(f"\n[Error] 对话失败: {str(e)}")

# 8. 交互入口
def start():
    global current_model

    # 启动时自动扫描并注册新技能
    scan_script = SKILLS_ROOT / "skill_manager" / "scripts" / "scan_skills.py"
    if scan_script.exists():
        print("\n[系统] 正在扫描新技能...")
        try:
            result = subprocess.run(
                ["python3", str(scan_script)],
                capture_output=True, text=True, timeout=30
            )
            summary = json.loads(result.stdout)
            if summary.get("new_skills"):
                print(f"[系统] ✅ 新注册技能: {', '.join(summary['new_skills'])}")
                _extend_tools_from_registry()   # 本次会话立即生效
            if summary.get("rejected"):
                for r in summary["rejected"]:
                    print(f"[系统] ⚠️  技能 {r['skill']} 校验未通过: {'; '.join(r['errors'])}")
            if not summary.get("new_skills") and not summary.get("rejected"):
                print(f"[系统] 技能扫描完成（共 {summary.get('total_registered', 0)} 个已注册技能）")
        except Exception as e:
            print(f"[系统] 技能扫描失败: {e}")

    # 启动时加载长期记忆并注入系统提示
    mem = load_memory()
    _refresh_system_prompt(mem)
    if mem:
        print(f"[系统] 已加载 {len(mem)} 条长期记忆: {', '.join(mem.keys())}")

    print("\n🐍 [Python 版] SiliconFlow 编排器已启动！")
    print(f"当前模型: {current_model}")
    print("输入 '/model' 切换模型，输入 'exit' 退出。")

    while True:
        try:
            user_input = input(f"\nUser [{current_model}] > ").strip()
            if not user_input: continue
            if user_input.lower() in ["exit", "quit"]: break

            if user_input == "/model":
                print("\n可用模型列表:")
                for k, v in MODELS.items(): print(f"{k}. {v}")
                choice = input("\n请选择序号或输入全名: ").strip()
                if MODELS.get(choice): current_model = MODELS[choice]
                elif choice: current_model = choice
                print(f"\n[系统] 已切换到: {current_model}")
                continue

            messages.append({"role": "user", "content": build_content(user_input)})
            process_conversation()

        except KeyboardInterrupt:
            print("\n已退出。")
            break

if __name__ == "__main__":
    if not API_KEY:
        print("Error: 未找到 SILICONFLOW_API_KEY，请检查 .env 文件。")
        sys.exit(1)
    start()
