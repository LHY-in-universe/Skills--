import ast
import json
import sys
from pathlib import Path

"""
Skill Manager — 安全扫描器
启动时自动运行，扫描 skills/ 下含 skill_manifest.json 的新技能，
对所有 .py 脚本做安全校验，通过后写入 skill_registry.json。
"""

PROJECT_ROOT  = Path(__file__).parent.parent.parent.parent
SKILLS_ROOT   = PROJECT_ROOT / "skills"
REGISTRY_PATH = PROJECT_ROOT / "siliconflow" / "skill_registry.json"

# 与 python_writer 保持一致的危险调用黑名单
DANGEROUS_CALLS = {
    "eval", "exec", "compile", "__import__",
    "os.system", "os.popen", "os.execv", "os.execve",
    "subprocess.call", "subprocess.Popen", "subprocess.run",
    "importlib.import_module",
    "shutil.rmtree", "shutil.move",
}

# skill_manifest.json 必须包含的字段
MANIFEST_REQUIRED = {"tool_name", "skill_dir", "script", "description", "parameters"}


def load_registry() -> list:
    if REGISTRY_PATH.exists():
        try:
            with open(REGISTRY_PATH, encoding="utf-8") as f:
                return json.load(f)
        except Exception:
            pass
    return []


def save_registry(data: list):
    with open(REGISTRY_PATH, "w", encoding="utf-8") as f:
        json.dump(data, f, ensure_ascii=False, indent=2)


def validate_python_security(py_path: Path) -> list[str]:
    """对单个 .py 文件做语法 + 安全校验，返回错误列表"""
    errors = []
    try:
        source = py_path.read_text(encoding="utf-8")
    except Exception as e:
        return [f"无法读取文件 {py_path.name}: {e}"]

    # 语法校验
    try:
        tree = ast.parse(source)
    except SyntaxError as e:
        return [f"{py_path.name} 语法错误（第 {e.lineno} 行）: {e.msg}"]

    # 危险调用检查
    for node in ast.walk(tree):
        if isinstance(node, ast.Call):
            func = node.func
            if isinstance(func, ast.Name) and func.id in DANGEROUS_CALLS:
                errors.append(f"{py_path.name}: 禁止使用 {func.id}()")
            elif isinstance(func, ast.Attribute):
                dotted = f"{getattr(func.value, 'id', '')}.{func.attr}"
                if dotted in DANGEROUS_CALLS:
                    errors.append(f"{py_path.name}: 禁止使用 {dotted}()")

    return errors


def validate_manifest(manifest: dict, skill_dir: Path) -> list[str]:
    """校验 manifest 字段完整性及引用脚本是否存在"""
    errors = []
    missing = MANIFEST_REQUIRED - set(manifest.keys())
    if missing:
        errors.append(f"skill_manifest.json 缺少字段: {', '.join(missing)}")
        return errors

    # skill_dir 字段应与实际目录名一致
    if manifest["skill_dir"] != skill_dir.name:
        errors.append(f"skill_dir 值 '{manifest['skill_dir']}' 与目录名 '{skill_dir.name}' 不符")

    # 主脚本必须存在
    script_path = skill_dir / manifest["script"]
    if not script_path.exists():
        errors.append(f"script 指定的文件不存在: {manifest['script']}")

    # parameters 必须是合法的 JSON Schema object
    params = manifest.get("parameters", {})
    if not isinstance(params, dict) or params.get("type") != "object":
        errors.append("parameters 必须是 JSON Schema object（含 'type': 'object'）")

    return errors


def scan():
    registry   = load_registry()
    registered = {s["tool_name"] for s in registry}

    new_skills = []
    rejected   = []

    for skill_dir in sorted(SKILLS_ROOT.iterdir()):
        if not skill_dir.is_dir():
            continue

        manifest_path = skill_dir / "skill_manifest.json"
        if not manifest_path.exists():
            continue  # 没有 manifest 的是旧式手动注册技能，跳过

        # 读取 manifest
        try:
            manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
        except Exception as e:
            rejected.append({"skill": skill_dir.name, "errors": [f"skill_manifest.json 解析失败: {e}"]})
            continue

        tool_name = manifest.get("tool_name", skill_dir.name)

        # 已注册则跳过
        if tool_name in registered:
            continue

        all_errors = []

        # 1. manifest 字段校验
        all_errors.extend(validate_manifest(manifest, skill_dir))

        # 2. 安全校验（trusted:true 的预装技能跳过）
        if not manifest.get("trusted", False):
            scripts_dir = skill_dir / "scripts"
            if scripts_dir.exists():
                for py_file in sorted(scripts_dir.glob("*.py")):
                    all_errors.extend(validate_python_security(py_file))

        if all_errors:
            rejected.append({"skill": tool_name, "errors": all_errors})
            continue

        # 全部通过 — 写入注册表
        registry.append({
            "tool_name":   manifest["tool_name"],
            "skill_dir":   manifest["skill_dir"],
            "script":      manifest["script"],
            "description": manifest["description"],
            "parameters":  manifest["parameters"]
        })
        registered.add(tool_name)
        new_skills.append(tool_name)

    save_registry(registry)

    print(json.dumps({
        "new_skills": new_skills,
        "rejected":   rejected,
        "total_registered": len(registry),
        "message": "扫描完成"
    }, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    scan()
