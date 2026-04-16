use serde::{Deserialize, Serialize};

/// 单条对话消息。
///
/// 这里优先兼容现有 Python 后端持久化格式，因此保留 `role/content`，
/// 并允许额外挂接任意 JSON 字段，避免第一阶段导入时丢失 tool_calls、
/// _model、_tokens 之类的扩展信息。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationMessage {
    pub role: String,
    #[serde(default)]
    pub content: String,
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

/// 会话列表项。
///
/// 对齐现有前端消费字段：
/// - `active`
/// - `message_count`
/// - `model`
#[derive(Debug, Clone, Serialize)]
pub struct ConversationSummary {
    pub id: String,
    pub name: String,
    pub created_at: f64,
    pub model: String,
    pub active: bool,
    pub message_count: usize,
}

/// 创建会话返回结构。
#[derive(Debug, Clone, Serialize)]
pub struct ConversationCreateResponse {
    pub id: String,
    pub name: String,
    pub created_at: f64,
    pub model: String,
    pub active: bool,
    pub message_count: usize,
}

/// 历史消息响应。
#[derive(Debug, Clone, Serialize)]
pub struct ConversationHistoryResponse {
    pub items: Vec<ConversationMessage>,
}

/// 重命名请求体。
#[derive(Debug, Clone, Deserialize)]
pub struct ConversationRenameRequest {
    pub name: String,
}

/// 通用操作响应。
#[derive(Debug, Clone, Serialize)]
pub struct ConversationActionResponse {
    pub status: &'static str,
    pub active_id: Option<String>,
}
