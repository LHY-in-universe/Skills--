import sys
import os
import json
import argparse
from pathlib import Path

"""
文件编辑器技能脚本
支持操作: list / read / write / append / replace
所有操作强制限定在 ALLOWED_ROOT 内
"""

ALLOWED_ROOT = (Path(__file__).resolve().parent.parent.parent.parent / "test").resolve()

def check_within_allowed(path: Path):
    """确保路径在 ALLOWED_ROOT 内，否则退出"""
    try:
        path.relative_to(ALLOWED_ROOT)
    except ValueError:
        print(json.dumps({"error": f"权限拒绝：操作必须限定在 {ALLOWED_ROOT} 内"}, ensure_ascii=False))
        sys.exit(1)

def safe_path(folder: str, filename: str) -> Path:
    """解析并校验目标文件路径"""
    folder_path = Path(folder).expanduser().resolve()
    check_within_allowed(folder_path)
    target = (folder_path / filename).resolve()
    check_within_allowed(target)
    return target

def main():
    parser = argparse.ArgumentParser(description="文件编辑器技能")
    parser.add_argument("--folder", required=True, help="目标文件夹路径")
    parser.add_argument("--op", required=True,
                        choices=["list", "read", "write", "append", "replace"],
                        help="操作类型: list/read/write/append/replace")
    parser.add_argument("--file", help="文件名（list 操作时可省略）")
    parser.add_argument("--content", help="写入或追加的内容（write/append 操作）")
    parser.add_argument("--old", help="要被替换的原始文本（replace 操作）")
    parser.add_argument("--new", help="替换后的新文本（replace 操作）")
    args = parser.parse_args()

    folder = Path(args.folder).expanduser().resolve()
    check_within_allowed(folder)
    if not folder.exists():
        print(json.dumps({"error": f"文件夹不存在: {folder}"}, ensure_ascii=False))
        sys.exit(1)

    # --- list ---
    if args.op == "list":
        files = [f.name for f in sorted(folder.iterdir()) if f.is_file()]
        dirs  = [d.name + "/" for d in sorted(folder.iterdir()) if d.is_dir()]
        print(json.dumps({"folder": str(folder), "dirs": dirs, "files": files}, ensure_ascii=False, indent=2))
        return

    # 其余操作需要 --file
    if not args.file:
        print(json.dumps({"error": "--file 参数不能为空"}, ensure_ascii=False))
        sys.exit(1)

    target = safe_path(str(folder), args.file)

    # --- read ---
    if args.op == "read":
        if not target.exists():
            print(json.dumps({"error": f"文件不存在: {target.name}"}, ensure_ascii=False))
            sys.exit(1)
        content = target.read_text(encoding="utf-8")
        print(json.dumps({"file": target.name, "content": content}, ensure_ascii=False, indent=2))

    # --- write ---
    elif args.op == "write":
        if args.content is None:
            print(json.dumps({"error": "write 操作需要 --content 参数"}, ensure_ascii=False))
            sys.exit(1)
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(args.content, encoding="utf-8")
        print(json.dumps({"ok": True, "file": target.name, "action": "write",
                          "bytes": len(args.content.encode())}, ensure_ascii=False))

    # --- append ---
    elif args.op == "append":
        if args.content is None:
            print(json.dumps({"error": "append 操作需要 --content 参数"}, ensure_ascii=False))
            sys.exit(1)
        with open(target, "a", encoding="utf-8") as f:
            f.write(args.content)
        print(json.dumps({"ok": True, "file": target.name, "action": "append",
                          "bytes": len(args.content.encode())}, ensure_ascii=False))

    # --- replace ---
    elif args.op == "replace":
        if args.old is None or args.new is None:
            print(json.dumps({"error": "replace 操作需要 --old 和 --new 参数"}, ensure_ascii=False))
            sys.exit(1)
        if not target.exists():
            print(json.dumps({"error": f"文件不存在: {target.name}"}, ensure_ascii=False))
            sys.exit(1)
        original = target.read_text(encoding="utf-8")
        count = original.count(args.old)
        if count == 0:
            print(json.dumps({"ok": False, "file": target.name,
                              "message": "未找到匹配内容，文件未修改"}, ensure_ascii=False))
            return
        updated = original.replace(args.old, args.new)
        target.write_text(updated, encoding="utf-8")
        print(json.dumps({"ok": True, "file": target.name, "action": "replace",
                          "replacements": count}, ensure_ascii=False))

if __name__ == "__main__":
    main()
