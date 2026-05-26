//! 技能视图 + 开关 + 配置 + 启动扫描。
//!
//! `skill_registry.json` 做运行时可执行技能列表，`skill_settings.json` 做可写覆盖。
//! 同时会扫描 `skills/` 下通过 `clawhub` 下载但尚未具备 `skill_manifest.json`
//! 的技能，并在 `/api/skills` 中显示为“已安装但不可执行”。

use crate::app::services::config_service::ConfigService;
use anyhow::anyhow;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

const HIGH_RISK_TOOL_NAMES: &[&str] = &[
    "run_terminal",
    "file_editor",
    "write_python",
    "pip_venv",
    "vision_analyze",
];

const RUST_BUILTIN_TOOL_NAMES: &[&str] = &["memory_save", "memory_forget"];

#[derive(Clone, Debug)]
struct InstalledSkillMeta {
    name: String,
    skill_dir: String,
    description: String,
    has_manifest: bool,
    has_skill_md: bool,
    executable: bool,
    managed_by: String,
    install_source: Option<String>,
    origin: Value,
    version_or_ref: Option<String>,
    risk_level: String,
    install_error: Option<String>,
}

#[derive(Clone, Debug)]
struct MarkdownArgSpec {
    name: String,
    ty: String,
    description: String,
}

#[derive(Clone, Debug)]
struct MarkdownToolSpec {
    name: String,
    args: Vec<MarkdownArgSpec>,
}

fn default_skill_config(enabled: bool) -> Value {
    serde_json::json!({
        "enabled": enabled,
        "api_key_ref": Value::Null,
        "env": {}
    })
}

fn nested_value<'a>(value: &'a Value, path: &[&str]) -> Option<&'a Value> {
    let mut current = value;
    for key in path {
        current = current.get(*key)?;
    }
    Some(current)
}

fn nested_str<'a>(value: &'a Value, path: &[&str]) -> Option<&'a str> {
    nested_value(value, path)?.as_str()
}

fn read_json_path(path: &Path) -> Option<Value> {
    let text = fs::read_to_string(path).ok()?;
    serde_json::from_str::<Value>(&text).ok()
}

fn sanitize_tool_name(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
        } else {
            out.push('_');
        }
    }
    while out.contains("__") {
        out = out.replace("__", "_");
    }
    out.trim_matches('_').to_string()
}

fn infer_skill_description_from_dir(skill_dir: &Path, fallback: &str) -> String {
    let skill_json = read_json_path(&skill_dir.join("skill.json"));
    skill_json
        .as_ref()
        .and_then(|v| v.get("description"))
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .or_else(|| {
            let skill_md = fs::read_to_string(skill_dir.join("SKILL.md")).ok()?;
            skill_md
                .lines()
                .find(|line| !line.trim().is_empty() && !line.trim_start().starts_with('#'))
                .map(|line| line.trim_matches('-').trim().to_string())
        })
        .unwrap_or_else(|| fallback.to_string())
}

fn infer_tool_name_from_dir(skill_dir: &Path, fallback: &str) -> String {
    let skill_json = read_json_path(&skill_dir.join("skill.json"));
    let raw = skill_json
        .as_ref()
        .and_then(|v| v.get("identifier"))
        .and_then(|v| v.as_str())
        .or_else(|| {
            skill_json
                .as_ref()
                .and_then(|v| v.get("name"))
                .and_then(|v| v.as_str())
        })
        .unwrap_or(fallback);
    sanitize_tool_name(raw)
}

fn collect_skill_scripts(skill_dir: &Path) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let candidates = [
        skill_dir.to_path_buf(),
        skill_dir.join("scripts"),
        skill_dir.join("bin"),
    ];
    for base in candidates {
        if !base.exists() || !base.is_dir() {
            continue;
        }
        let entries = match fs::read_dir(&base) {
            Ok(entries) => entries,
            Err(_) => continue,
        };
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let Some(ext) = path.extension().and_then(|v| v.to_str()) else {
                continue;
            };
            if !matches!(ext, "py" | "sh" | "bash" | "zsh" | "ts" | "js" | "mjs" | "cjs") {
                continue;
            }
            let file_name = path.file_name().and_then(|v| v.to_str()).unwrap_or_default();
            if file_name.starts_with("__clawhub_") {
                continue;
            }
            let relative = path
                .strip_prefix(skill_dir)
                .ok()
                .and_then(|p| p.to_str())
                .unwrap_or_default()
                .to_string();
            let stem = path
                .file_stem()
                .and_then(|v| v.to_str())
                .map(sanitize_tool_name)
                .unwrap_or_else(|| "run".to_string());
            out.push((relative, stem));
        }
    }
    out.sort();
    out
}

fn find_logic_entry(skill_dir: &Path) -> Option<String> {
    ["logic.ts", "logic.js", "logic.mjs", "logic.cjs"]
        .iter()
        .find(|name| skill_dir.join(name).exists())
        .map(|name| (*name).to_string())
}

