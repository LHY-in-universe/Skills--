//! 技能视图 + 开关 + 配置。
//!
//! `skill_registry.json` 做只读元数据，`skill_settings.json` 做可写覆盖。

use crate::app::services::config_service::ConfigService;
use anyhow::anyhow;
use serde_json::Value;
use std::fs;

impl ConfigService {
    pub fn skills_view(&self) -> anyhow::Result<Vec<Value>> {
        let registry =
            self.read_json_file("siliconflow/data/skill_registry.json", Value::Array(vec![]))?;
        let settings = self.read_json_file(
            "siliconflow/config/skill_settings.json",
            serde_json::json!({ "entries": {} }),
        )?;

        let registry_items = registry.as_array().cloned().unwrap_or_default();
        let settings_entries = settings
            .get("entries")
            .and_then(|v| v.as_object())
            .cloned()
            .unwrap_or_default();

        let mut items = Vec::new();
        for skill in registry_items {
            let tool_name = skill
                .get("tool_name")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();
            let cfg = settings_entries.get(&tool_name).cloned().unwrap_or_else(|| {
                serde_json::json!({
                    "enabled": true,
                    "api_key_ref": Value::Null,
                    "env": {}
                })
            });
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
                "name": tool_name,
                "description": skill.get("description").cloned().unwrap_or(Value::String(String::new())),
                "enabled": cfg.get("enabled").and_then(|v| v.as_bool()).unwrap_or(true),
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
        let path = self.project_root().join("siliconflow/config/skill_settings.json");
        let text = fs::read_to_string(&path)?;
        let mut raw: Value = serde_json::from_str(&text)?;
        let entries = raw
            .get_mut("entries")
            .and_then(|v| v.as_object_mut())
            .ok_or_else(|| anyhow!("skill_settings.json 缺少 entries"))?;
        let item = entries.entry(name.to_string()).or_insert_with(|| {
            serde_json::json!({ "enabled": true, "api_key_ref": Value::Null, "env": {} })
        });
        let obj = item
            .as_object_mut()
            .ok_or_else(|| anyhow!("无效的 skill 配置项"))?;
        obj.insert("enabled".to_string(), Value::Bool(enabled));
        fs::write(&path, serde_json::to_string_pretty(&raw)?)?;
        self.skills_view()
    }

    pub fn update_skill_config(
        &self,
        name: &str,
        value: &Value,
    ) -> anyhow::Result<Vec<Value>> {
        let path = self.project_root().join("siliconflow/config/skill_settings.json");
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
}
