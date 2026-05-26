use skills_rust_backend::api::router::build_router;
use skills_rust_backend::app::state::AppState;
use std::net::SocketAddr;
use std::path::PathBuf;
use tracing::info;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            std::env::var("RUST_LOG")
                .unwrap_or_else(|_| "skills_rust_backend=debug,tower_http=debug".to_string()),
        )
        .with_target(false)
        .init();

    // 项目根目录默认取当前 crate 的上一级目录，也就是仓库根目录。
    // 这样可以直接复用现有的 siliconflow 配置目录和 webapp/backend 下的模型配置。
    let project_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("rust-backend 必须位于仓库根目录下一层")
        .to_path_buf();

    let state = AppState::bootstrap(project_root).await?;
    let app = build_router(state);

    let addr = SocketAddr::from(([127, 0, 0, 1], 18000));
    info!("Rust backend listening on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
