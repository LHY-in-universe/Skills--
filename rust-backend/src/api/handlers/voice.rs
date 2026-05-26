use crate::app::services::voice_bridge::{voice_session_state_payload, ClientSink};
use crate::app::services::voice_pipeline::{PipelineEvent, VoicePipeline};
use crate::app::state::AppState;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::response::IntoResponse;
use futures_util::{SinkExt, StreamExt as FuturesStreamExt};
use std::sync::Arc;
use tokio::sync::Mutex as AsyncMutex;

/// Rust 版语音桥入口。
///
/// 单 WebSocket 内进程闭环：
/// - 前端音频帧 → `VoicePipeline`（Rust 内 KWS/VAD/ASR）→ ASR 文本
/// - ASR 文本 → `VoiceBridge::run_chat` → 模型流式输出
/// - 切句 → Rust Edge TTS → `audio_stream` 帧写回前端
///
/// 不再 fork Python `voice_worker.py`、不再有 stdin/stdout JSON 协议。
pub async fn voice_bridge(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| run_voice_bridge(socket, state))
}

async fn run_voice_bridge(client_socket: WebSocket, state: AppState) {
    let models_root = state
        .config_service
        .project_root()
        .join("rust-backend/models/voice");

    let pipeline = match VoicePipeline::new(models_root) {
        Ok(p) => Arc::new(AsyncMutex::new(p)),
        Err(err) => {
            let mut socket = client_socket;
            let _ = socket
                .send(Message::Text(
                    serde_json::json!({
                        "type": "error",
                        "content": format!("voice pipeline init failed: {err}"),
                    })
                    .to_string()
                    .into(),
                ))
                .await;
            let _ = socket.close().await;
            return;
        }
    };

    let (client_tx_raw, mut client_rx) = client_socket.split();
    let client_tx: ClientSink = Arc::new(AsyncMutex::new(client_tx_raw));
    let current_conv_id = Arc::new(AsyncMutex::new(None::<String>));

    while let Some(Ok(msg)) = client_rx.next().await {
        match msg {
            Message::Binary(bin) => {
                let pipeline = pipeline.clone();
                let bytes = bin.to_vec();
                let events = {
                    let mut guard = pipeline.lock().await;
                    guard.push_audio_chunk(&bytes)
                };
                if dispatch_pipeline_events(events, &state, &client_tx, &current_conv_id)
                    .await
                    .is_err()
                {
                    break;
                }
            }
            Message::Text(text) => {
                let value: serde_json::Value = match serde_json::from_str(&text) {
                    Ok(v) => v,
                    Err(_) => continue,
                };
                let msg_type = value.get("type").and_then(|v| v.as_str()).unwrap_or("");
                match msg_type {
                    "debug_config" => {
                        let bypass = value
                            .get("bypass_wakeword")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false);
                        {
                            let mut guard = pipeline.lock().await;
                            guard.set_bypass_wakeword(bypass);
                        }
                        if let Some(conv_id) = value.get("conv_id").and_then(|v| v.as_str()) {
                            let conv_id = conv_id.trim();
                            let mut current = current_conv_id.lock().await;
                            *current = if conv_id.is_empty() {
                                None
                            } else {
                                Some(conv_id.to_string())
                            };
                        }
                        let conv_id = current_conv_id.lock().await.clone();
                        let _ = send_voice_session_state(
                            &client_tx,
                            conv_id,
                            "listening",
                            "debug_config",
                        )
                        .await;
                        {
                            let mut sink = client_tx.lock().await;
                            let _ = sink
                                .send(Message::Text(
                                    serde_json::json!({
                                        "type": "debug_config_ack",
                                        "bypass_wakeword": bypass,
                                    })
                                    .to_string()
                                    .into(),
                                ))
                                .await;
                        }
                    }
                    "end_utterance" => {
                        if let Some(conv_id) = value.get("conv_id").and_then(|v| v.as_str()) {
                            let conv_id = conv_id.trim();
                            let mut current = current_conv_id.lock().await;
                            *current = if conv_id.is_empty() {
                                None
                            } else {
                                Some(conv_id.to_string())
                            };
                        }
                        let events = {
                            let mut guard = pipeline.lock().await;
                            guard.flush_pending_asr(false)
                        };
                        let conv_id = current_conv_id.lock().await.clone();
                        let _ = send_voice_session_state(
                            &client_tx,
                            conv_id,
                            "listening",
                            "end_utterance",
                        )
                        .await;
                        if dispatch_pipeline_events(events, &state, &client_tx, &current_conv_id)
                            .await
                            .is_err()
                        {
                            break;
                        }
                    }
                    "debug_inject_text" => {
                        let text = value
                            .get("content")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .trim()
                            .to_string();
                        if text.is_empty() {
                            continue;
                        }
                        let conv_id = value
                            .get("conv_id")
                            .and_then(|v| v.as_str())
                            .map(str::trim)
                            .filter(|v| !v.is_empty())
                            .map(ToOwned::to_owned);
                        if let Some(conv_id) = conv_id.clone() {
                            let mut current = current_conv_id.lock().await;
                            *current = Some(conv_id);
                        }
                        let current = current_conv_id.lock().await.clone();
                        {
                            let mut tx = client_tx.lock().await;
                            if tx
                                .send(Message::Text(
                                    serde_json::json!({
                                        "type": "asr_result",
                                        "content": text,
                                    })
                                    .to_string()
                                    .into(),
                                ))
                                .await
                                .is_err()
                            {
                                break;
                            }
                        }
                        let _ = send_voice_session_state(
                            &client_tx,
                            current.clone(),
                            "processing",
                            "debug_inject_text",
                        )
                        .await;
                        let tx = client_tx.clone();
                        let conv_id = current;
                        let bridge_state = state.clone();
                        tokio::spawn(async move {
                            if let Err(err) =
                                stream_chat_to_voice_ws(bridge_state, text, conv_id, tx.clone())
                                    .await
                            {
                                let mut sink = tx.lock().await;
                                let _ = sink
                                    .send(Message::Text(
                                        serde_json::json!({
                                            "type": "error",
                                            "content": format!("voice_chat_bridge_failed: {}", err),
                                        })
                                        .to_string()
                                        .into(),
                                    ))
                                    .await;
                            }
                        });
                    }
                    "abort" => {
                        let conv_id = current_conv_id.lock().await.clone();
                        state.voice_bridge.abort(conv_id.as_deref());
                        let _ = send_voice_session_state(&client_tx, conv_id, "listening", "abort")
                            .await;
                    }
                    _ => {}
                }
            }
            Message::Close(_) => break,
            Message::Ping(_) | Message::Pong(_) => {}
        }
    }
}

