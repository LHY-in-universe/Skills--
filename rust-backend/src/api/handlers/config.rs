use crate::api::handlers::shared::{
    internal_error, map_config_error, LarkConfigRequest, TerminalCwdRequest,
};
use crate::app::state::AppState;
use crate::domain::models::{
    ConfigUpdateRequest, ConfigView, ModelCreateRequest, ModelSpec, ModelUpdateRequest,
    RuntimeSettings,
};
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use std::collections::BTreeMap;

/// 返回当前基础配置视图。
///
/// 这里先对齐现有前端最核心的读取行为：当前模型、有效 provider、基础 URL。
pub async fn get_config(State(state): State<AppState>) -> Json<ConfigView> {
    Json(state.config_service.config_view())
}

/// 更新当前激活模型。
///
/// 这个接口先只支持"切换当前模型"这一件事，保持和现有前端交互最小兼容。
/// 后续再把 provider、api_url、auth profile 等配置拆到更细粒度的接口。
pub async fn update_config(
    State(state): State<AppState>,
    Json(req): Json<ConfigUpdateRequest>,
) -> Result<Json<ConfigView>, (StatusCode, Json<serde_json::Value>)> {
    if let Some(model) = req.model.as_deref() {
        return state
            .config_service
            .set_current_model(model)
            .map(Json)
            .map_err(map_config_error);
    }
    Ok(Json(state.config_service.config_view()))
}

/// 返回模型清单。
pub async fn get_models(State(state): State<AppState>) -> Json<BTreeMap<String, ModelSpec>> {
    Json(state.config_service.models_view())
}

/// 返回运行时策略。
pub async fn get_runtime_settings(State(state): State<AppState>) -> Json<RuntimeSettings> {
    Json(state.config_service.runtime_settings())
}

pub async fn save_runtime_settings(
    State(state): State<AppState>,
    Json(req): Json<serde_json::Value>,
) -> Result<Json<RuntimeSettings>, (StatusCode, Json<serde_json::Value>)> {
    state
        .config_service
        .save_runtime_settings(&req)
        .map(Json)
        .map_err(internal_error)
}

pub async fn get_providers_catalog(State(state): State<AppState>) -> Json<Vec<serde_json::Value>> {
    Json(state.config_service.providers_catalog())
}

pub async fn get_security_audit(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    state
        .config_service
        .security_audit()
        .map(Json)
        .map_err(internal_error)
}

pub async fn get_auth_profiles(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    state
        .config_service
        .auth_profiles()
        .map(Json)
        .map_err(internal_error)
}

pub async fn get_runtime_health(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let mut health = state
        .config_service
        .runtime_health()
        .map_err(internal_error)?;
    let active_runs: Vec<serde_json::Value> = state
        .chat_service
        .active_runs()
        .into_iter()
        .map(|(conv_id, status)| {
            serde_json::json!({
                "conv_id": conv_id,
                "status": status.as_str()
            })
        })
        .collect();
    if let Some(obj) = health.as_object_mut() {
        obj.insert(
            "active_runs".to_string(),
            serde_json::Value::Array(active_runs),
        );
    }
    Ok(Json(health))
}

pub async fn get_lark_config(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    state
        .config_service
        .lark_config()
        .map(Json)
        .map_err(internal_error)
}

pub async fn save_lark_config(
    State(state): State<AppState>,
    Json(req): Json<LarkConfigRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    state
        .config_service
        .save_lark_config(&req.app_id, &req.app_secret)
        .map(Json)
        .map_err(internal_error)
}

pub async fn get_terminal_cwd(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    state
        .config_service
        .terminal_cwd()
        .map(Json)
        .map_err(internal_error)
}

pub async fn save_terminal_cwd(
    State(state): State<AppState>,
    Json(req): Json<TerminalCwdRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    state
        .config_service
        .save_terminal_cwd(&req.cwd)
        .map(Json)
        .map_err(map_config_error)
}

pub async fn create_model(
    State(state): State<AppState>,
    Json(req): Json<ModelCreateRequest>,
) -> Result<Json<BTreeMap<String, ModelSpec>>, (StatusCode, Json<serde_json::Value>)> {
    state
        .config_service
        .add_model(&req)
        .map(Json)
        .map_err(map_config_error)
}

pub async fn update_model(
    State(state): State<AppState>,
    Path(name): Path<String>,
    Json(req): Json<ModelUpdateRequest>,
) -> Result<Json<BTreeMap<String, ModelSpec>>, (StatusCode, Json<serde_json::Value>)> {
    state
        .config_service
        .update_model(&name, &req)
        .map(Json)
        .map_err(map_config_error)
}

pub async fn delete_model(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Result<Json<BTreeMap<String, ModelSpec>>, (StatusCode, Json<serde_json::Value>)> {
    state
        .config_service
        .delete_model(&name)
        .map(Json)
        .map_err(map_config_error)
}
