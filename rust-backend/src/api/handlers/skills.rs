use crate::api::handlers::shared::{internal_error, SkillToggleRequest};
use crate::app::state::AppState;
use axum::extract::{Path, State};
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

pub async fn toggle_skill(
    State(state): State<AppState>,
    Json(req): Json<SkillToggleRequest>,
) -> Result<Json<Vec<serde_json::Value>>, (StatusCode, Json<serde_json::Value>)> {
    state
        .config_service
        .toggle_skill(&req.name, req.enabled)
        .map(Json)
        .map_err(internal_error)
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
