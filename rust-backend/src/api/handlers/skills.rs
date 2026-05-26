use crate::api::handlers::shared::{
    internal_error, map_skill_error, SkillCatalogQuery, SkillInstallRequest, SkillToggleRequest,
    SkillUninstallRequest,
};
use crate::app::state::AppState;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;

pub async fn get_skills(
    State(state): State<AppState>,
) -> Result<Json<Vec<serde_json::Value>>, (StatusCode, Json<serde_json::Value>)> {
    state
        .config_service
        .skills_view()
        .map(Json)
        .map_err(internal_error)
}

pub async fn get_skills_runtime(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    Ok(Json(state.clawhub_service.runtime_info().await))
}

pub async fn get_skills_catalog(
    State(state): State<AppState>,
    Query(query): Query<SkillCatalogQuery>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    state
        .clawhub_service
        .catalog(query.limit, query.sort.as_deref(), query.query.as_deref())
        .await
        .map(Json)
        .map_err(map_skill_error)
}

pub async fn install_skill(
    State(state): State<AppState>,
    Json(req): Json<SkillInstallRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    state
        .clawhub_service
        .install_skill(&req.slug)
        .await
        .map(Json)
        .map_err(map_skill_error)
}

pub async fn uninstall_skill(
    State(state): State<AppState>,
    Json(req): Json<SkillUninstallRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    state
        .clawhub_service
        .uninstall_skill(&req.slug)
        .await
        .map(Json)
        .map_err(map_skill_error)
}

pub async fn rescan_skills(
    State(state): State<AppState>,
) -> Result<Json<Vec<serde_json::Value>>, (StatusCode, Json<serde_json::Value>)> {
    state
        .config_service
        .scan_and_sync_skills()
        .and_then(|_| state.config_service.skills_view())
        .map(Json)
        .map_err(map_skill_error)
}

pub async fn toggle_skill(
    State(state): State<AppState>,
    Json(req): Json<SkillToggleRequest>,
) -> Result<Json<Vec<serde_json::Value>>, (StatusCode, Json<serde_json::Value>)> {
    state
        .config_service
        .toggle_skill(&req.name, req.enabled)
        .map(Json)
        .map_err(map_skill_error)
}

pub async fn update_skill(
    State(state): State<AppState>,
    Path(name): Path<String>,
    Json(req): Json<serde_json::Value>,
) -> Result<Json<Vec<serde_json::Value>>, (StatusCode, Json<serde_json::Value>)> {
    state
        .config_service
        .update_skill_config(&name, &req)
        .map(Json)
        .map_err(internal_error)
}

pub async fn get_token_usage(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    state
        .config_service
        .token_usage()
        .map(Json)
        .map_err(internal_error)
}

pub async fn get_routing(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    state
        .config_service
        .routing_config()
        .map(Json)
        .map_err(internal_error)
}

pub async fn save_routing(
    State(state): State<AppState>,
    Json(req): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    state
        .config_service
        .save_routing_config(&req)
        .map(Json)
        .map_err(internal_error)
}
