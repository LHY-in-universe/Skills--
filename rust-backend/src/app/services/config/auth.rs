//! auth_profiles + doctor 修复 + permission_grants 相关方法。
//!
//! permission 部分以 SQLite 为权威源，JSON 只作一次性导入来源。

use crate::app::services::config_service::ConfigService;
use serde_json::Value;

impl ConfigService {
    pub fn auth_profiles(&self) -> anyhow::Result<Value> {
        self.read_json_file(
            "siliconflow/config/auth_profiles.json",
            serde_json::json!({ "providers": {} }),
        )
    }

    pub fn permission_settings(&self) -> anyhow::Result<Value> {
        let tools = self.permission_store().list_always_allowed()?;
        Ok(serde_json::json!({ "always_allow_tools": tools }))
    }

    pub fn is_tool_always_allowed(&self, tool_name: &str) -> bool {
        self.permission_store().is_always_allowed(tool_name)
    }

    pub fn allow_tool_always(&self, tool_name: &str) -> anyhow::Result<Value> {
        self.permission_store()
            .upsert(tool_name, true, "user_grant")?;
        self.permission_settings()
    }

    pub fn doctor_fix(&self, dry_run: bool) -> anyhow::Result<Value> {
        let mut skipped = vec!["真实修复逻辑尚未迁移到 Rust".to_string()];
        if dry_run {
            skipped.insert(0, "dry_run".to_string());
        }
        Ok(serde_json::json!({
            "applied": [],
            "skipped": skipped
        }))
    }
}