fn parse_markdown_tool_specs(skill_dir: &Path) -> Vec<MarkdownToolSpec> {
    let text = match fs::read_to_string(skill_dir.join("SKILL.md")) {
        Ok(text) => text,
        Err(_) => return Vec::new(),
    };
    let mut specs = Vec::new();
    let mut current: Option<MarkdownToolSpec> = None;
    let mut in_tools = false;
    let mut in_args = false;

    for raw_line in text.lines() {
        let line = raw_line.trim();
        if line.starts_with("## ") {
            in_tools = line.eq_ignore_ascii_case("## Tools");
            in_args = false;
            if !in_tools {
                if let Some(tool) = current.take() {
                    specs.push(tool);
                }
            }
            continue;
        }
        if !in_tools {
            continue;
        }
        if let Some(name) = line.strip_prefix("### ") {
            if let Some(tool) = current.take() {
                specs.push(tool);
            }
            current = Some(MarkdownToolSpec {
                name: name.trim().to_string(),
                args: Vec::new(),
            });
            in_args = false;
            continue;
        }
        if line.starts_with("**Args:**") {
            in_args = true;
            continue;
        }
        if in_args {
            if let Some(rest) = line.strip_prefix("- `") {
                if let Some((arg_name, tail)) = rest.split_once("`") {
                    let (arg_type, description) = if let Some(start) = tail.find('(') {
                        if let Some(end) = tail[start + 1..].find(')') {
                            let ty = tail[start + 1..start + 1 + end]
                                .split(',')
                                .next()
                                .unwrap_or("string")
                                .trim()
                                .to_string();
                            let desc = tail[start + 1 + end + 1..]
                                .trim()
                                .trim_start_matches('-')
                                .trim()
                                .to_string();
                            (ty, desc)
                        } else {
                            ("string".to_string(), tail.trim().to_string())
                        }
                    } else {
                        ("string".to_string(), tail.trim().to_string())
                    };
                    if let Some(tool) = current.as_mut() {
                        tool.args.push(MarkdownArgSpec {
                            name: arg_name.trim().to_string(),
                            ty: arg_type,
                            description,
                        });
                    }
                    continue;
                }
            }
            if line.is_empty() || line.starts_with("### ") || line.starts_with("## ") {
                in_args = false;
            }
        }
    }

    if let Some(tool) = current.take() {
        specs.push(tool);
    }
    specs
}

fn markdown_arg_type_to_schema(ty: &str) -> &'static str {
    match ty.trim().to_ascii_lowercase().as_str() {
        "boolean" | "bool" => "boolean",
        "number" | "float" | "double" | "int" | "integer" => "number",
        "array" | "list" => "array",
        "object" => "object",
        _ => "string",
    }
}

fn build_tool_parameters_from_markdown(
    tool_specs: &[MarkdownToolSpec],
    default_tool: Option<&str>,
) -> Value {
    if let Some(default_tool) = default_tool {
        if let Some(tool) = tool_specs.iter().find(|spec| spec.name == default_tool) {
            let properties = tool
                .args
                .iter()
                .map(|arg| {
                    (
                        arg.name.clone(),
                        serde_json::json!({
                            "type": markdown_arg_type_to_schema(&arg.ty),
                            "description": arg.description,
                        }),
                    )
                })
                .collect::<serde_json::Map<String, Value>>();
            return serde_json::json!({
                "type": "object",
                "properties": properties,
                "required": []
            });
        }
    }
    serde_json::json!({
        "type": "object",
        "properties": {
            "tool": {
                "type": "string",
                "description": "要调用的 TypeScript tool 名称"
            },
            "args": {
                "type": "object",
                "description": "传给 tool.execute() 的参数对象"
            }
        },
        "required": []
    })
}

fn adapter_script_python() -> &'static str {
    r#"#!/usr/bin/env python3
import json
import os
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent
MANIFEST = json.loads((ROOT / "skill_manifest.json").read_text(encoding="utf-8"))

def load_payload():
    payload = {}
    raw = os.environ.get("SKILL_ARGS_JSON")
    if raw:
        try:
            return json.loads(raw)
        except Exception:
            return {}
    for arg in sys.argv[1:]:
        if arg.startswith("--args="):
            try:
                return json.loads(arg.split("=", 1)[1])
            except Exception:
                return {}
    return payload

def append_arg(cmd, key, value):
    flag = f"--{key.replace('_', '-')}"
    if isinstance(value, bool):
        if value:
            cmd.append(flag)
        return
    if isinstance(value, list):
        for item in value:
            append_arg(cmd, key, item)
        return
    if value is None:
        return
    cmd.extend([flag, str(value)])

