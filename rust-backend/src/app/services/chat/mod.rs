//! chat_service 的内部拆分。
//!
//! `ChatService` 在这一轮重构里继续作为对外门面：
//! handler 层只调用 `ChatService` 的少量方法，具体算法逐步下沉到这几个子模块：
//!
//! - `planner`    — 计划/审计启发式
//! - `failover`   — 候选链构造与上游错误分类
//! - `permission` — 危险工具挂起态的数据结构
//! - `tool_loop`  — tool_calls 解析与累加辅助函数
//! - `usage`      — token 使用累加

pub mod executor;
pub mod failover;
pub mod permission;
pub mod planner;
pub mod policy;
pub mod run_phase;
pub mod tool_loop;
pub mod usage;
