use crate::domain::models::{
    ModelCapabilities, ModelSpec, ProviderSpec, RuntimeSettings, RuntimeSnapshot,
};
use anyhow::Context;
use serde_json::Value;
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

/// 配置加载器。
///
/// 这层的职责非常明确：
/// - 从现有 Python 项目的配置文件读取内容
/// - 转成 Rust 强类型快照
/// - 在加载阶段完成最小归一化
///
/// 它不负责业务判断，也不负责运行时热更新策略。
pub struct ConfigLoader;

impl ConfigLoader {
    pub fn load_snapshot(project_root: PathBuf) -> anyhow::Result<RuntimeSnapshot> {
        let models_path = project_root.join("rust-backend/config/models.json");
        let providers_path = project_root.join("siliconflow/config/providers.json");
        let env_path = project_root.join("siliconflow/config/.env");
        let runtime_path = project_root.join("siliconflow/config/runtime_config.json");

        let models = Self::load_models(&models_path)
            .with_context(|| format!("读取模型配置失败: {}", models_path.display()))?;
        let providers = Self::load_providers(&providers_path)
            .with_context(|| format!("读取 provider 配置失败: {}", providers_path.display()))?;
        let env = Self::load_env(&env_path)
            .with_context(|| format!("读取环境变量失败: {}", env_path.display()))?;
        let runtime = Self::load_runtime(&runtime_path)
            .with_context(|| format!("读取运行时配置失败: {}", runtime_path.display()))?;

        Ok(RuntimeSnapshot {
            project_root,
            models,
            providers,
            env,
            runtime,
        })
    }

    fn load_models(path: &PathBuf) -> anyhow::Result<BTreeMap<String, ModelSpec>> {
        if !path.exists() {
            return Ok(BTreeMap::new());
        }
        let text = fs::read_to_string(path)?;
        let raw: Value = serde_json::from_str(&text)?;
        let mut out = BTreeMap::new();

        if let Value::Object(map) = raw {
            for (name, item) in map {
                match item {
                    Value::String(model_id) => {
                        out.insert(
                            name,
                            ModelSpec {
                                id: model_id,
                                provider: "siliconflow".to_string(),
                                api_url_override: None,
                                enabled: true,
                                capabilities: ModelCapabilities {
                                    chat: true,
                                    vision: false,
                                    tools: true,
                                },
                                requires: Vec::new(),
                            },
                        );
                    }
                    Value::Object(obj) => {
                        let api_url_override = obj
                            .get("api_url")
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string());

                        let capabilities = obj.get("capabilities").cloned().unwrap_or(Value::Null);
                        let chat = capabilities
                            .get("chat")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(true);
                        let vision = capabilities
                            .get("vision")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false);
                        let tools = capabilities
                            .get("tools")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(true);

                        let requires = obj
                            .get("requires")
                            .and_then(|v| v.as_array())
                            .map(|items| {
                                items
                                    .iter()
                                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                                    .collect::<Vec<_>>()
                            })
                            .unwrap_or_default();

                        out.insert(
                            name,
                            ModelSpec {
                                id: obj
                                    .get("id")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or_default()
                                    .to_string(),
                                provider: obj
                                    .get("provider")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("siliconflow")
                                    .to_string(),
                                api_url_override,
                                enabled: obj
                                    .get("enabled")
                                    .and_then(|v| v.as_bool())
                                    .unwrap_or(true),
                                capabilities: ModelCapabilities {
                                    chat,
                                    vision,
                                    tools,
                                },
                                requires,
                            },
                        );
                    }
                    _ => {}
                }
            }
        }

        Ok(out)
    }

    fn load_runtime(path: &PathBuf) -> anyhow::Result<RuntimeSettings> {
        if !path.exists() {
            return Ok(RuntimeSettings::default());
        }
        let text = fs::read_to_string(path)?;
        let settings: RuntimeSettings = serde_json::from_str(&text)?;
        Ok(settings)
    }

    fn load_providers(path: &PathBuf) -> anyhow::Result<BTreeMap<String, ProviderSpec>> {
        if !path.exists() {
            return Ok(BTreeMap::new());
        }
        let text = fs::read_to_string(path)?;
        let raw: Value = serde_json::from_str(&text)?;
        let mut out = BTreeMap::new();
        if let Some(items) = raw.get("providers").and_then(|v| v.as_array()) {
            for item in items {
                let id = item
                    .get("id")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string();
                if id.is_empty() {
                    continue;
                }
                out.insert(
                    id.clone(),
                    ProviderSpec {
                        id,
                        label: item
                            .get("label")
                            .and_then(|v| v.as_str())
                            .unwrap_or("Unknown")
                            .to_string(),
                        default_api_url: item
                            .get("default_api_url")
                            .and_then(|v| v.as_str())
                            .unwrap_or_default()
                            .to_string(),
                        required_env_keys: item
                            .get("required_env_keys")
                            .and_then(|v| v.as_array())
                            .map(|arr| {
                                arr.iter()
                                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                                    .collect::<Vec<_>>()
                            })
                            .unwrap_or_default(),
                    },
                );
            }
        }
        Ok(out)
    }

    fn load_env(path: &PathBuf) -> anyhow::Result<BTreeMap<String, String>> {
        let mut out = BTreeMap::new();
        if !path.exists() {
            return Ok(out);
        }
        let text = fs::read_to_string(path)?;
        for line in text.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            if let Some((key, value)) = trimmed.split_once('=') {
                out.insert(key.trim().to_string(), value.trim().to_string());
            }
        }
        Ok(out)
    }
}
