use crate::api::handlers::shared::{internal_error, LimitQuery};
use crate::app::state::AppState;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::Json;

pub async fn get_failover_recent(
    State(state): State<AppState>,
    Query(query): Query<LimitQuery>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    Ok(Json(serde_json::json!({
        "items": state.chat_service.recent_failovers(query.limit.unwrap_or(8))
    })))
}

pub async fn get_observability_summary(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    state
        .config_service
        .observability_summary()
        .map(Json)
        .map_err(internal_error)
}

pub async fn get_observability_events(
    State(state): State<AppState>,
    Query(query): Query<LimitQuery>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    state
        .config_service
        .observability_events(query.limit.unwrap_or(20))
        .map(Json)
        .map_err(internal_error)
}
