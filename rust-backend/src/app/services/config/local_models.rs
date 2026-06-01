use crate::app::services::config_service::ConfigService;
use anyhow::anyhow;
use reqwest::StatusCode;
use serde_json::{json, Value};
use tokio::process::Command;

impl ConfigService {
    pub async fn local_model_status(&self) -> anyhow::Result<Value> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .build()?;
        let tags_result = client.get("http://127.0.0.1:11434/api/tags").send().await;
        let ps_result = client.get("http://127.0.0.1:11434/api/ps").send().await;

        match tags_result {
            Ok(resp) => {
                let status = resp.status();
                let body: Value = resp.json().await.unwrap_or_else(|_| json!({}));
                let installed = body
                    .get("models")
                    .and_then(|v| v.as_array())
                    .cloned()
                    .unwrap_or_default();
                let loaded = match ps_result {
                    Ok(ps) if ps.status() == StatusCode::OK => ps
                        .json::<Value>()
                        .await
                        .ok()
                        .and_then(|v| v.get("models").and_then(|m| m.as_array()).cloned())
                        .unwrap_or_default(),
                    _ => Vec::new(),
                };
                Ok(json!({
                    "service_running": status == StatusCode::OK,
                    "status": status.as_u16(),
                    "installed_models": installed,
                    "loaded_models": loaded
                }))
            }
            Err(err) => Ok(json!({
                "service_running": false,
                "status": null,
                "error": err.to_string(),
                "installed_models": [],
                "loaded_models": []
            })),
        }
    }

    pub async fn local_model_pull(&self, model: &str) -> anyhow::Result<Value> {
        let output = Command::new("ollama")
            .arg("pull")
            .arg(model)
            .output()
            .await?;
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        Ok(json!({
            "ok": output.status.success(),
            "stdout": stdout,
            "stderr": stderr
        }))
    }

    pub async fn local_model_load(&self, model: &str) -> anyhow::Result<Value> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()?;
        let resp = client
            .post("http://127.0.0.1:11434/api/generate")
            .json(&json!({
                "model": model,
                "prompt": "ping",
                "stream": false,
                "keep_alive": "30m"
            }))
            .send()
            .await?;
        let status = resp.status().as_u16();
        let body: Value = resp.json().await.unwrap_or_else(|_| json!({}));
        Ok(json!({
            "ok": (200..300).contains(&status),
            "status": status,
            "body": body
        }))
    }

    pub async fn local_model_unload(&self, model: &str) -> anyhow::Result<Value> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            .build()?;
        let resp = client
            .post("http://127.0.0.1:11434/api/generate")
            .json(&json!({
                "model": model,
                "prompt": "",
                "stream": false,
                "keep_alive": 0
            }))
            .send()
            .await?;
        let status = resp.status().as_u16();
        let body: Value = resp.json().await.unwrap_or_else(|_| json!({}));
        Ok(json!({
            "ok": (200..300).contains(&status),
            "status": status,
            "body": body
        }))
    }

    pub async fn local_model_service_control(&self, action: &str) -> anyhow::Result<Value> {
        match action {
            "start" => {
                Ok(json!({
                    "ok": Command::new("ollama")
                        .arg("serve")
                        .spawn()
                        .is_ok()
                }))
            }
            "stop" => {
                let output = Command::new("pkill")
                    .arg("ollama")
                    .output()
                    .await?;
                Ok(json!({
                    "ok": output.status.success(),
                    "stdout": String::from_utf8_lossy(&output.stdout).to_string(),
                    "stderr": String::from_utf8_lossy(&output.stderr).to_string()
                }))
            }
            _ => Err(anyhow!("invalid_action")),
        }
    }
}
