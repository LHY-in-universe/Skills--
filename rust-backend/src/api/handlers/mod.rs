//! HTTP handler 模块。
//!
//! 按路由聚类拆成多个文件。`router.rs` 仍然通过 `handlers::<fn_name>` 访问，
//! 这里把各子模块的处理器统一 `pub use` 出去。

pub mod shared;

pub mod chat;
pub mod config;
pub mod conversations;
pub mod doctor;
pub mod health;
pub mod observability;
pub mod skills;
pub mod voice;

pub use chat::*;
pub use config::*;
pub use conversations::*;
pub use doctor::*;
pub use health::*;
pub use observability::*;
pub use skills::*;
pub use voice::*;
