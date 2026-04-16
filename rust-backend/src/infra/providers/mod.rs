//! 上游 provider 差异的适配层。
//!
//! 第一版把「构造 Chat Completions payload」抽成 trait，给 SiliconFlow / NVIDIA /
//! DeepSeek / Kimi 等将来需要分化的地方留好入口。当前所有 provider 都走
//! OpenAI 兼容端点，所以默认驱动就是 `OpenAiCompatDriver`。

pub mod openai_compat;

pub use openai_compat::OpenAiCompatDriver;

use serde_json::Value;

/// 上游 provider 驱动。
///
/// 目前只抽了 payload 构造；chunk 解析仍走 executor 内联的 OpenAI 格式。等某个
/// provider 真的需要不同的 SSE 分片格式时，再往这里加 `parse_chunk`。
pub trait ProviderDriver: Send + Sync {
    /// 构造一次 Chat Completions 请求体。
    fn build_payload(
        &self,
        model_id: &str,
        messages: &[Value],
        tools: &[Value],
    ) -> Value;
}
