use serde_json::Value;

/// token 使用统计快照。
#[derive(Debug, Clone, Copy, Default)]
pub struct UsageSnapshot {
    pub prompt_tokens: i64,
    pub completion_tokens: i64,
    pub total_tokens: i64,
}

impl UsageSnapshot {
    /// 从一条 SSE data 对象里的 `usage` 字段累加（上游每条 chunk 都可能带）。
    pub fn absorb(&mut self, usage_obj: &Value) {
        if let Some(prompt) = usage_obj.get("prompt_tokens").and_then(|v| v.as_i64()) {
            self.prompt_tokens = prompt;
        }
        if let Some(completion) = usage_obj.get("completion_tokens").and_then(|v| v.as_i64()) {
            self.completion_tokens = completion;
        }
        self.total_tokens = usage_obj
            .get("total_tokens")
            .and_then(|v| v.as_i64())
            .unwrap_or_else(|| self.prompt_tokens + self.completion_tokens);
    }
}
