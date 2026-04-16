use crate::app::services::chat_service::PreparedChatRun;
use serde_json::Value;

/// 会话挂起的权限请求。
///
/// 当模型请求危险工具时，Rust 后端会把继续执行所需的上下文暂存到这里，
/// 等前端调用 `/api/chat/resume` 后再继续原来的工具循环。
#[derive(Debug, Clone)]
pub struct PendingPermission {
    pub conversation_id: String,
    pub prepared: PreparedChatRun,
    pub request_messages: Vec<Value>,
    pub tool_calls: Vec<Value>,
    pub next_index: usize,
}
