//! 语音桥服务。
//!
//! 原先 `handlers/voice.rs` 通过 self-HTTP POST 到 `127.0.0.1:18000/api/chat`
//! 驱动对话，再从 SSE 返回流里切句喂 TTS。本模块把这一层回环拆掉：
//!
//! - ASR 文本直接调 `ChatExecutor::stream_once` 拿内部事件流
//! - 事件按需转发给 WebSocket 客户端
//! - `type=text` 累积到 TTS 缓冲，按句切块交给 Rust Edge TTS 流式合成
//! - abort 改成直接调 `ChatService::abort`

use crate::app::services::chat::executor::ChatExecutor;
use crate::app::services::chat_service::ChatService;
use crate::app::services::edge_tts;
use axum::extract::ws::{Message, WebSocket};
use base64::Engine;
use futures_util::stream::SplitSink;
use futures_util::SinkExt;
use serde_json::Value;
use std::sync::Arc;
use tokio::io::AsyncWriteExt;
use tokio::process::ChildStdin;
use tokio::sync::mpsc;
use tokio::sync::Mutex as AsyncMutex;

pub type WorkerStdin = Arc<AsyncMutex<ChildStdin>>;
pub type ClientSink = Arc<AsyncMutex<SplitSink<WebSocket, Message>>>;

/// 构造 `voice_session_state` 事件 payload。
pub fn voice_session_state_payload(
    conv_id: Option<&str>,
    phase: &str,
    source: &str,
) -> Value {
    serde_json::json!({
        "type": "voice_session_state",
        "conv_id": conv_id,
        "phase": phase,
        "source": source,
    })
}

/// 把一个 JSON 消息以 `<line>\n` 形式写入 voice_worker 的 stdin。
pub async fn write_worker_line(
    worker_stdin: &WorkerStdin,
    value: &Value,
) -> anyhow::Result<()> {
    let mut stdin = worker_stdin.lock().await;
    stdin.write_all(value.to_string().as_bytes()).await?;
    stdin.write_all(b"\n").await?;
    stdin.flush().await?;
    Ok(())
}

#[derive(Clone)]
pub struct VoiceBridge {
    chat_service: ChatService,
    chat_executor: ChatExecutor,
}

impl VoiceBridge {
    pub fn new(chat_service: ChatService, chat_executor: ChatExecutor) -> Self {
        Self {
            chat_service,
            chat_executor,
        }
    }

