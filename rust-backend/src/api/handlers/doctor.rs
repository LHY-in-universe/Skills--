use crate::api::handlers::shared::{internal_error, DoctorFixRequest};
use crate::app::state::AppState;
use crate::domain::doctor::DoctorReport;
use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;

/// 返回 doctor 基础报告。
pub async fn get_doctor(State(state): State<AppState>) -> Json<DoctorReport> {
    Json(state.config_service.doctor_report())
}

pub async fn doctor_fix(
    State(state): State<AppState>,
    Json(req): Json<DoctorFixRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    state
        .config_service
        .doctor_fix(req.dry_run)
        .map(Json)
        .map_err(internal_error)
}
