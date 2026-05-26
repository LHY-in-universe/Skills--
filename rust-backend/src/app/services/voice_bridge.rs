//! 语音桥服务。
//!
//! 把 ASR 文本驱动 `ChatExecutor`、按句切分 → Rust Edge TTS 流式合成
//! 的全部链路封装在一起。原本通过 `voice_worker.py` 子进程的 stdin/stdout
//! 串联 TTS 已经全部下沉到 Rust，本模块直接把音频写回 WebSocket。

use crate::app::services::chat::executor::ChatExecutor;
use crate::app::services::chat_service::ChatService;
use crate::app::services::edge_tts;
use axum::extract::ws::{Message, WebSocket};
use base64::Engine;
use futures_util::stream::SplitSink;
use futures_util::SinkExt;
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::sync::Mutex as AsyncMutex;

/// WebSocket sink 的共享句柄。两端都需要写：
/// - 主循环（chat 事件、voice_session_state）
/// - TTS 后台任务（audio_stream）
pub type ClientSink = Arc<AsyncMutex<SplitSink<WebSocket, Message>>>;

const DEFAULT_TTS_VOICE: &str = "zh-CN-XiaoxiaoNeural";

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

    /// 按 ASR 文本触发一次内部聊天。
    /// `on_event` 收到执行器吐出的 JSON；TTS 切句直接走 Rust Edge TTS 写回 WS。
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
        let prepared = self.chat_service.prepare_chat(&text, conv_id.as_deref()).await?;
        let run_guard = self
            .chat_service
            .acquire_run_slot(&prepared.conversation_id)
            .await;

        let (event_tx, mut event_rx) = mpsc::channel::<Value>(64);
        let (tts_tx, tts_rx) = mpsc::channel::<String>(16);
        let tts_task = spawn_tts_consumer(tts_rx, client_tx.clone());

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
            if let Err(err) = executor
                .stream_once(prepared_clone, event_tx_exec.clone())
                .await
            {
                let _ = event_tx_exec
                    .send(serde_json::json!({
                        "type": "error",
                        "content": err.to_string(),
                        "error_class": match err {
                            crate::domain::run::RunError::Upstream { ref class, .. } => class.clone(),
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
        let _ = exec_handle.await;
        let _ = tts_task.await;
        Ok(())
    }

    /// 中断指定会话（取代原来的 self-HTTP POST /api/chat/abort）。
    pub fn abort(&self, conv_id: Option<&str>) {
        self.chat_service.abort(conv_id);
    }
}

/// 后台任务：串行消费 TTS 文本片段，逐段调用 Edge TTS，写回 WebSocket。
fn spawn_tts_consumer(
    mut tts_rx: mpsc::Receiver<String>,
    client_tx: ClientSink,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        while let Some(chunk_text) = tts_rx.recv().await {
            let sink = client_tx.clone();
            let result = edge_tts::stream_audio(&chunk_text, DEFAULT_TTS_VOICE, |audio_bytes| {
                if audio_bytes.is_empty() {
                    return;
                }
                let b64 = base64::engine::general_purpose::STANDARD.encode(&audio_bytes);
                let payload = serde_json::json!({
                    "type": "audio_stream",
                    "data": b64,
                });
                let sink_inner = sink.clone();
                tokio::spawn(async move {
                    let mut guard = sink_inner.lock().await;
                    let _ = guard.send(Message::Text(payload.to_string().into())).await;
                });
            })
            .await;
            if let Err(err) = result {
                tracing::warn!(error = %err, "Edge TTS 合成失败");
            }
        }
    })
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

/// 公开给 handler 用：构造 `voice_session_state` 事件 JSON。
pub fn voice_session_state_payload(conv_id: Option<&str>, phase: &str, source: &str) -> Value {
    serde_json::json!({
        "type": "voice_session_state",
        "conv_id": conv_id,
        "phase": phase,
        "source": source,
    })
}
