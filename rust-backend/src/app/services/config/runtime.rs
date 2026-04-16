//! Runtime / Lark / terminal / 语音 worker 相关配置。
//!
//! 都是轻量的 JSON 或 `.env` 读写，集中在一起避免 config_service.rs 继续膨胀。

use crate::app::services::config_service::ConfigService;
use crate::domain::models::RuntimeSettings;
use anyhow::anyhow;
use serde_json::Value;
use std::path::PathBuf;

impl ConfigService {
    pub fn save_runtime_settings(&self, value: &Value) -> anyhow::Result<RuntimeSettings> {
        self.write_json_file("siliconflow/config/runtime_config.json", value)?;
        self.reload()?;
        Ok(self.runtime_settings())
    }

    pub fn lark_config(&self) -> anyhow::Result<Value> {
        let env = self.read_env_file()?;
        let app_id = env.get("LARK_APP_ID").cloned().unwrap_or_default();
        let has_app_secret = env
            .get("LARK_APP_SECRET")
            .map(|v| !v.trim().is_empty())
            .unwrap_or(false);
        Ok(serde_json::json!({
            "app_id": app_id,
            "has_app_secret": has_app_secret
        }))
    }

    pub fn save_lark_config(&self, app_id: &str, app_secret: &str) -> anyhow::Result<Value> {
        let mut env = self.read_env_file()?;
        env.insert("LARK_APP_ID".to_string(), app_id.trim().to_string());
        env.insert("LARK_APP_SECRET".to_string(), app_secret.trim().to_string());
        self.write_env_file(&env)?;
        self.reload()?;
        self.lark_config()
    }

    pub fn terminal_cwd(&self) -> anyhow::Result<Value> {
        self.read_json_file(
            "siliconflow/config/terminal.json",
            serde_json::json!({ "cwd": self.project_root().to_string_lossy().to_string() }),
        )
    }

    pub fn save_terminal_cwd(&self, cwd: &str) -> anyhow::Result<Value> {
        let path = PathBuf::from(cwd.trim());
        if !path.is_dir() {
            return Err(anyhow!("invalid_cwd"));
        }
        let canonical = path.canonicalize().unwrap_or(path);
        let value = serde_json::json!({
            "cwd": canonical.to_string_lossy().to_string()
        });
        self.write_json_file("siliconflow/config/terminal.json", &value)?;
        Ok(value)
    }

    pub fn voice_bridge_upstream_ws_url(&self) -> String {
        self.read_env_file()
            .ok()
            .and_then(|env| env.get("VOICE_BRIDGE_UPSTREAM_WS_URL").cloned())
            .filter(|v| !v.trim().is_empty())
            .unwrap_or_else(|| "ws://127.0.0.1:8000/api/voice/bridge".to_string())
    }

    pub fn voice_worker_script(&self) -> PathBuf {
        self.project_root().join("webapp/backend/voice_worker.py")
    }

    pub fn voice_worker_python(&self) -> PathBuf {
        let venv_python = self.project_root().join("webapp/backend/venv/bin/python");
        if venv_python.exists() {
            return venv_python;
        }
        PathBuf::from("python3")
    }
}
