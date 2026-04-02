import sys
import ast
import json
import re
from pathlib import Path

"""
Python 文件安全写入技能脚本
对写入过程施加四类限制：路径范围、语法格式、安全、结构规范
用法: python3 write_python.py --folder <目录> --file <文件名> --content <代码>
"""

# ── 1. 路径限制 ────────────────────────────────────────────────
ALLOWED_ROOT = Path("/Users/lhy/Desktop/Skills探索/test").resolve()

# ── 2. 安全限制：禁止使用的调用 ────────────────────────────────
DANGEROUS_CALLS = [
    "eval", "exec", "compile",
    "os.system", "os.popen", "os.execv", "os.execve",
    "subprocess.call", "subprocess.Popen", "subprocess.run",
    "__import__", "importlib.import_module",
    "open",       # 禁止直接文件 IO（应使用 file_editor 技能）
    "shutil.rmtree", "shutil.move",
]

# ── 3. 结构规范：必须包含的元素 ────────────────────────────────
REQUIRED_ENCODING  = r"#.*coding[:=]\s*(utf-8)"   # 文件头编码声明
REQUIRED_DOCSTRING = True                           # 模块级 docstring
REQUIRED_MAIN      = True                           # if __name__ == "__main__"


def abort(errors: list):
    print(json.dumps({"ok": False, "errors": errors}, ensure_ascii=False, indent=2))
    sys.exit(1)


def check_path(folder: str, filename: str) -> Path:
    """路径范围校验"""
    errors = []
    if not filename.endswith(".py"):
        errors.append(f"文件名必须以 .py 结尾，当前: {filename}")

    folder_path = Path(folder).expanduser().resolve()
    try:
        folder_path.relative_to(ALLOWED_ROOT)
    except ValueError:
        errors.append(f"目标文件夹超出允许范围（必须在 {ALLOWED_ROOT} 内）")

    target = (folder_path / filename).resolve()
    try:
        target.relative_to(ALLOWED_ROOT)
    except ValueError:
        errors.append("目标文件路径超出允许范围")

    if target.exists():
        errors.append(f"文件已存在: {filename}（python_writer 不允许覆盖已有文件）")

    return errors, target


def check_syntax(code: str) -> list:
    """语法校验"""
    errors = []
    try:
        ast.parse(code)
    except SyntaxError as e:
        errors.append(f"语法错误（第 {e.lineno} 行）: {e.msg}")
    return errors


def check_encoding(code: str) -> list:
    """必须有 UTF-8 编码声明（前两行内）"""
    errors = []
    first_two = "\n".join(code.splitlines()[:2])
    if not re.search(REQUIRED_ENCODING, first_two, re.IGNORECASE):
        errors.append("缺少 UTF-8 编码声明，请在文件前两行加入: # -*- coding: utf-8 -*-")
    return errors


def check_security(code: str) -> list:
    """安全限制：禁止危险调用"""
    errors = []
    try:
        tree = ast.parse(code)
    except SyntaxError:
        return errors  # 语法错误已在 check_syntax 中报告

    for node in ast.walk(tree):
        # 检查函数调用：eval(...)、exec(...) 等
        if isinstance(node, ast.Call):
            func = node.func
            # 直接调用: eval(...)
            if isinstance(func, ast.Name) and func.id in DANGEROUS_CALLS:
                errors.append(f"禁止使用危险调用: {func.id}()")
            # 属性调用: os.system(...)、subprocess.run(...) 等
            elif isinstance(func, ast.Attribute):
                dotted = f"{getattr(func.value, 'id', '')}.{func.attr}"
                if dotted in DANGEROUS_CALLS:
                    errors.append(f"禁止使用危险调用: {dotted}()")
        # 检查 __import__("xxx")
        if isinstance(node, ast.Expr) and isinstance(getattr(node, 'value', None), ast.Call):
            pass  # 已在 Call 分支处理
    return errors


def check_structure(code: str) -> list:
    """结构规范：docstring + if __name__ == '__main__'"""
    errors = []
    try:
        tree = ast.parse(code)
    except SyntaxError:
        return errors

    # 模块级 docstring
    if REQUIRED_DOCSTRING:
        if not (tree.body and isinstance(tree.body[0], ast.Expr)
                and isinstance(tree.body[0].value, ast.Constant)
                and isinstance(tree.body[0].value.value, str)):
            errors.append("缺少模块级 docstring（文件第一个语句应为三引号字符串）")

    # if __name__ == '__main__' 入口
    if REQUIRED_MAIN:
        has_main = False
        for node in ast.walk(tree):
            if isinstance(node, ast.If):
                test = node.test
                if (isinstance(test, ast.Compare)
                        and isinstance(test.left, ast.Name)
                        and test.left.id == "__name__"
                        and len(test.comparators) == 1
                        and isinstance(test.comparators[0], ast.Constant)
                        and test.comparators[0].value == "__main__"):
                    has_main = True
                    break
        if not has_main:
            errors.append("缺少入口守卫: if __name__ == '__main__':")

    # 函数/类命名规范（snake_case 函数名，PascalCase 类名）
    for node in ast.walk(tree):
        if isinstance(node, ast.FunctionDef):
            if not re.match(r'^[a-z_][a-z0-9_]*$', node.name) and not node.name.startswith('__'):
                errors.append(f"函数名不符合 snake_case 规范: {node.name}")
        if isinstance(node, ast.ClassDef):
            if not re.match(r'^[A-Z][a-zA-Z0-9]*$', node.name):
                errors.append(f"类名不符合 PascalCase 规范: {node.name}")

    return errors


def main():
    # 解析参数（简单 key=value 格式）
    args = {}
    for arg in sys.argv[1:]:
        if "=" in arg:
            k, v = arg.split("=", 1)
            args[k.lstrip("-")] = v

    folder  = args.get("folder", "")
    file    = args.get("file", "")
    content = args.get("content", "")

    if not folder or not file or not content:
        print(json.dumps({
            "ok": False,
            "errors": ["用法: python3 write_python.py --folder=<目录> --file=<文件名> --content=<代码>"]
        }, ensure_ascii=False, indent=2))
        sys.exit(1)

    # 反转义 \n → 真实换行（供 AI 调用时传入转义换行）
    content = content.replace("\\n", "\n").replace("\\t", "\t")

    all_errors = []

    # 路径校验
    path_errors, target = check_path(folder, file)
    all_errors.extend(path_errors)

    # 语法校验
    all_errors.extend(check_syntax(content))

    # 编码声明校验
    all_errors.extend(check_encoding(content))

    # 安全校验
    all_errors.extend(check_security(content))

    # 结构规范校验
    all_errors.extend(check_structure(content))

    if all_errors:
        abort(all_errors)

    # 全部通过，写入文件
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(content, encoding="utf-8")

    print(json.dumps({
        "ok": True,
        "file": str(target),
        "lines": len(content.splitlines()),
        "bytes": len(content.encode())
    }, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    main()
