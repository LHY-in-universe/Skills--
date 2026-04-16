use crate::app::services::voice_bridge::{
    voice_session_state_payload, write_worker_line, WorkerStdin,
};
use crate::app::state::AppState;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::response::IntoResponse;
use base64::Engine;
use futures_util::{SinkExt, StreamExt as FuturesStreamExt};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::Mutex as AsyncMutex;

/// Rust 版语音桥入口。
///
/// 改造后不再自打 HTTP 到 `127.0.0.1:18000`：ASR 文本通过 `VoiceBridge` 直接驱动
/// `ChatExecutor` 的内部事件流；abort 通过 `ChatService::abort` 直接触发。
pub async fn voice_bridge(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| run_voice_bridge(socket, state))
}

async fn run_voice_bridge(client_socket: WebSocket, state: AppState) {
    let worker_python = state.config_service.voice_worker_python();
    let worker_script = state.config_service.voice_worker_script();

    let mut child = match Command::new(&worker_python)
        .arg(&worker_script)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
    {
        Ok(c) => c,
        Err(err) => {
            let mut socket = client_socket;
            let _ = socket
                .send(Message::Text(
                    serde_json::json!({
                        "type": "error",
                        "content": format!("Rust 无法启动语音 worker: {}", err),
                    })
                    .to_string()
                    .into(),
                ))
                .await;
            let _ = socket.close().await;
            return;
        }
    };

    let Some(child_stdin) = child.stdin.take() else {
        return;
    };
    let Some(child_stdout) = child.stdout.take() else {
        return;
    };

    let (client_tx_raw, mut client_rx) = client_socket.split();
    let client_tx = std::sync::Arc::new(AsyncMutex::new(client_tx_raw));
    let worker_stdin = std::sync::Arc::new(AsyncMutex::new(child_stdin));
    let current_conv_id = std::sync::Arc::new(AsyncMutex::new(None::<String>));

    let worker_reader_tx = client_tx.clone();
    let worker_reader_conv_id = current_conv_id.clone();
    let worker_reader_state = state.clone();
    let worker_stdout_task = tokio::spawn(async move {
        let reader = BufReader::new(child_stdout);
        let mut lines = reader.lines();
        while let Ok(Some(line)) = lines.next_line().await {
            if line.trim().is_empty() {
                continue;
            }
            let parsed: serde_json::Value = match serde_json::from_str(&line) {
                Ok(v) => v,
                Err(_) => continue,
            };

            {
                let mut tx = worker_reader_tx.lock().await;
                if tx
                    .send(Message::Text(parsed.to_string().into()))
                    .await
                    .is_err()
                {
                    break;
                }
            }

            if parsed.get("type").and_then(|v| v.as_str()) == Some("asr_result") {
                let text = parsed
                    .get("content")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .trim()
                    .to_string();
                if !text.is_empty() {
                    let tx = worker_reader_tx.clone();
                    let conv_id = worker_reader_conv_id.lock().await.clone();
                    let _ = send_voice_session_state(
                        &tx,
                        conv_id.clone(),
                        "processing",
                        "asr_result",
                    )
                    .await;
                    let bridge_state = worker_reader_state.clone();
                    tokio::spawn(async move {
                        if let Err(err) = stream_chat_to_voice_ws(
                            bridge_state,
                            text,
                            conv_id,
                            tx.clone(),
                        )
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
            }
        }
    });

    while let Some(Ok(msg)) = client_rx.next().await {
        match msg {
            Message::Binary(bin) => {
                let payload = serde_json::json!({
                    "type": "audio_chunk",
                    "data_b64": base64::engine::general_purpose::STANDARD.encode(bin),
                });
                if write_worker_json(&worker_stdin, &payload).await.is_err() {
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
                    "debug_config" | "end_utterance" => {
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
                            msg_type,
                        )
                        .await;
                        if write_worker_json(&worker_stdin, &value).await.is_err() {
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
                            if let Err(err) = stream_chat_to_voice_ws(
                                bridge_state,
                                text,
                                conv_id,
                                tx.clone(),
                            )
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
                        let _ = send_voice_session_state(
                            &client_tx,
                            conv_id,
                            "listening",
                            "abort",
                        )
                        .await;
                    }
                    _ => {}
                }
            }
            Message::Close(_) => break,
            Message::Ping(_) | Message::Pong(_) => {}
        }
    }

    let _ = write_worker_json(&worker_stdin, &serde_json::json!({ "type": "shutdown" })).await;
    let _ = worker_stdout_task.await;
    let _ = child.kill().await;
}

async fn write_worker_json(
    worker_stdin: &WorkerStdin,
    value: &serde_json::Value,
) -> anyhow::Result<()> {
    write_worker_line(worker_stdin, value).await
}

/// 把 `ChatExecutor` 的内部事件流翻译成 WebSocket 消息。
///
/// TTS 切句由 `VoiceBridge` 内部直接通过 Rust Edge TTS 合成并发送到前端，
/// 不再经由 voice_worker 的 stdin/stdout。
async fn stream_chat_to_voice_ws(
    state: AppState,
    text: String,
    conv_id: Option<String>,
    client_tx: std::sync::Arc<AsyncMutex<futures_util::stream::SplitSink<WebSocket, Message>>>,
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
                    let _ =
                        send_voice_session_state(&tx, conv_id.clone(), "speaking", "chat_done")
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
                    let _ = send_voice_session_state(&tx, conv_id, "listening", "tts_flushed")
                        .await;
                }
            });
        })
        .await?;
    Ok(())
}

async fn send_voice_session_state(
    client_tx: &std::sync::Arc<AsyncMutex<futures_util::stream::SplitSink<WebSocket, Message>>>,
    conv_id: Option<String>,
    phase: &str,
    source: &str,
) -> anyhow::Result<()> {
    let payload = voice_session_state_payload(conv_id.as_deref(), phase, source);
    let mut tx = client_tx.lock().await;
    tx.send(Message::Text(payload.to_string().into())).await?;
    Ok(())
}
