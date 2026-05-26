use crate::domain::models::RuntimeSnapshot;
use crate::infra::auth_profile::AuthProfileResolver;
use std::collections::BTreeSet;

/// 单个模型在当前运行期可直接请求的解析结果。
#[derive(Debug, Clone)]
pub struct ModelRuntime {
    pub model_name: String,
    pub model_id: String,
    pub provider: String,
    pub api_url: String,
    pub api_key: String,
}

/// 构造 failover 候选链。
///
/// 顺序与 Python 版保持一致：
/// 1. 当前激活模型（如果和已选模型不同）
/// 2. 其余已配置模型
///
/// 会自动跳过需要 API Key 但当前未配置密钥的 provider。
pub fn build_fallback_chain(
    auth_resolver: &dyn AuthProfileResolver,
    snapshot: &RuntimeSnapshot,
    current_selected_name: &str,
    current_default_name: &str,
) -> Vec<ModelRuntime> {
    let mut out = Vec::new();
    let mut seen = BTreeSet::new();
    for name in [current_default_name.to_string()]
        .into_iter()
        .chain(snapshot.models.keys().cloned())
    {
        if name == current_selected_name || seen.contains(&name) {
            continue;
        }
        if let Some(spec) = snapshot.models.get(&name) {
            let requires_key = snapshot
                .providers
                .get(&spec.provider)
                .map(|p| !p.required_env_keys.is_empty())
                .unwrap_or(true);
            let api_key = match auth_resolver.resolve_api_key(&spec.provider) {
                Some(v) => v,
                None if !requires_key => String::new(),
                None => continue,
            };
            let api_url = spec
                .api_url_override
                .clone()
                .unwrap_or_else(|| auth_resolver.default_api_url(Some(&spec.provider)));
            out.push(ModelRuntime {
                model_name: name.clone(),
                model_id: spec.id.clone(),
                provider: spec.provider.clone(),
                api_url,
                api_key,
            });
            seen.insert(name);
        }
    }
    out
}

/// 把上游错误消息粗略归类成 failover 决策使用的语义。
pub fn classify_upstream_error(err_msg: &str) -> &'static str {
    let lower = err_msg.to_lowercase();
    let is_timeout = lower.contains("timeout") || lower.contains("timed out");
    let is_connection = lower.contains("connection error")
        || lower.contains("connection reset")
        || lower.contains("server disconnected")
        || lower.contains("remoteprotocolerror");
    if is_timeout || is_connection {
        return "network_timeout";
    }
    if lower.contains("401") || lower.contains("unauthorized") || lower.contains("invalid api key")
    {
        return "auth_error";
    }
    if lower.contains("403") || lower.contains("forbidden") {
        return "permission_error";
    }
    if lower.contains("429") || lower.contains("rate limit") || lower.contains("too many requests")
    {
        return "rate_limit";
    }
    if lower.contains("400") || lower.contains("bad request") {
        return "bad_request";
    }
    if lower.contains("model does not exist")
        || lower.contains("model not exist")
        || lower.contains("\"code\": 20012")
        || lower.contains("'code': 20012")
    {
        return "model_not_found";
    }
    if lower.contains("500") || lower.contains("502") || lower.contains("503") {
        return "provider_5xx";
    }
    if lower.contains("tool_calls") || lower.contains("role 'tool'") {
        return "protocol_mismatch";
    }
    "upstream_error"
}
