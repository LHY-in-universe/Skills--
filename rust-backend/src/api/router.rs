use crate::api::handlers;
use crate::app::state::AppState;
use axum::routing::{get, patch, post};
use axum::Router;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;

/// 构建 HTTP 路由。
///
/// 这一层只做协议装配，不承载业务判断。后续如果把 Python 后端完全替换，
/// 所有公开 API 都应该在这里统一注册，并映射到 application layer 的 use case。
pub fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(handlers::health))
        .route(
            "/api/config",
            get(handlers::get_config).post(handlers::update_config),
        )
        .route(
            "/api/models",
            get(handlers::get_models).post(handlers::create_model),
        )
        .route(
            "/api/models/:name",
            patch(handlers::update_model).delete(handlers::delete_model),
        )
        .route(
            "/api/providers/catalog",
            get(handlers::get_providers_catalog),
        )
        .route(
            "/api/runtime-settings",
            get(handlers::get_runtime_settings).post(handlers::save_runtime_settings),
        )
        .route("/api/doctor", get(handlers::get_doctor))
        .route("/api/doctor/fix", post(handlers::doctor_fix))
        .route("/api/security-audit", get(handlers::get_security_audit))
        .route("/api/auth-profiles", get(handlers::get_auth_profiles))
        .route("/api/runtime-health", get(handlers::get_runtime_health))
        .route("/api/model-connectivity", get(handlers::get_model_connectivity))
        .route("/api/failover/recent", get(handlers::get_failover_recent))
        .route(
            "/api/observability/summary",
            get(handlers::get_observability_summary),
        )
        .route(
            "/api/observability/events",
            get(handlers::get_observability_events),
        )
        .route("/api/skills", get(handlers::get_skills))
        .route("/api/skills/runtime", get(handlers::get_skills_runtime))
        .route("/api/skills/catalog", get(handlers::get_skills_catalog))
        .route("/api/skills/install", post(handlers::install_skill))
        .route("/api/skills/uninstall", post(handlers::uninstall_skill))
        .route("/api/skills/rescan", post(handlers::rescan_skills))
        .route("/api/skills/toggle", post(handlers::toggle_skill))
        .route("/api/skills/:name", patch(handlers::update_skill))
        .route(
            "/api/routing",
            get(handlers::get_routing).post(handlers::save_routing),
        )
        .route(
            "/api/lark/config",
            get(handlers::get_lark_config).post(handlers::save_lark_config),
        )
        .route(
            "/api/terminal/cwd",
            get(handlers::get_terminal_cwd).post(handlers::save_terminal_cwd),
        )
        .route(
            "/api/conversations",
            get(handlers::list_conversations).post(handlers::create_conversation),
        )
        .route(
            "/api/conversations/:conv_id/activate",
            post(handlers::activate_conversation),
        )
        .route(
            "/api/conversations/:conv_id",
            patch(handlers::rename_conversation).delete(handlers::delete_conversation),
        )
        .route("/api/history", get(handlers::get_history))
        .route("/api/history/clear", post(handlers::clear_history))
        .route("/api/token-usage", get(handlers::get_token_usage))
        .route("/api/chat", post(handlers::chat))
        .route("/api/chat/resume", post(handlers::resume_chat))
        .route("/api/chat/abort", post(handlers::abort_chat))
        .route("/api/voice/bridge", get(handlers::voice_bridge))
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any),
        )
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
