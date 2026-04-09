import sys
import json
import argparse
from pathlib import Path

"""
summary_rules skill — 管理历史压缩摘要的注意事项规则

操作:
  list   — 列出所有规则
  add    — 追加一条规则（rule 参数必填）
  remove — 删除一条规则（rule 为规则文本 或 序号如 "1"）
  clear  — 清空所有规则
"""

# 规则文件位于项目根 siliconflow/summary_rules.json
# 脚本路径: {project_root}/skills/summary_rules/scripts/manage_rules.py
RULES_PATH = (Path(__file__).resolve().parent.parent.parent.parent
              / "siliconflow" / "summary_rules.json")


def load_rules() -> list:
    try:
        if RULES_PATH.exists():
            return json.loads(RULES_PATH.read_text(encoding="utf-8")).get("rules", [])
    except Exception:
        pass
    return []


def save_rules(rules: list) -> None:
    RULES_PATH.parent.mkdir(parents=True, exist_ok=True)
    RULES_PATH.write_text(
        json.dumps({"rules": rules}, ensure_ascii=False, indent=2),
        encoding="utf-8"
    )


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--args", default="{}")
    parsed = parser.parse_args()

    try:
        args = json.loads(parsed.args)
    except Exception:
        print(json.dumps({"ok": False, "error": "args 解析失败"}, ensure_ascii=False))
        sys.exit(1)

    op   = args.get("op", "list").strip().lower()
    rule = args.get("rule", "").strip()
    rules = load_rules()

    if op == "list":
        if not rules:
            print(json.dumps({"ok": True, "rules": [], "message": "暂无规则"}, ensure_ascii=False))
        else:
            numbered = [f"{i+1}. {r}" for i, r in enumerate(rules)]
            print(json.dumps({"ok": True, "rules": rules, "display": numbered}, ensure_ascii=False))

    elif op == "add":
        if not rule:
            print(json.dumps({"ok": False, "error": "add 操作需要提供 rule 参数"}, ensure_ascii=False))
            sys.exit(1)
        if rule in rules:
            print(json.dumps({"ok": False, "error": "规则已存在", "rules": rules}, ensure_ascii=False))
            sys.exit(1)
        rules.append(rule)
        save_rules(rules)
        print(json.dumps({"ok": True, "message": f"已添加规则（共 {len(rules)} 条）", "rules": rules}, ensure_ascii=False))

    elif op == "remove":
        if not rule:
            print(json.dumps({"ok": False, "error": "remove 操作需要提供 rule 参数（规则文本或序号）"}, ensure_ascii=False))
            sys.exit(1)
        # 支持序号删除（"1", "2", ...）
        if rule.isdigit():
            idx = int(rule) - 1
            if idx < 0 or idx >= len(rules):
                print(json.dumps({"ok": False, "error": f"序号 {rule} 超出范围（共 {len(rules)} 条）"}, ensure_ascii=False))
                sys.exit(1)
            removed = rules.pop(idx)
        else:
            if rule not in rules:
                print(json.dumps({"ok": False, "error": "未找到匹配的规则"}, ensure_ascii=False))
                sys.exit(1)
            rules.remove(rule)
            removed = rule
        save_rules(rules)
        print(json.dumps({"ok": True, "message": f"已删除：{removed}", "rules": rules}, ensure_ascii=False))

    elif op == "clear":
        save_rules([])
        print(json.dumps({"ok": True, "message": "已清空所有规则", "rules": []}, ensure_ascii=False))

    else:
        print(json.dumps({"ok": False, "error": f"未知操作: {op}，支持 list/add/remove/clear"}, ensure_ascii=False))
        sys.exit(1)


if __name__ == "__main__":
    main()