def main():
    payload = load_payload()
    mode = MANIFEST.get("adapter_mode", "dispatch")
    if mode == "doc":
        content = (ROOT / "SKILL.md").read_text(encoding="utf-8") if (ROOT / "SKILL.md").exists() else ""
        focus = payload.get("question") or payload.get("topic") or ""
        if focus:
            print(f"[Focus] {focus}\n")
        print(content[:8000] if content else MANIFEST.get("description", ""))
        return 0

    actions = MANIFEST.get("adapter_actions", {})
    if not isinstance(actions, dict) or not actions:
        print("No adapter actions configured", file=sys.stderr)
        return 1

    action = payload.get("action")
    if not action and len(actions) == 1:
        action = next(iter(actions.keys()))
    if not action or action not in actions:
        print(f"Unknown action: {action}. Available: {', '.join(actions.keys())}", file=sys.stderr)
        return 1

    script_rel = actions[action]
    script_path = ROOT / script_rel
    if not script_path.exists():
        print(f"Missing script: {script_rel}", file=sys.stderr)
        return 1

    args_obj = payload.get("args")
    if not isinstance(args_obj, dict):
        args_obj = {k: v for k, v in payload.items() if k != "action"}

    cmd = ["python3" if script_path.suffix == ".py" else "bash", str(script_path)]
    for key, value in args_obj.items():
        append_arg(cmd, key, value)

    result = subprocess.run(cmd, capture_output=True, text=True)
    output = (result.stdout or "").strip() or (result.stderr or "").strip()
    if output:
        print(output)
    return result.returncode

if __name__ == "__main__":
    raise SystemExit(main())
"#
}

fn adapter_script_node_tooldef() -> &'static str {
    r#"import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const ROOT = path.dirname(fileURLToPath(import.meta.url));
const MANIFEST = JSON.parse(fs.readFileSync(path.join(ROOT, 'skill_manifest.json'), 'utf8'));

function loadPayload() {
  const raw = process.env.SKILL_ARGS_JSON;
  if (raw) {
    try { return JSON.parse(raw); } catch {}
  }
  for (const arg of process.argv.slice(2)) {
    if (arg.startsWith('--args=')) {
      try { return JSON.parse(arg.slice(7)); } catch {}
    }
  }
  return {};
}

const payload = loadPayload();
const entry = MANIFEST.adapter_entry || 'logic.ts';
const mod = await import(pathToFileURL(path.join(ROOT, entry)).href);
const tools = Array.isArray(mod.tools)
  ? mod.tools
  : Object.values(mod).filter((value) => value && typeof value === 'object' && typeof value.name === 'string' && typeof value.execute === 'function');

if (!tools.length) {
  console.error('No exported tools found in module:', entry);
  process.exit(1);
}

let toolName = payload.tool || MANIFEST.default_tool || '';
if (!toolName && tools.length === 1) {
  toolName = tools[0].name;
}

const tool = tools.find((item) => item.name === toolName);
if (!tool) {
  console.error(`Unknown tool: ${toolName}. Available: ${tools.map((item) => item.name).join(', ')}`);
  process.exit(1);
}

const args = payload.args && typeof payload.args === 'object' && !Array.isArray(payload.args)
  ? payload.args
  : Object.fromEntries(Object.entries(payload).filter(([key]) => key !== 'tool'));

const result = await tool.execute(args);
if (typeof result === 'string') {
  console.log(result);
} else {
  console.log(JSON.stringify(result ?? null, null, 2));
}
"#
}

