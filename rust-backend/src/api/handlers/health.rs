use axum::Json;

/// 健康检查接口。
///
/// 当前只返回服务自身是否成功启动。后续会扩展为：
/// - 配置快照加载状态
/// - SQLite 连接状态
/// - provider 激活面状态
/// - 语音桥和技能运行时状态
pub async fn health() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "ok": true,
        "service": "skills-rust-backend",
        "version": env!("CARGO_PKG_VERSION"),
    }))
}
