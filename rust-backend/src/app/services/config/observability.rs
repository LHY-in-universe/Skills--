//! 可观测面 / 失败回退 / token usage / runtime health。
//!
//! 这些方法对应 `/api/runtime-health`、`/api/observability/*`、`/api/token-usage`、
//! `/api/failover/recent`，前端要的字段 shape 在这里完整保留。

use crate::app::services::config_service::ConfigService;
use serde_json::{Map, Value};
use std::fs;

impl ConfigService {
    pub fn token_usage(&self) -> anyhow::Result<Value> {
        let mut raw = self.read_json_file(
            "siliconflow/data/token_usage.json",
            serde_json::json!({
                "version": 2,
                "global": {
                    "calls": 0,
                    "prompt": 0,
                    "completion": 0,
                    "total": 0,
                    "errors": { "count": 0, "by_class": {} },
                    "failover": { "count": 0, "by_type": {}, "success": 0, "exhausted": 0 },
                    "latency": { "count": 0, "sum_ms": 0, "avg_ms": 0, "p95_ms_est": 0, "max_ms": 0 },
                    "by_type": {}
                },
                "daily": {}
            }),
        )?;
        if let Ok(totals) = self.token_store().aggregate_total() {
            if let Some(global) = raw.get_mut("global").and_then(|v| v.as_object_mut()) {
                global.insert("calls".into(), Value::from(totals.calls));
                global.insert("prompt".into(), Value::from(totals.prompt));
                global.insert("completion".into(), Value::from(totals.completion));
                global.insert("total".into(), Value::from(totals.total));
            }
        }
        if let Ok(by_model) = self.token_store().aggregate_by_model() {
            let mut by_type = Map::new();
            for (model, totals) in by_model {
                by_type.insert(
                    model,
                    serde_json::json!({
                        "calls": totals.calls,
                        "prompt": totals.prompt,
                        "completion": totals.completion,
                        "total": totals.total,
                    }),
                );
            }
            if let Some(global) = raw.get_mut("global").and_then(|v| v.as_object_mut()) {
                global.insert("by_type".into(), Value::Object(by_type));
            }
        }
        Ok(raw)
    }

    pub fn runtime_health(&self) -> anyhow::Result<Value> {
        let skills = self.skills_view()?;
        let active_conversation_id = self
            .read_json_file("siliconflow/data/conversations.json", serde_json::json!({}))
            .ok()
            .and_then(|v| v.get("active_id").cloned())
            .unwrap_or(Value::Null);
        Ok(serde_json::json!({
            "active_conversation_id": active_conversation_id,
            "models_count": self.snapshot().models.len(),
            "enabled_skills": skills
                .iter()
                .filter(|v| v.get("enabled").and_then(|x| x.as_bool()).unwrap_or(false))
                .count(),
        }))
    }

    pub fn security_audit(&self) -> anyhow::Result<Value> {
        let snapshot = self.snapshot();
        let mut findings = Vec::new();
        for provider in snapshot.providers.values() {
            let has_key = provider.required_env_keys.iter().any(|key| {
                snapshot
                    .env
                    .get(key)
                    .map(|v| !v.trim().is_empty())
                    .unwrap_or(false)
            });
            if !has_key {
                findings.push(serde_json::json!({
                    "severity": "warn",
                    "message": format!("provider {} 缺少 API Key", provider.label),
                    "code": "missing_api_key"
                }));
            }
        }
        Ok(serde_json::json!({
            "ok": findings.is_empty(),
            "findings": findings
        }))
    }

    pub fn failover_recent(&self, limit: usize) -> anyhow::Result<Value> {
        let token_usage = self.token_usage()?;
        let fail_count = token_usage
            .get("global")
            .and_then(|v| v.get("failover"))
            .and_then(|v| v.get("count"))
            .and_then(|v| v.as_i64())
            .unwrap_or(0);
        let mut items = Vec::new();
        if fail_count > 0 {
            items.push(serde_json::json!({
                "from_model": "",
                "to_model": "",
                "failover_type": "historical_aggregate",
                "message": format!("历史累计 failover {} 次，明细尚未迁移到 Rust", fail_count),
            }));
        }
        if items.len() > limit {
            items.truncate(limit);
        }
        Ok(serde_json::json!({ "items": items }))
    }

    pub fn observability_summary(&self) -> anyhow::Result<Value> {
        let token_usage = self.token_usage()?;
        let errors_count = token_usage
            .get("global")
            .and_then(|v| v.get("errors"))
            .and_then(|v| v.get("count"))
            .and_then(|v| v.as_i64())
            .unwrap_or(0);
        let failover = token_usage
            .get("global")
            .and_then(|v| v.get("failover"))
            .cloned()
            .unwrap_or_else(|| serde_json::json!({}));
        let success = failover.get("success").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let exhausted = failover.get("exhausted").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let total = success + exhausted;
        let success_rate = if total > 0.0 {
            (success / total * 100.0).round() as i64
        } else {
            0
        };
        let events = self.observability_events(200)?;
        let items = events
            .get("items")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        let today_events = items.len();
        let today_errors = items
            .iter()
            .filter(|v| v.get("level").and_then(|x| x.as_str()) == Some("error"))
            .count();
        Ok(serde_json::json!({
            "errors": { "count": errors_count },
            "failover": { "success_rate": success_rate },
            "execution_logs": {
                "today_events": today_events,
                "today_errors": today_errors
            }
        }))
    }

    pub fn observability_events(&self, limit: usize) -> anyhow::Result<Value> {
        let mut items = self.execution_store().recent(limit).unwrap_or_default();
        if items.len() >= limit {
            return Ok(serde_json::json!({ "items": items }));
        }
        let remaining = limit - items.len();
        let logs_dir = self.project_root().join("siliconflow/data/logs");
        if logs_dir.exists() {
            let mut legacy = Vec::new();
            let mut files = fs::read_dir(&logs_dir)?
                .filter_map(|entry| entry.ok().map(|e| e.path()))
                .filter(|path| path.extension().and_then(|s| s.to_str()) == Some("jsonl"))
                .collect::<Vec<_>>();
            files.sort();
            files.reverse();
            for file in files {
                let text = fs::read_to_string(&file).unwrap_or_default();
                for line in text.lines().rev() {
                    if line.trim().is_empty() {
                        continue;
                    }
                    if let Ok(value) = serde_json::from_str::<Value>(line) {
                        legacy.push(value);
                        if legacy.len() >= remaining {
                            break;
                        }
                    }
                }
                if legacy.len() >= remaining {
                    break;
                }
            }
            items.extend(legacy);
        }
        Ok(serde_json::json!({ "items": items }))
    }
}
