//! `ConfigService` 的职责分层。
//!
//! 原先 746+ LOC 的 `config_service.rs` 里塞满了 models / skills / routing /
//! runtime / observability / auth / permission 七类逻辑；本 mod 按职责把它们拆
//! 进各自文件。`ConfigService` 本体留在 `services/config_service.rs`，这里的文
//! 件只追加 `impl ConfigService { ... }` 块，handler 层签名保持不变。

pub mod auth;
pub mod connectivity;
pub mod local_models;
pub mod models;
pub mod observability;
pub mod routing;
pub mod runtime;
pub mod skills;
