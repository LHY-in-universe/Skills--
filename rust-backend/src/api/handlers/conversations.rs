use crate::api::handlers::shared::{internal_error, map_conversation_error, HistoryQuery};
use crate::app::state::AppState;
use crate::domain::conversation::{
    ConversationActionResponse, ConversationCreateResponse, ConversationHistoryResponse,
    ConversationRenameRequest, ConversationSummary,
};
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;

pub async fn list_conversations(
    State(state): State<AppState>,
) -> Result<Json<Vec<ConversationSummary>>, (StatusCode, Json<serde_json::Value>)> {
    state
        .conversation_service
        .list()
        .map(Json)
        .map_err(internal_error)
}

pub async fn create_conversation(
    State(state): State<AppState>,
) -> Result<Json<ConversationCreateResponse>, (StatusCode, Json<serde_json::Value>)> {
    state
        .conversation_service
        .create()
        .map(Json)
        .map_err(internal_error)
}

pub async fn activate_conversation(
    State(state): State<AppState>,
    Path(conv_id): Path<String>,
) -> Result<Json<ConversationActionResponse>, (StatusCode, Json<serde_json::Value>)> {
    state
        .conversation_service
        .activate(&conv_id)
        .map_err(map_conversation_error)?;
    Ok(Json(ConversationActionResponse {
        status: "ok",
        active_id: Some(conv_id),
    }))
}

pub async fn rename_conversation(
    State(state): State<AppState>,
    Path(conv_id): Path<String>,
    Json(req): Json<ConversationRenameRequest>,
) -> Result<Json<ConversationActionResponse>, (StatusCode, Json<serde_json::Value>)> {
    state
        .conversation_service
        .rename(&conv_id, &req.name)
        .map_err(map_conversation_error)?;
    Ok(Json(ConversationActionResponse {
        status: "ok",
        active_id: None,
    }))
}

pub async fn delete_conversation(
    State(state): State<AppState>,
    Path(conv_id): Path<String>,
) -> Result<Json<Vec<ConversationSummary>>, (StatusCode, Json<serde_json::Value>)> {
    state
        .conversation_service
        .delete(&conv_id)
        .map(Json)
        .map_err(map_conversation_error)
}

pub async fn get_history(
    State(state): State<AppState>,
    Query(query): Query<HistoryQuery>,
) -> Result<
    Json<Vec<crate::domain::conversation::ConversationMessage>>,
    (StatusCode, Json<serde_json::Value>),
> {
    let history: ConversationHistoryResponse = state
        .conversation_service
        .history(query.conv_id.as_deref())
        .map_err(internal_error)?;
    Ok(Json(history.items))
}

pub async fn clear_history(
    State(state): State<AppState>,
    Query(query): Query<HistoryQuery>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    state
        .conversation_service
        .clear_history(query.conv_id.as_deref())
        .map_err(internal_error)?;
    Ok(Json(serde_json::json!({ "status": "ok" })))
}
