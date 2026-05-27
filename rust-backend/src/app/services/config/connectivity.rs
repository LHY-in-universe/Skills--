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
            let result = client
                .post(&api_url)
                .bearer_auth(api_key)
                .json(&payload)
                .send()
                .await;

            match result {
                Ok(resp) => {
                    let status = resp.status();
                    let body = resp.text().await.unwrap_or_default();
                    items.push(json!({
                        "model_name": name,
                        "model_id": spec.id,
                        "provider": spec.provider,
                        "api_url": api_url,
                        "ok": status.is_success(),
                        "status": status.as_u16(),
                        "error": if status.is_success() { Value::Null } else { Value::String(body.chars().take(240).collect()) }
                    }));
                }
                Err(err) => {
                    items.push(json!({
                        "model_name": name,
                        "model_id": spec.id,
                        "provider": spec.provider,
                        "api_url": api_url,
                        "ok": false,
                        "error": err.to_string()
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