    /// 按 ASR 文本触发一次内部聊天，同步把事件流和 TTS 合成交给 handler 层处理。
    ///
    /// TTS 切句后直接通过 Rust Edge TTS 合成 MP3 并经 `client_tx` 推送到前端，
    /// 不再绕行 voice_worker 的 stdin/stdout。
    pub async fn run_chat<F>(
        &self,
        text: String,
        conv_id: Option<String>,
        client_tx: ClientSink,
        mut on_event: F,
    ) -> anyhow::Result<()>
    where
        F: FnMut(Value) + Send,
    {
        let prepared = self
            .chat_service
            .prepare_chat(&text, conv_id.as_deref())?;
        let run_guard = self
            .chat_service
            .acquire_run_slot(&prepared.conversation_id)
            .await;

        let (event_tx, mut event_rx) = mpsc::channel::<Value>(64);

        let _ = event_tx
            .send(serde_json::json!({
                "type": "start",
                "_model": prepared.model_name,
                "_model_id": prepared.model_id,
                "_provider": prepared.provider,
                "_conv_id": prepared.conversation_id,
                "_route": prepared.route,
                "_tier": prepared.tier,
            }))
            .await;
        if prepared.plan_enabled && !prepared.plan_steps.is_empty() {
            let _ = event_tx
                .send(serde_json::json!({
                    "type": "plan",
                    "steps": prepared.plan_steps,
                }))
                .await;
            let _ = event_tx
                .send(serde_json::json!({
                    "type": "step_start",
                    "step": "执行计划"
                }))
                .await;
        }

        let executor = self.chat_executor.clone();
        let prepared_clone = prepared.clone();
        let event_tx_exec = event_tx.clone();
        let exec_handle = tokio::spawn(async move {
            let _run_guard = run_guard;
            if let Err(err) = executor.stream_once(prepared_clone, event_tx_exec.clone()).await {
                let _ = event_tx_exec
                    .send(serde_json::json!({
                        "type": "error",
                        "content": err.to_string(),
                        "error_class": match err {
                            crate::domain::run::RunError::Upstream(ref c) => c.clone(),
                            crate::domain::run::RunError::Aborted => "aborted".to_string(),
                            crate::domain::run::RunError::Tool(_) => "tool_error".to_string(),
                            crate::domain::run::RunError::PermissionDenied(_) =>
                                "permission_denied".to_string(),
                            crate::domain::run::RunError::InvalidState(_) =>
                                "invalid_state".to_string(),
                        }
                    }))
                    .await;
            }
        });
        drop(event_tx);

        // TTS 排队 channel：保证句子按序合成
        let (tts_tx, mut tts_rx) = mpsc::channel::<String>(16);
        let tts_client_tx = client_tx.clone();
        let tts_task = tokio::spawn(async move {
            while let Some(chunk_text) = tts_rx.recv().await {
                let tx = tts_client_tx.clone();
                if let Err(e) = edge_tts::stream_audio(
                    &chunk_text,
                    "zh-CN-XiaoxiaoNeural",
                    |audio_bytes| {
                        let b64 = base64::engine::general_purpose::STANDARD.encode(&audio_bytes);
                        let payload = serde_json::json!({
                            "type": "audio_stream",
                            "data": b64,
                        });
                        let tx_inner = tx.clone();
                        tokio::spawn(async move {
                            let mut sink = tx_inner.lock().await;
                            let _ = sink
                                .send(Message::Text(payload.to_string().into()))
                                .await;
                        });
                    },
                )
                .await
                {
                    tracing::warn!("Edge TTS error: {e}");
                }
            }
        });

        let mut tts_buffer = String::new();
        while let Some(parsed) = event_rx.recv().await {
            let event_type = parsed.get("type").and_then(|v| v.as_str()).unwrap_or("");

            if event_type == "text" {
                if let Some(content) = parsed.get("content").and_then(|v| v.as_str()) {
                    if !content.trim().is_empty() {
                        tts_buffer.push_str(content);
                        flush_tts_sentences(&mut tts_buffer, &tts_tx, false).await;
                    }
                }
            }
            if event_type == "done" {
                flush_tts_sentences(&mut tts_buffer, &tts_tx, true).await;
            }

            on_event(parsed);
        }
        flush_tts_sentences(&mut tts_buffer, &tts_tx, true).await;
        drop(tts_tx);
        let _ = tts_task.await;
        let _ = exec_handle.await;
        Ok(())
    }

    /// 中断指定会话（取代原来的 self-HTTP POST /api/chat/abort）。
    pub fn abort(&self, conv_id: Option<&str>) {
        self.chat_service.abort(conv_id);
    }
}

async fn flush_tts_sentences(
    tts_buffer: &mut String,
    tts_tx: &mpsc::Sender<String>,
    force_flush: bool,
) {
    let mut flush_upto = 0usize;
    let total_chars = tts_buffer.chars().count();
    for (idx, ch) in tts_buffer.char_indices() {
        if matches!(ch, '。' | '！' | '？' | '!' | '?' | '；' | ';' | '\n') {
            flush_upto = idx + ch.len_utf8();
        } else if matches!(ch, '，' | ',' | '、' | '：' | ':') && total_chars >= 48 {
            flush_upto = idx + ch.len_utf8();
        }
    }

    if force_flush && flush_upto == 0 && !tts_buffer.trim().is_empty() {
        flush_upto = tts_buffer.len();
    }

    if flush_upto == 0 && total_chars >= 72 {
        flush_upto = tts_buffer.len();
    }

    if flush_upto == 0 {
        return;
    }

    let chunk = tts_buffer[..flush_upto].trim().to_string();
    let rest = tts_buffer[flush_upto..].to_string();
    *tts_buffer = rest;

    if !chunk.is_empty() {
        let _ = tts_tx.send(chunk).await;
    }
}