fn ensure_auto_generated_manifest(skill_dir: &Path, skill_dir_name: &str) -> anyhow::Result<()> {
    let manifest_path = skill_dir.join("skill_manifest.json");
    if manifest_path.exists() {
        return Ok(());
    }

    let origin = read_json_path(&skill_dir.join(".clawhub/origin.json")).unwrap_or(Value::Null);
    let skill_json = read_json_path(&skill_dir.join("skill.json")).unwrap_or(Value::Null);
    let has_skill_md = skill_dir.join("SKILL.md").exists();
    if origin.is_null() && !has_skill_md && skill_json.is_null() {
        return Ok(());
    }

    let scripts = collect_skill_scripts(skill_dir);
    let tool_name = infer_tool_name_from_dir(skill_dir, skill_dir_name);
    let description = infer_skill_description_from_dir(skill_dir, skill_dir_name);
    let adapter_path = skill_dir.join("__clawhub_adapter.py");
    let logic_entry = find_logic_entry(skill_dir);
    let markdown_tool_specs = parse_markdown_tool_specs(skill_dir);

    if let Some(entry) = logic_entry {
        let default_tool = if markdown_tool_specs.len() == 1 {
            Some(markdown_tool_specs[0].name.clone())
        } else {
            None
        };
        let manifest = serde_json::json!({
            "tool_name": default_tool
                .as_deref()
                .map(sanitize_tool_name)
                .filter(|name| !name.is_empty())
                .unwrap_or_else(|| infer_tool_name_from_dir(skill_dir, skill_dir_name)),
            "skill_dir": skill_dir_name,
            "script": "__clawhub_tooldef_adapter.mjs",
            "description": if let Some(tool_name) = default_tool.as_deref() {
                format!("{}。TypeScript tool: {}", description, tool_name)
            } else {
                format!("{}。TypeScript tools wrapper", description)
            },
            "trusted": false,
            "adapter_mode": "node_tooldef",
            "adapter_entry": entry,
            "default_tool": default_tool,
            "parameters": build_tool_parameters_from_markdown(&markdown_tool_specs, default_tool.as_deref())
        });
        fs::write(
            skill_dir.join("__clawhub_tooldef_adapter.mjs"),
            adapter_script_node_tooldef(),
        )?;
        fs::write(&manifest_path, serde_json::to_string_pretty(&manifest)?)?;
        tracing::info!(
            skill_dir = %skill_dir_name,
            tool_name = %manifest.get("tool_name").and_then(|v| v.as_str()).unwrap_or(skill_dir_name),
            adapter_entry = %manifest.get("adapter_entry").and_then(|v| v.as_str()).unwrap_or_default(),
            "auto-generated TypeScript clawhub adapter"
        );
        return Ok(());
    }

    let manifest = if scripts.is_empty() {
        serde_json::json!({
            "tool_name": tool_name,
            "skill_dir": skill_dir_name,
            "script": "__clawhub_adapter.py",
            "description": format!("{}（文档型 ClawHub skill 适配器）", description),
            "trusted": false,
            "adapter_mode": "doc",
            "parameters": {
                "type": "object",
                "properties": {
                    "question": { "type": "string", "description": "想关注的审计问题或阅读重点" }
                },
                "required": []
            }
        })
    } else {
        let actions = scripts
            .iter()
            .map(|(relative, stem)| (stem.clone(), Value::String(relative.clone())))
            .collect::<serde_json::Map<String, Value>>();
        let action_list = actions.keys().cloned().collect::<Vec<_>>().join(", ");
        serde_json::json!({
            "tool_name": tool_name,
            "skill_dir": skill_dir_name,
            "script": "__clawhub_adapter.py",
            "description": format!("{}。可用 action: {}", description, action_list),
            "trusted": false,
            "adapter_mode": "dispatch",
            "adapter_actions": actions,
            "parameters": {
                "type": "object",
                "properties": {
                    "action": {
                        "type": "string",
                        "description": format!("要执行的子动作。可选: {}", action_list)
                    },
                    "args": {
                        "type": "object",
                        "description": "传给对应脚本的参数对象，会自动转成 --key value 命令行参数"
                    }
                },
                "required": scripts.len().gt(&1).then_some(vec!["action"]).unwrap_or_default()
            }
        })
    };

    fs::write(&adapter_path, adapter_script_python())?;
    fs::write(&manifest_path, serde_json::to_string_pretty(&manifest)?)?;
    tracing::info!(
        skill_dir = %skill_dir_name,
        tool_name = %manifest.get("tool_name").and_then(|v| v.as_str()).unwrap_or(skill_dir_name),
        generated_script_count = scripts.len(),
        "auto-generated clawhub skill adapter"
    );
    Ok(())
}

fn script_kind(script: &str) -> &'static str {
    if script.ends_with(".py") {
        "python"
    } else if script.ends_with(".ts")
        || script.ends_with(".js")
        || script.ends_with(".mjs")
        || script.ends_with(".cjs")
    {
        "node"
    } else if script.ends_with(".sh") || script.ends_with(".bash") || script.ends_with(".zsh") {
        "shell"
    } else {
        "unknown"
    }
}

fn infer_version_or_ref(origin: &Value) -> Option<String> {
    [
        &["resolvedVersion"][..],
        &["version"][..],
        &["ref"][..],
        &["tag"][..],
        &["source", "version"][..],
        &["source", "ref"][..],
        &["source", "tag"][..],
    ]
    .iter()
    .find_map(|path| nested_str(origin, path).map(str::to_string))
}

fn infer_description(origin: &Value, fallback: &str) -> String {
    [
        &["description"][..],
        &["summary"][..],
        &["package", "description"][..],
        &["skill", "description"][..],
    ]
    .iter()
    .find_map(|path| nested_str(origin, path))
    .map(str::to_string)
    .unwrap_or_else(|| fallback.to_string())
}

fn compute_risk_level(
    tool_name: &str,
    script: &str,
    managed_by: &str,
    trusted: bool,
) -> &'static str {
    if HIGH_RISK_TOOL_NAMES.contains(&tool_name)
        || script_kind(script) == "shell"
        || tool_name.contains("terminal")
        || tool_name.contains("editor")
        || tool_name.contains("write_")
        || tool_name.contains("pip")
    {
        "high"
    } else if managed_by == "clawhub" && !trusted {
        "medium"
    } else {
        "low"
    }
}

