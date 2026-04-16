//! `ConfigService` 门面。
//!
//! 本文件只放 struct 定义、构造、快照访问、通用 JSON/env 工具方法和 doctor 报告。
//! 其余 models / skills / routing / runtime / observability / auth 方法已经按
//! 职责搬到 `services/config/*.rs`，通过 `impl ConfigService` 块附加到本类型上。

use crate::domain::doctor::{DoctorFinding, DoctorReport};
use crate::domain::models::{
    ConfigView, ModelSpec, ProviderDisplay, RuntimeSettings, RuntimeSnapshot,
};
use crate::infra::config_loader::ConfigLoader;
use crate::infra::execution_store::ExecutionStore;
use crate::infra::memory_store::MemoryStore;
use crate::infra::permission_store::PermissionStore;
use crate::infra::token_store::TokenStore;
use serde_json::Value;
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

#[derive(Clone)]
pub struct ConfigService {
    project_root: PathBuf,
    snapshot: Arc<RwLock<RuntimeSnapshot>>,
    permission_store: PermissionStore,
    token_store: TokenStore,
    execution_store: ExecutionStore,
    memory_store: MemoryStore,
}

impl ConfigService {
    pub fn load(project_root: PathBuf) -> anyhow::Result<Self> {
        let snapshot = ConfigLoader::load_snapshot(project_root.clone())?;
        let permission_store = PermissionStore::bootstrap(project_root.clone())?;
        let token_store = TokenStore::bootstrap(project_root.clone())?;
        let execution_store = ExecutionStore::bootstrap(project_root.clone())?;
        let memory_store = MemoryStore::bootstrap(project_root.clone())?;
        Ok(Self {
            project_root,
            snapshot: Arc::new(RwLock::new(snapshot)),
            permission_store,
            token_store,
            execution_store,
            memory_store,
        })
    }

    pub fn memory_store(&self) -> &MemoryStore {
        &self.memory_store
    }

    pub fn token_store(&self) -> &TokenStore {
        &self.token_store
    }

    pub fn execution_store(&self) -> &ExecutionStore {
        &self.execution_store
    }

    pub(crate) fn permission_store(&self) -> &PermissionStore {
        &self.permission_store
    }

    pub(crate) fn project_root(&self) -> &PathBuf {
        &self.project_root
    }

    pub fn config_view(&self) -> ConfigView {
        let snapshot = self.snapshot();
        let current_model_name = snapshot.current_model_name();
        let current_model = snapshot.current_model();
        let provider = current_model
            .map(|m| ProviderDisplay::from_provider_id(&m.provider))
            .unwrap_or(ProviderDisplay::SiliconFlow);
        let has_api_key = current_model
            .and_then(|m| self.resolve_api_key(&m.provider))
            .is_some();

        ConfigView {
            api_url: current_model
                .and_then(|m| m.api_url_override.clone())
                .unwrap_or_else(|| self.default_api_url(current_model.map(|m| m.provider.as_str()))),
            current_model: current_model_name.to_string(),
            effective_model_id: current_model
                .map(|m| m.id.clone())
                .unwrap_or_default(),
            effective_api_url: current_model
                .and_then(|m| m.api_url_override.clone())
                .unwrap_or_else(|| self.default_api_url(current_model.map(|m| m.provider.as_str()))),
            effective_provider: provider.label().to_string(),
            has_api_key,
        }
    }

    pub fn models_view(&self) -> BTreeMap<String, ModelSpec> {
        self.snapshot().models
    }

    pub fn runtime_settings(&self) -> RuntimeSettings {
        self.snapshot().runtime.clone()
    }

    pub fn doctor_report(&self) -> DoctorReport {
        let snapshot = self.snapshot();
        let mut findings = Vec::new();

        if snapshot.models.is_empty() {
            findings.push(DoctorFinding::critical(
                "no_models",
                "未检测到任何模型配置",
                "请先在 models.json 中添加至少一个可用模型",
            ));
        }

        for (name, model) in &snapshot.models {
            if model.id.trim().is_empty() {
                findings.push(DoctorFinding::warn(
                    "empty_model_id",
                    format!("模型 {name} 的 id 为空"),
                    "请修正 models.json 中该模型的 id 字段",
                ));
            }
        }

        DoctorReport {
            ok: !findings.iter().any(|f| f.severity == "critical"),
            findings,
            runtime: snapshot.runtime.clone(),
        }
    }

    pub fn snapshot(&self) -> RuntimeSnapshot {
        self.snapshot
            .read()
            .expect("配置快照读锁已中毒")
            .clone()
    }

    pub fn resolve_api_key(&self, provider_id: &str) -> Option<String> {
        let snapshot = self.snapshot();
        let provider = snapshot.providers.get(provider_id)?;
        provider
            .required_env_keys
            .iter()
            .find_map(|key| snapshot.env.get(key).cloned().filter(|v| !v.trim().is_empty()))
    }

    pub fn default_api_url(&self, provider_id: Option<&str>) -> String {
        if let Some(pid) = provider_id {
            let guard = self.snapshot.read().expect("配置快照读锁已中毒");
            if let Some(provider) = guard.providers.get(pid) {
                return provider.default_api_url.clone();
            }
        }
        ProviderDisplay::SiliconFlow.default_api_url().to_string()
    }

    pub fn reload(&self) -> anyhow::Result<()> {
        let snapshot = ConfigLoader::load_snapshot(self.project_root.clone())?;
        let mut guard = self.snapshot.write().expect("配置快照写锁已中毒");
        *guard = snapshot;
        Ok(())
    }

    pub(crate) fn read_json_file(&self, rel_path: &str, default: Value) -> anyhow::Result<Value> {
        let path = self.project_root.join(rel_path);
        if !path.exists() {
            return Ok(default);
        }
        let text = fs::read_to_string(path)?;
        let value = serde_json::from_str(&text)?;
        Ok(value)
    }

    pub(crate) fn write_json_file(&self, rel_path: &str, value: &Value) -> anyhow::Result<()> {
        let path = self.project_root.join(rel_path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, serde_json::to_string_pretty(value)?)?;
        Ok(())
    }

    pub(crate) fn read_env_file(&self) -> anyhow::Result<BTreeMap<String, String>> {
        let path = self.project_root.join("siliconflow/config/.env");
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

    pub(crate) fn write_env_file(&self, env: &BTreeMap<String, String>) -> anyhow::Result<()> {
        let path = self.project_root.join("siliconflow/config/.env");
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let text = env
            .iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect::<Vec<_>>()
            .join("\n");
        fs::write(path, format!("{text}\n"))?;
        Ok(())
    }
}
