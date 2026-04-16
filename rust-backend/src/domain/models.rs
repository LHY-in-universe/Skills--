use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

/// 模型能力描述。
///
/// 这是从当前 Python `models.json` 的松散对象中抽出来的强类型结构。
/// 后续如果 provider 或前端需要区分 chat / tools / vision 能力，都会基于这里判断。
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ModelCapabilities {
    pub chat: bool,
    pub vision: bool,
    pub tools: bool,
}

/// 单个模型配置。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelSpec {
    pub id: String,
    pub provider: String,
    /// 可选的模型级 API URL 覆盖。
    ///
    /// 这里序列化时保持为 `api_url`，以兼容现有前端和 Python 配置文件。
    #[serde(rename = "api_url", alias = "api_url_override", default)]
    pub api_url_override: Option<String>,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub capabilities: ModelCapabilities,
    #[serde(default)]
    pub requires: Vec<String>,
}

/// Provider 配置。
///
/// 第一阶段只承载聊天调用必需字段；后续再扩展 auth profile、error mapping、
/// tool capability、secret ref 等细节。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderSpec {
    pub id: String,
    pub label: String,
    pub default_api_url: String,
    #[serde(default)]
    pub required_env_keys: Vec<String>,
}

fn default_true() -> bool {
    true
}

/// 运行时策略。
///
/// 该结构用于承接当前 `runtime_config.json`，后续 planner / retry / self-correction
/// 的所有策略都会正式挂到这里，而不是散落在 orchestrator 长函数里。
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RuntimeSettings {
    #[serde(default)]
    pub embedding_policy: String,
    #[serde(default)]
    pub fast_first_token: bool,
    #[serde(default)]
    pub self_correction_enabled: bool,
    #[serde(default)]
    pub self_correction_max_retries: i32,
    #[serde(default)]
    pub loop_guard_enabled: bool,
    #[serde(default)]
    pub planner_enabled: bool,
    #[serde(default)]
    pub planner_max_steps: i32,
    #[serde(default)]
    pub vision_models: serde_json::Value,
    #[serde(default)]
    pub retry_policy: serde_json::Value,
}

/// 运行时完整快照。
///
/// 第一阶段它只是“配置加载结果”，后续会扩成：
/// - 激活 provider
/// - secret 解析状态
/// - session runtime registry
/// - SQLite 连接元数据
#[derive(Debug, Clone)]
pub struct RuntimeSnapshot {
    pub project_root: PathBuf,
    pub models: BTreeMap<String, ModelSpec>,
    pub providers: BTreeMap<String, ProviderSpec>,
    pub env: BTreeMap<String, String>,
    pub runtime: RuntimeSettings,
}

impl RuntimeSnapshot {
    pub fn current_model_name(&self) -> &str {
        self.models
            .iter()
            .find(|(_, spec)| spec.enabled)
            .map(|(name, _)| name.as_str())
            .or_else(|| self.models.keys().next().map(|s| s.as_str()))
            .unwrap_or("")
    }

    pub fn current_model(&self) -> Option<&ModelSpec> {
        self.models
            .iter()
            .find(|(_, spec)| spec.enabled)
            .map(|(_, spec)| spec)
            .or_else(|| self.models.values().next())
    }
}

/// `/api/config` 返回结构。
#[derive(Debug, Clone, Serialize)]
pub struct ConfigView {
    pub api_url: String,
    pub current_model: String,
    pub effective_model_id: String,
    pub effective_api_url: String,
    pub effective_provider: String,
    pub has_api_key: bool,
}

/// `/api/config` 的更新请求。
#[derive(Debug, Clone, Deserialize)]
pub struct ConfigUpdateRequest {
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub api_url: Option<String>,
    #[serde(default)]
    pub api_key: Option<String>,
}

/// 新增模型请求。
#[derive(Debug, Clone, Deserialize)]
pub struct ModelCreateRequest {
    pub name: String,
    pub model_id: String,
    #[serde(default)]
    pub provider: Option<String>,
    #[serde(default)]
    pub api_url: Option<String>,
}

/// 更新模型请求。
#[derive(Debug, Clone, Deserialize)]
pub struct ModelUpdateRequest {
    #[serde(default)]
    pub model_id: Option<String>,
    #[serde(default)]
    pub provider: Option<String>,
    #[serde(default)]
    pub api_url: Option<String>,
}

/// 前端展示时使用的 provider 标签。
#[derive(Debug, Clone, Copy)]
pub enum ProviderDisplay {
    SiliconFlow,
    DeepSeek,
    Kimi,
    Nvidia,
    Unknown,
}

impl ProviderDisplay {
    pub fn from_provider_id(provider: &str) -> Self {
        match provider {
            "siliconflow" => Self::SiliconFlow,
            "deepseek" => Self::DeepSeek,
            "kimi" | "kimi-coding" => Self::Kimi,
            "nvidia" | "nim" | "nvidia-nim" => Self::Nvidia,
            _ => Self::Unknown,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::SiliconFlow => "SiliconFlow",
            Self::DeepSeek => "DeepSeek",
            Self::Kimi => "KimiCoding",
            Self::Nvidia => "NVIDIA",
            Self::Unknown => "Unknown",
        }
    }

    pub fn default_api_url(&self) -> &'static str {
        match self {
            Self::SiliconFlow => "https://api.siliconflow.cn/v1/chat/completions",
            Self::DeepSeek => "https://api.deepseek.com/v1/chat/completions",
            Self::Kimi => "https://api.kimi.com/coding/v1/chat/completions",
            Self::Nvidia => "https://integrate.api.nvidia.com/v1/chat/completions",
            Self::Unknown => "",
        }
    }
}