fn is_rust_builtin_tool(tool_name: &str) -> bool {
    RUST_BUILTIN_TOOL_NAMES.contains(&tool_name)
}

fn default_enabled_for_skill(executable: bool, risk_level: &str) -> bool {
    executable && risk_level != "high"
}

fn skill_status(meta: &InstalledSkillMeta) -> &'static str {
    if meta.install_error.is_some() {
        "invalid_manifest"
    } else if !meta.has_manifest {
        "installed_not_runnable"
    } else if !meta.executable {
        "unsupported_script"
    } else {
        "ready"
    }
}

fn validate_manifest(
    manifest: &Value,
    skill_dir_name: &str,
    skill_dir: &Path,
) -> anyhow::Result<()> {
    let required = [
        "tool_name",
        "skill_dir",
        "script",
        "description",
        "parameters",
    ];
    for field in required {
        if manifest.get(field).is_none() {
            return Err(anyhow!("skill_manifest.json 缺少字段: {}", field));
        }
    }

    let declared_dir = manifest
        .get("skill_dir")
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    if declared_dir != skill_dir_name {
        return Err(anyhow!(
            "skill_dir 值 '{}' 与目录名 '{}' 不符",
            declared_dir,
            skill_dir_name
        ));
    }

    let script = manifest
        .get("script")
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    let tool_name = manifest
        .get("tool_name")
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    if !skill_dir.join(script).exists() && !is_rust_builtin_tool(tool_name) {
        return Err(anyhow!("script 指定的文件不存在: {}", script));
    }

    if manifest
        .get("parameters")
        .and_then(|v| v.get("type"))
        .and_then(|v| v.as_str())
        != Some("object")
    {
        return Err(anyhow!(
            "parameters 必须是 JSON Schema object（含 'type': 'object'）"
        ));
    }

    Ok(())
}

impl ConfigService {
    /// 启动时扫描 `skills/` 目录，将磁盘上的 `skill_manifest.json` 与
    /// `skill_registry.json` 同步：新增的补进去，目录已删除的移除。
    pub fn scan_and_sync_skills(&self) -> anyhow::Result<()> {
        let skills_dir = self.project_root().join("skills");
        if !skills_dir.exists() {
            tracing::warn!("skills/ 目录不存在，跳过扫描");
            return Ok(());
        }

        let mut disk_skills: Vec<Value> = Vec::new();
        let mut entries: Vec<_> = fs::read_dir(&skills_dir)?
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
            .collect();
        entries.sort_by_key(|e| e.file_name());

        for entry in entries {
            let skill_dir = entry.path();
            let skill_dir_name = entry.file_name().to_string_lossy().to_string();
            if let Err(err) = ensure_auto_generated_manifest(&skill_dir, &skill_dir_name) {
                tracing::warn!("自动适配技能 {} 失败: {}", skill_dir_name, err);
            }
            let manifest_path = skill_dir.join("skill_manifest.json");
            if !manifest_path.exists() {
                continue;
            }

            let text = match fs::read_to_string(&manifest_path) {
                Ok(text) => text,
                Err(err) => {
                    tracing::warn!("读取 {} 失败: {}", manifest_path.display(), err);
                    continue;
                }
            };
            let mut manifest: Value = match serde_json::from_str(&text) {
                Ok(manifest) => manifest,
                Err(err) => {
                    tracing::warn!("解析 {} 失败: {}", manifest_path.display(), err);
                    continue;
                }
            };

            if let Err(err) = validate_manifest(&manifest, &skill_dir_name, &skill_dir) {
                tracing::warn!("跳过无效技能 {}: {}", skill_dir_name, err);
                continue;
            }

            let origin =
                read_json_path(&skill_dir.join(".clawhub/origin.json")).unwrap_or(Value::Null);
            let managed_by = if origin.is_null() { "local" } else { "clawhub" };
            let install_source = if managed_by == "clawhub" {
                Some("clawhub".to_string())
            } else {
                None
            };
            let script = manifest
                .get("script")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();
            let tool_name = manifest
                .get("tool_name")
                .and_then(|v| v.as_str())
                .unwrap_or(&skill_dir_name)
                .to_string();
            let trusted = manifest
                .get("trusted")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let risk_level = compute_risk_level(&tool_name, &script, managed_by, trusted);
            let runtime = serde_json::json!({
                "executable": is_rust_builtin_tool(&tool_name) || script_kind(&script) != "unknown",
                "has_skill_md": skill_dir.join("SKILL.md").exists(),
                "script_kind": script_kind(&script),
                "execution_mode": if is_rust_builtin_tool(&tool_name) { "rust_builtin" } else { "script" },
            });

            if let Some(obj) = manifest.as_object_mut() {
                obj.insert(
                    "managed_by".to_string(),
                    Value::String(managed_by.to_string()),
                );
                obj.insert(
                    "install_source".to_string(),
                    install_source.map(Value::String).unwrap_or(Value::Null),
                );
                obj.insert("origin".to_string(), origin.clone());
                obj.insert(
                    "version_or_ref".to_string(),
                    infer_version_or_ref(&origin)
                        .map(Value::String)
                        .unwrap_or(Value::Null),
                );
                obj.insert(
                    "risk_level".to_string(),
                    Value::String(risk_level.to_string()),
                );
                obj.insert("runtime".to_string(), runtime);
            }

            tracing::debug!("扫描到技能: {} ({})", tool_name, skill_dir_name);
            disk_skills.push(manifest);
        }

        let registry_path = self
            .project_root()
            .join("siliconflow/data/skill_registry.json");
        let existing: Vec<Value> = if registry_path.exists() {
            let text = fs::read_to_string(&registry_path).unwrap_or_default();
            serde_json::from_str(&text).unwrap_or_default()
        } else {
            vec![]
        };

        let existing_names: BTreeSet<String> = existing
            .iter()
            .filter_map(|v| {
                v.get("tool_name")
                    .and_then(|n| n.as_str())
                    .map(String::from)
            })
            .collect();
        let disk_names: BTreeSet<String> = disk_skills
            .iter()
            .filter_map(|v| {
                v.get("tool_name")
                    .and_then(|n| n.as_str())
                    .map(String::from)
            })
            .collect();

        let added: BTreeSet<_> = disk_names.difference(&existing_names).cloned().collect();
        let removed: BTreeSet<_> = existing_names.difference(&disk_names).cloned().collect();
        let registry_changed = existing != disk_skills;

        if registry_changed {
            if let Some(parent) = registry_path.parent() {
                fs::create_dir_all(parent)?;
            }
            let json = serde_json::to_string_pretty(&disk_skills)?;
            fs::write(&registry_path, &json)?;
            tracing::info!(
                "技能注册表已更新: {} 个技能 (+{} -{})",
                disk_skills.len(),
                added.len(),
                removed.len()
            );
        } else {
            tracing::info!(
                "技能扫描完成，共 {} 个技能，无注册表变更",
                disk_skills.len()
            );
        }

        self.sync_skill_settings(&disk_skills, &added, &removed)?;
        Ok(())
    }

