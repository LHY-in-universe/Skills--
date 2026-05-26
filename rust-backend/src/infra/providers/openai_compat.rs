//! OpenAI 兼容格式的默认驱动。
//!
//! SiliconFlow / NVIDIA / DeepSeek / Kimi 当前都能用同一套 payload，所以这一份
//! 实现覆盖全部 provider；将来如果某家差异化，就新增子模块而不改这里。

use super::ProviderDriver;
use serde_json::{json, Value};

#[derive(Debug, Clone, Default)]
pub struct OpenAiCompatDriver;

impl ProviderDriver for OpenAiCompatDriver {
    fn build_payload(&self, model_id: &str, messages: &[Value], tools: &[Value]) -> Value {
        let mut payload = json!({
            "model": model_id,
            "messages": messages,
            "stream": true,
            "stream_options": { "include_usage": true },
        });
        if !tools.is_empty() {
            payload["tools"] = json!(tools);
        }
        payload
    }
}
