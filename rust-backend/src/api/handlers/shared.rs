//! handler 层共享的请求 DTO 与工具函数。

use axum::http::StatusCode;
use axum::response::sse::Event;
use axum::Json;
use serde::Deserialize;
use serde_json::Value;

// ===== 请求 / 响应 DTO =====

#[derive(Debug, Deserialize)]
pub struct SkillToggleRequest {
    pub name: String,
    pub enabled: bool,
}

#[derive(Debug, Deserialize)]
pub struct LimitQuery {
    pub limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
pub struct DoctorFixRequest {
    #[serde(default)]
    pub dry_run: bool,
}

#[derive(Debug, Deserialize)]
pub struct TerminalCwdRequest {
    pub cwd: String,
}

#[derive(Debug, Deserialize)]
pub struct LarkConfigRequest {
    pub app_id: String,
    pub app_secret: String,
}

#[derive(Debug, Deserialize)]
pub struct HistoryQuery {
    pub conv_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ChatRequest {
    pub user_input: String,
    pub conv_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct AbortChatRequest {
    pub conv_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct PermissionResumeRequest {
    pub granted: bool,
    #[serde(default)]
    pub always_allow: bool,
    pub conv_id: Option<String>,
}

// ===== 错误与响应辅助 =====

pub fn internal_error(err: anyhow::Error) -> (StatusCode, Json<Value>) {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(serde_json::json!({
            "code": "internal_error",
            "message": err.to_string()
        })),
    )
}

pub fn sse_json(value: Value) -> Event {
    Event::default().data(value.to_string())
}

pub fn map_conversation_error(err: anyhow::Error) -> (StatusCode, Json<Value>) {
    if err.to_string().contains("conversation_not_found") {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({
                "code": "conversation_not_found",
                "message": "会话不存在"
            })),
        );
    }
    internal_error(err)
}

pub fn map_config_error(err: anyhow::Error) -> (StatusCode, Json<Value>) {
    if err.to_string().contains("model_already_exists") {
        return (
            StatusCode::CONFLICT,
            Json(serde_json::json!({
                "code": "model_already_exists",
                "message": "模型已存在"
            })),
        );
    }
    if err.to_string().contains("model_not_found") {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({
                "code": "model_not_found",
                "message": "模型不存在"
            })),
        );
    }
    if err.to_string().contains("invalid_cwd") {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "code": "invalid_cwd",
                "message": "目录无效"
            })),
        );
    }
    internal_error(err)
}