    pub fn skills_view(&self) -> anyhow::Result<Vec<Value>> {
        let registry =
            self.read_json_file("siliconflow/data/skill_registry.json", Value::Array(vec![]))?;
        let settings = self.read_json_file(
            "siliconflow/config/skill_settings.json",
            serde_json::json!({ "entries": {} }),
        )?;
        let installed = self.installed_skill_metadata()?;

        let registry_items = registry.as_array().cloned().unwrap_or_default();
        let settings_entries = settings
            .get("entries")
            .and_then(|v| v.as_object())
            .cloned()
            .unwrap_or_default();

        let mut registry_by_name = BTreeMap::new();
        for skill in registry_items {
            if let Some(name) = skill.get("tool_name").and_then(|v| v.as_str()) {
                registry_by_name.insert(name.to_string(), skill);
            }
        }

        let mut names: BTreeSet<String> = registry_by_name.keys().cloned().collect();
        names.extend(installed.keys().cloned());

        let mut items = Vec::new();
        for name in names {
            let registry_item = registry_by_name.get(&name);
            let meta = installed.get(&name).cloned().unwrap_or_else(|| {
                let skill_dir = registry_item
                    .and_then(|item| item.get("skill_dir"))
                    .and_then(|v| v.as_str())
                    .unwrap_or(&name)
                    .to_string();
                let description = registry_item
                    .and_then(|item| item.get("description"))
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string();
                let managed_by = registry_item
                    .and_then(|item| item.get("managed_by"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("local")
                    .to_string();
                let origin = registry_item
                    .and_then(|item| item.get("origin"))
                    .cloned()
                    .unwrap_or(Value::Null);
                let risk_level = registry_item
                    .and_then(|item| item.get("risk_level"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("low")
                    .to_string();
                let version_or_ref = registry_item
                    .and_then(|item| item.get("version_or_ref"))
                    .and_then(|v| v.as_str())
                    .map(str::to_string);
                let executable = registry_item
                    .and_then(|item| item.get("runtime"))
                    .and_then(|v| v.get("executable"))
                    .and_then(|v| v.as_bool())
                    .unwrap_or(true);
                InstalledSkillMeta {
                    name: name.clone(),
                    skill_dir,
                    description,
                    has_manifest: true,
                    has_skill_md: registry_item
                        .and_then(|item| item.get("runtime"))
                        .and_then(|v| v.get("has_skill_md"))
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false),
                    executable,
                    managed_by: managed_by.clone(),
                    install_source: (managed_by == "clawhub").then(|| "clawhub".to_string()),
                    origin,
                    version_or_ref,
                    risk_level,
                    install_error: None,
                }
            });

            let cfg = settings_entries.get(&name).cloned().unwrap_or_else(|| {
                default_skill_config(default_enabled_for_skill(meta.executable, &meta.risk_level))
            });
            let description_value = if meta.description.is_empty() {
                registry_item
                    .and_then(|item| item.get("description"))
                    .cloned()
                    .unwrap_or(Value::String(String::new()))
            } else {
                Value::String(meta.description.clone())
            };
            let secret_ready = cfg
                .get("api_key_ref")
                .and_then(|v| v.as_object())
                .map(|api_key_ref| {
                    let source = api_key_ref
                        .get("source")
                        .and_then(|v| v.as_str())
                        .unwrap_or("env");
                    if source != "env" {
                        return true;
                    }
                    let id = api_key_ref
                        .get("id")
                        .and_then(|v| v.as_str())
                        .unwrap_or_default();
                    !id.is_empty()
                        && self
                            .snapshot()
                            .env
                            .get(id)
                            .map(|v| !v.trim().is_empty())
                            .unwrap_or(false)
                })
                .unwrap_or(true);

            items.push(serde_json::json!({
                "name": meta.name,
                "skill_dir": meta.skill_dir,
                "description": description_value,
                "enabled": cfg.get("enabled").and_then(|v| v.as_bool()).unwrap_or(false),
                "installed": true,
                "executable": meta.executable,
                "has_manifest": meta.has_manifest,
                "has_skill_md": meta.has_skill_md,
                "managed_by": meta.managed_by,
                "install_source": meta.install_source,
                "origin": meta.origin,
                "version_or_ref": meta.version_or_ref,
                "risk_level": meta.risk_level,
                "status": skill_status(&meta),
                "install_error": meta.install_error,
                "config": {
                    "api_key_ref": cfg.get("api_key_ref").cloned().unwrap_or(Value::Null),
                    "env": cfg.get("env").cloned().unwrap_or_else(|| serde_json::json!({})),
                    "secret_ready": secret_ready
                }
            }));
        }
        Ok(items)
    }

    pub fn toggle_skill(&self, name: &str, enabled: bool) -> anyhow::Result<Vec<Value>> {
        let installed = self.installed_skill_metadata()?;
        if let Some(meta) = installed.get(name) {
            if !meta.executable {
                return Err(anyhow!("skill_not_runnable"));
            }
        }

        let path = self
            .project_root()
            .join("siliconflow/config/skill_settings.json");
        let text = fs::read_to_string(&path)?;
        let mut raw: Value = serde_json::from_str(&text)?;
        let entries = raw
            .get_mut("entries")
            .and_then(|v| v.as_object_mut())
            .ok_or_else(|| anyhow!("skill_settings.json 缺少 entries"))?;
        let item = entries
            .entry(name.to_string())
            .or_insert_with(|| default_skill_config(enabled));
        let obj = item
            .as_object_mut()
            .ok_or_else(|| anyhow!("无效的 skill 配置项"))?;
        obj.insert("enabled".to_string(), Value::Bool(enabled));
        fs::write(&path, serde_json::to_string_pretty(&raw)?)?;
        self.skills_view()
    }

    pub fn update_skill_config(&self, name: &str, value: &Value) -> anyhow::Result<Vec<Value>> {
        let path = self
            .project_root()
            .join("siliconflow/config/skill_settings.json");
        let text = fs::read_to_string(&path)?;
        let mut raw: Value = serde_json::from_str(&text)?;
        let entries = raw
            .get_mut("entries")
            .and_then(|v| v.as_object_mut())
            .ok_or_else(|| anyhow!("skill_settings.json 缺少 entries"))?;
        entries.insert(name.to_string(), value.clone());
        fs::write(&path, serde_json::to_string_pretty(&raw)?)?;
        self.skills_view()
    }

    fn installed_skill_metadata(&self) -> anyhow::Result<BTreeMap<String, InstalledSkillMeta>> {
        let skills_dir = self.project_root().join("skills");
        let mut out = BTreeMap::new();
        if !skills_dir.exists() {
            return Ok(out);
        }

        let mut entries: Vec<_> = fs::read_dir(&skills_dir)?
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
            .collect();
        entries.sort_by_key(|e| e.file_name());

        for entry in entries {
            let skill_dir_path = entry.path();
            let skill_dir = entry.file_name().to_string_lossy().to_string();
            if let Err(err) = ensure_auto_generated_manifest(&skill_dir_path, &skill_dir) {
                tracing::warn!("自动适配技能 {} 失败: {}", skill_dir, err);
            }
            let manifest_path = skill_dir_path.join("skill_manifest.json");
            let has_manifest = manifest_path.exists();
            let has_skill_md = skill_dir_path.join("SKILL.md").exists();
            let origin =
                read_json_path(&skill_dir_path.join(".clawhub/origin.json")).unwrap_or(Value::Null);
            let managed_by = if origin.is_null() {
                "local".to_string()
            } else {
                "clawhub".to_string()
            };
            let install_source = (managed_by == "clawhub").then(|| "clawhub".to_string());

            let (name, description, executable, version_or_ref, risk_level, install_error) =
                if has_manifest {
                    match fs::read_to_string(&manifest_path) {
                        Ok(text) => match serde_json::from_str::<Value>(&text) {
                            Ok(manifest) => {
                                match validate_manifest(&manifest, &skill_dir, &skill_dir_path) {
                                    Ok(()) => {
                                        let name = manifest
                                            .get("tool_name")
                                            .and_then(|v| v.as_str())
                                            .unwrap_or(&skill_dir)
                                            .to_string();
                                        let description = manifest
                                            .get("description")
                                            .and_then(|v| v.as_str())
                                            .unwrap_or_default()
                                            .to_string();
                                        let script = manifest
                                            .get("script")
                                            .and_then(|v| v.as_str())
                                            .unwrap_or_default()
                                            .to_string();
                                        let trusted = manifest
                                            .get("trusted")
                                            .and_then(|v| v.as_bool())
                                            .unwrap_or(false);
                                        let risk_level = compute_risk_level(
                                            &name,
                                            &script,
                                            &managed_by,
                                            trusted,
                                        )
                                        .to_string();
                                        (
                                            name,
                                            description,
                                            is_rust_builtin_tool(
                                                manifest
                                                    .get("tool_name")
                                                    .and_then(|v| v.as_str())
                                                    .unwrap_or_default(),
                                            ) || script_kind(&script) != "unknown",
                                            infer_version_or_ref(&origin),
                                            risk_level,
                                            None,
                                        )
                                    }
                                    Err(err) => (
                                        skill_dir.clone(),
                                        infer_description(&origin, &skill_dir),
                                        false,
                                        infer_version_or_ref(&origin),
                                        "high".to_string(),
                                        Some(err.to_string()),
                                    ),
                                }
                            }
                            Err(err) => (
                                skill_dir.clone(),
                                infer_description(&origin, &skill_dir),
                                false,
                                infer_version_or_ref(&origin),
                                "high".to_string(),
                                Some(format!("skill_manifest.json 解析失败: {}", err)),
                            ),
                        },
                        Err(err) => (
                            skill_dir.clone(),
                            infer_description(&origin, &skill_dir),
                            false,
                            infer_version_or_ref(&origin),
                            "high".to_string(),
                            Some(format!("读取 skill_manifest.json 失败: {}", err)),
                        ),
                    }
                } else {
                    (
                        skill_dir.clone(),
                        infer_description(&origin, &skill_dir),
                        false,
                        infer_version_or_ref(&origin),
                        "medium".to_string(),
                        None,
                    )
                };

            out.insert(
                name.clone(),
                InstalledSkillMeta {
                    name,
                    skill_dir,
                    description,
                    has_manifest,
                    has_skill_md,
                    executable,
                    managed_by,
                    install_source,
                    origin,
                    version_or_ref,
                    risk_level,
                    install_error,
                },
            );
        }

        Ok(out)
    }

    fn sync_skill_settings(
        &self,
        registry_items: &[Value],
        added: &BTreeSet<String>,
        removed: &BTreeSet<String>,
    ) -> anyhow::Result<()> {
        let path = self
            .project_root()
            .join("siliconflow/config/skill_settings.json");
        let mut raw = if path.exists() {
            serde_json::from_str::<Value>(&fs::read_to_string(&path)?)
                .unwrap_or_else(|_| serde_json::json!({ "entries": {} }))
        } else {
            serde_json::json!({ "entries": {} })
        };

        let entries = raw
            .get_mut("entries")
            .and_then(|v| v.as_object_mut())
            .ok_or_else(|| anyhow!("skill_settings.json 缺少 entries"))?;

        let registry_by_name = registry_items
            .iter()
            .filter_map(|item| {
                item.get("tool_name")
                    .and_then(|v| v.as_str())
                    .map(|name| (name.to_string(), item))
            })
            .collect::<BTreeMap<_, _>>();

        let mut changed = false;
        for name in added {
            if entries.contains_key(name) {
                continue;
            }
            let item = registry_by_name
                .get(name)
                .ok_or_else(|| anyhow!("新增 skill 缺少 registry 项: {}", name))?;
            let executable = item
                .get("runtime")
                .and_then(|v| v.get("executable"))
                .and_then(|v| v.as_bool())
                .unwrap_or(true);
            let risk_level = item
                .get("risk_level")
                .and_then(|v| v.as_str())
                .unwrap_or("low");
            entries.insert(
                name.clone(),
                default_skill_config(default_enabled_for_skill(executable, risk_level)),
            );
            changed = true;
        }

        for name in removed {
            if entries.remove(name).is_some() {
                changed = true;
            }
        }

        if changed {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(&path, serde_json::to_string_pretty(&raw)?)?;
        }
        Ok(())
    }
}
