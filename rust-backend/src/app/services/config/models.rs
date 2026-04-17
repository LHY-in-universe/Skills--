//! 模型 CRUD + providers_catalog。
//!
//! 操作仍以 `rust-backend/config/models.json` 为事实源，避免 Rust / Python 在过渡期
//! 分叉。写完一律 `self.reload()` 让快照同步。

use crate::app::services::config_service::ConfigService;
use crate::domain::models::{ModelCreateRequest, ModelSpec, ModelUpdateRequest};
use anyhow::anyhow;
use serde_json::{Map, Value};
use std::collections::BTreeMap;
use std::fs;

impl ConfigService {
    pub fn set_current_model(
        &self,
        model_name: &str,
    ) -> anyhow::Result<crate::domain::models::ConfigView> {
        let models_path = self.project_root().join("rust-backend/config/models.json");
        let text = fs::read_to_string(&models_path)?;
        let mut raw: Value = serde_json::from_str(&text)?;
        let object = raw
            .as_object_mut()
            .ok_or_else(|| anyhow!("models.json 顶层必须是对象"))?;

        if !object.contains_key(model_name) {
            return Err(anyhow!("model_not_found"));
        }

        for (_, item) in object.iter_mut() {
            match item {
                Value::Object(obj) => {
                    obj.insert("enabled".to_string(), Value::Bool(false));
                }
                Value::String(existing_id) => {
                    let id = existing_id.clone();
                    *item = Value::Object(Map::from_iter([
                        ("id".to_string(), Value::String(id)),
                        ("provider".to_string(), Value::String("siliconflow".to_string())),
                        ("enabled".to_string(), Value::Bool(false)),
                    ]));
                }
                _ => {}
            }
        }

        if let Some(Value::Object(obj)) = object.get_mut(model_name) {
            obj.insert("enabled".to_string(), Value::Bool(true));
        }

        fs::write(&models_path, serde_json::to_string_pretty(&raw)?)?;
        self.reload()?;
        Ok(self.config_view())
    }

    pub fn add_model(
        &self,
        req: &ModelCreateRequest,
    ) -> anyhow::Result<BTreeMap<String, ModelSpec>> {
        let models_path = self.project_root().join("rust-backend/config/models.json");
        let text = fs::read_to_string(&models_path)?;
        let mut raw: Value = serde_json::from_str(&text)?;
        let object = raw
            .as_object_mut()
            .ok_or_else(|| anyhow!("models.json 顶层必须是对象"))?;

        if object.contains_key(&req.name) {
            return Err(anyhow!("model_already_exists"));
        }

        object.insert(
            req.name.clone(),
            Value::Object(Map::from_iter([
                ("id".to_string(), Value::String(req.model_id.clone())),
                (
                    "provider".to_string(),
                    Value::String(req.provider.clone().unwrap_or_else(|| "siliconflow".to_string())),
                ),
                ("enabled".to_string(), Value::Bool(false)),
                (
                    "api_url".to_string(),
                    req.api_url.clone().map(Value::String).unwrap_or(Value::Null),
                ),
            ])),
        );

        fs::write(&models_path, serde_json::to_string_pretty(&raw)?)?;
        self.reload()?;
        Ok(self.models_view())
    }

    pub fn update_model(
        &self,
        display_name: &str,
        req: &ModelUpdateRequest,
    ) -> anyhow::Result<BTreeMap<String, ModelSpec>> {
        let models_path = self.project_root().join("rust-backend/config/models.json");
        let text = fs::read_to_string(&models_path)?;
        let mut raw: Value = serde_json::from_str(&text)?;
        let object = raw
            .as_object_mut()
            .ok_or_else(|| anyhow!("models.json 顶层必须是对象"))?;

        let item = object
            .get_mut(display_name)
            .ok_or_else(|| anyhow!("model_not_found"))?;

        let obj = match item {
            Value::Object(obj) => obj,
            Value::String(existing_id) => {
                let mut rebuilt = Map::new();
                rebuilt.insert("id".to_string(), Value::String(existing_id.clone()));
                rebuilt.insert("provider".to_string(), Value::String("siliconflow".to_string()));
                rebuilt.insert("enabled".to_string(), Value::Bool(false));
                *item = Value::Object(rebuilt);
                item.as_object_mut().expect("重建后的模型配置必须是对象")
            }
            _ => return Err(anyhow!("invalid_model_entry")),
        };

        if let Some(model_id) = &req.model_id {
            obj.insert("id".to_string(), Value::String(model_id.clone()));
        }
        if let Some(provider) = &req.provider {
            obj.insert("provider".to_string(), Value::String(provider.clone()));
        }
        if let Some(api_url) = &req.api_url {
            obj.insert("api_url".to_string(), Value::String(api_url.clone()));
        }

        fs::write(&models_path, serde_json::to_string_pretty(&raw)?)?;
        self.reload()?;
        Ok(self.models_view())
    }

    pub fn delete_model(
        &self,
        display_name: &str,
    ) -> anyhow::Result<BTreeMap<String, ModelSpec>> {
        let models_path = self.project_root().join("rust-backend/config/models.json");
        let text = fs::read_to_string(&models_path)?;
        let mut raw: Value = serde_json::from_str(&text)?;
        let object = raw
            .as_object_mut()
            .ok_or_else(|| anyhow!("models.json 顶层必须是对象"))?;

        if object.remove(display_name).is_none() {
            return Err(anyhow!("model_not_found"));
        }

        if !object
            .values()
            .any(|v| v.get("enabled").and_then(|x| x.as_bool()).unwrap_or(false))
        {
            if let Some((_, first)) = object.iter_mut().next() {
                match first {
                    Value::Object(obj) => {
                        obj.insert("enabled".to_string(), Value::Bool(true));
                    }
                    Value::String(existing_id) => {
                        let id = existing_id.clone();
                        *first = Value::Object(Map::from_iter([
                            ("id".to_string(), Value::String(id)),
                            ("provider".to_string(), Value::String("siliconflow".to_string())),
                            ("enabled".to_string(), Value::Bool(true)),
                        ]));
                    }
                    _ => {}
                }
            }
        }

        fs::write(&models_path, serde_json::to_string_pretty(&raw)?)?;
        self.reload()?;
        Ok(self.models_view())
    }

    pub fn providers_catalog(&self) -> Vec<Value> {
        let snapshot = self.snapshot();
        snapshot
            .providers
            .values()
            .map(|provider| {
                serde_json::json!({
                    "id": provider.id,
                    "label": provider.label,
                    "default_api_url": provider.default_api_url,
                    "required_env_keys": provider.required_env_keys,
                })
            })
            .collect()
    }
}