async fn dispatch_pipeline_events(
    events: Vec<PipelineEvent>,
    state: &AppState,
    client_tx: &ClientSink,
    current_conv_id: &Arc<AsyncMutex<Option<String>>>,
) -> anyhow::Result<()> {
    for event in events {
        match event {
            PipelineEvent::Wakeword(kw) => {
                let mut sink = client_tx.lock().await;
                sink.send(Message::Text(
                    serde_json::json!({
                        "type": "wakeword",
                        "keyword": kw,
                    })
                    .to_string()
                    .into(),
                ))
                .await?;
            }
            PipelineEvent::AsrResult(text) => {
                {
                    let mut sink = client_tx.lock().await;
                    sink.send(Message::Text(
                        serde_json::json!({
                            "type": "asr_result",
                            "content": text,
                        })
                        .to_string()
                        .into(),
                    ))
                    .await?;
                }
                let conv_id = current_conv_id.lock().await.clone();
                let _ = send_voice_session_state(
                    client_tx,
                    conv_id.clone(),
                    "processing",
                    "asr_result",
                )
                .await;
                let bridge_state = state.clone();
                let tx = client_tx.clone();
                tokio::spawn(async move {
                    if let Err(err) =
                        stream_chat_to_voice_ws(bridge_state, text, conv_id, tx.clone()).await
                    {
                        let mut sink = tx.lock().await;
                        let _ = sink
                            .send(Message::Text(
                                serde_json::json!({
                                    "type": "error",
                                    "content": format!("voice_chat_bridge_failed: {}", err),
                                })
                                .to_string()
                                .into(),
                            ))
                            .await;
                    }
                });
            }
        }
    }
    Ok(())
}

async fn stream_chat_to_voice_ws(
    state: AppState,
    text: String,
    conv_id: Option<String>,
    client_tx: ClientSink,
) -> anyhow::Result<()> {
    let _ = send_voice_session_state(&client_tx, conv_id.clone(), "processing", "chat_start").await;

    let client_tx_for_cb = client_tx.clone();
    let conv_id_for_cb = conv_id.clone();
    state
        .voice_bridge
        .run_chat(text, conv_id, client_tx.clone(), move |value| {
            let tx = client_tx_for_cb.clone();
            let conv_id = conv_id_for_cb.clone();
            let event_type = value
                .get("type")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            tokio::spawn(async move {
                if event_type == "done" {
                    let _ = send_voice_session_state(&tx, conv_id.clone(), "speaking", "chat_done")
                        .await;
                }
                {
                    let mut sink = tx.lock().await;
                    if sink
                        .send(Message::Text(value.to_string().into()))
                        .await
                        .is_err()
                    {
                        return;
                    }
                }
                if event_type == "done" {
                    let _ =
                        send_voice_session_state(&tx, conv_id, "listening", "tts_flushed").await;
                }
            });
        })
        .await?;
    Ok(())
}

async fn send_voice_session_state(
    client_tx: &ClientSink,
    conv_id: Option<String>,
    phase: &str,
    source: &str,
) -> anyhow::Result<()> {
    let payload = voice_session_state_payload(conv_id.as_deref(), phase, source);
    let mut tx = client_tx.lock().await;
    tx.send(Message::Text(payload.to_string().into())).await?;
    Ok(())
}
