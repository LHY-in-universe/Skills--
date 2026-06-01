//! 模型连通性自检。

use crate::app::services::config_service::ConfigService;
use serde_json::{json, Value};
use std::collections::BTreeSet;

impl ConfigService {
    pub async fn model_connectivity_check(&self) -> anyhow::Result<Value> {
        let snapshot = self.snapshot();
        let routing = self
            .read_json_file("siliconflow/config/routing_config.json", json!({}))
            .unwrap_or_else(|_| json!({}));

        let mut targets = BTreeSet::new();
        let current = snapshot.current_model_name().to_string();
        if !current.is_empty() {
            targets.insert(current);
        }
        for key in ["router_model", "summary_model"] {
            if let Some(name) = routing.get(key).and_then(|v| v.as_str()).map(str::trim) {
                if !name.is_empty() {
                    targets.insert(name.to_string());
                }
            }
        }
        if let Some(tiers) = routing.get("tiers").and_then(|v| v.as_object()) {
            for value in tiers.values() {
                if let Some(name) = value.as_str().map(str::trim) {
                    if !name.is_empty() {
                        targets.insert(name.to_string());
                    }
                }
            }
        }

        let mut items = Vec::new();
        for name in targets {
            let Some(spec) = snapshot.models.get(&name) else {
                items.push(json!({
                    "model_name": name,
                    "ok": false,
                    "error": "model_not_found"
                }));
                continue;
            };

            let api_url = spec
                .api_url_override
                .clone()
                .unwrap_or_else(|| self.default_api_url(Some(&spec.provider)));
            let has_api_key = self.resolve_api_key(&spec.provider).is_some();
            if !has_api_key {
                items.push(json!({
                    "model_name": name,
                    "model_id": spec.id,
                    "provider": spec.provider,
                    "api_url": api_url,
                    "ok": false,
                    "error": "missing_api_key"
                }));
                continue;
            }

            let api_key = self.resolve_api_key(&spec.provider).unwrap_or_default();
            let payload = json!({
                "model": spec.id,
                "messages": [
                    { "role": "user", "content": "ping" }
                ],
                "stream": false,
                "max_tokens": 8
            });

            let client = reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(20))
                .build()?;
            let result = self
                .apply_provider_auth(client.post(&api_url), &spec.provider, &api_key)
                .json(&payload)
                .send()
                .await;

            match result {
                Ok(resp) => {
                    let status = resp.status();
                    let body = resp.text().await.unwrap_or_default();
                    let (diagnosis, recommendation) =
                        classify_connectivity_failure(&spec.provider, status.as_u16(), &body);
                    items.push(json!({
                        "model_name": name,
                        "model_id": spec.id,
                        "provider": spec.provider,
                        "api_url": api_url,
                        "ok": status.is_success(),
                        "status": status.as_u16(),
                        "error": if status.is_success() { Value::Null } else { Value::String(body.chars().take(240).collect()) },
                        "diagnosis": diagnosis,
                        "recommendation": recommendation
                    }));
                }
                Err(err) => {
                    let message = err.to_string();
                    items.push(json!({
                        "model_name": name,
                        "model_id": spec.id,
                        "provider": spec.provider,
                        "api_url": api_url,
                        "ok": false,
                        "error": message,
                        "diagnosis": "network_or_transport_error",
                        "recommendation": "检查本机网络、代理设置、TLS 连接和 provider 域名是否可直连"
                    }));
                }
            }
        }

        let ok = items
            .iter()
            .all(|item| item.get("ok").and_then(|v| v.as_bool()) == Some(true));

        Ok(json!({
            "ok": ok,
            "items": items
        }))
    }
}

fn classify_connectivity_failure(
    provider: &str,
    status: u16,
    body: &str,
) -> (&'static str, &'static str) {
    let lowered = body.to_lowercase();
    if status == 401 {
        if provider == "mimo" && lowered.contains("invalid api key") {
            return (
                "invalid_mimo_api_key",
                "当前 MIMO_API_KEY 对 MiMo 官方接口无效；请在 siliconflow/config/.env 中更换有效的 MiMo 平台 key 后重试",
            );
        }
        if provider == "siliconflow" && lowered.contains("api key is invalid") {
            return (
                "invalid_provider_api_key",
                "当前 SILICONFLOW_API_KEY 对官方接口无效；请更换有效 key 后重试",
            );
        }
        if provider == "minimax" && lowered.contains("authorized_error") {
            return (
                "invalid_or_unsupported_minimax_key",
                "当前 MiniMax key 无法通过官方 chat/completions 鉴权；请确认 key 类型、账号权限或重新生成正式 API key",
            );
        }
        return (
            "unauthorized",
            "鉴权失败；请检查 provider 对应 API key、账号权限和模型访问范围",
        );
    }
    if status == 403 {
        return (
            "forbidden_or_no_model_access",
            "账号已通过鉴权但没有该模型访问权限；请检查模型开通状态和套餐权限",
        );
    }
    if status == 404 {
        return (
            "endpoint_or_model_not_found",
            "接口地址或模型名无效；请检查 provider endpoint 与 model id 是否为官方正式值",
        );
    }
    if status >= 500 {
        return (
            "provider_server_error",
            "provider 侧服务异常；稍后重试，必要时切换回退模型",
        );
    }
    (
        "unknown_failure",
        "请结合 error 字段原文进一步排查 provider 鉴权、模型名或网络链路",
    )
}
