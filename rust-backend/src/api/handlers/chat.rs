use crate::api::handlers::shared::{
    internal_error, sse_json, AbortChatRequest, ChatRequest, PermissionResumeRequest,
};
use crate::app::state::AppState;
use crate::domain::run::RunError;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::Json;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tokio_stream::Stream;

/// 把 `RunError` 平铺到前端 SSE error payload。
///
/// 字段形状保持与改造前一致：`type=error` + `content` + `error_class`。
/// 具体 class 由 executor 内部的 `classify_upstream_error` 决定，
/// 这里只负责按变体分流。
fn run_error_to_sse(err: &RunError) -> serde_json::Value {
    match err {
        RunError::Aborted => serde_json::json!({ "type": "aborted" }),
        RunError::Upstream {
            class,
            message,
            provider,
            model,
            api_url,
        } => serde_json::json!({
            "type": "error",
            "content": message,
            "error_class": class,
            "provider": provider,
            "model": model,
            "api_url": api_url,
        }),
        RunError::Tool(msg) => serde_json::json!({
            "type": "error",
            "content": msg,
            "error_class": "tool_error",
        }),
        RunError::PermissionDenied(msg) => serde_json::json!({
            "type": "error",
            "content": msg,
            "error_class": "permission_denied",
        }),
        RunError::InvalidState(msg) => serde_json::json!({
            "type": "error",
            "content": msg,
            "error_class": "invalid_state",
        }),
    }
}

/// 发起一次聊天请求，SSE 流式返回。
pub async fn chat(
    State(state): State<AppState>,
    Json(req): Json<ChatRequest>,
) -> Result<
    Sse<impl Stream<Item = Result<Event, std::convert::Infallible>>>,
    (StatusCode, Json<serde_json::Value>),
> {
    let prepared = state
        .chat_service
        .prepare_chat(&req.user_input, req.conv_id.as_deref())
        .await
        .map_err(internal_error)?;
    let run_guard = state
        .chat_service
        .acquire_run_slot(&prepared.conversation_id)
        .await;

    let (event_tx, event_rx) = mpsc::channel::<serde_json::Value>(64);
    let (sse_tx, sse_rx) = mpsc::channel::<Result<Event, std::convert::Infallible>>(64);
    let executor = state.chat_executor.clone();

    tokio::spawn(relay_values_to_sse(event_rx, sse_tx));

    tokio::spawn(async move {
        let _run_guard = run_guard;
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

        if let Err(err) = executor.stream_once(prepared, event_tx.clone()).await {
            let _ = event_tx.send(run_error_to_sse(&err)).await;
        }
    });

    Ok(Sse::new(ReceiverStream::new(sse_rx)).keep_alive(KeepAlive::default()))
}

/// 把执行器吐出的 `Value` 流转成 axum SSE Event 流。
async fn relay_values_to_sse(
    mut rx: mpsc::Receiver<serde_json::Value>,
    tx: mpsc::Sender<Result<Event, std::convert::Infallible>>,
) {
    while let Some(value) = rx.recv().await {
        if tx.send(Ok(sse_json(value))).await.is_err() {
            break;
        }
    }
}

/// 中断当前 / 指定会话的聊天流。
pub async fn abort_chat(
    State(state): State<AppState>,
    Json(req): Json<AbortChatRequest>,
) -> Json<serde_json::Value> {
    state.chat_service.abort(req.conv_id.as_deref());
    Json(serde_json::json!({ "status": "ok" }))
}

/// 权限确认后继续挂起的工具循环。
pub async fn resume_chat(
    State(state): State<AppState>,
    Json(req): Json<PermissionResumeRequest>,
) -> Result<
    Sse<impl Stream<Item = Result<Event, std::convert::Infallible>>>,
    (StatusCode, Json<serde_json::Value>),
> {
    let conv_id = req
        .conv_id
        .clone()
        .or_else(|| {
            state
                .conversation_service
                .list()
                .ok()?
                .into_iter()
                .find(|c| c.active)
                .map(|c| c.id)
        })
        .ok_or_else(|| internal_error(anyhow::anyhow!("缺少 conv_id，且当前没有 active 会话")))?;

    let run_guard = state.chat_service.acquire_run_slot(&conv_id).await;
    let pending = state
        .chat_service
        .take_pending_permission(&conv_id)
        .ok_or_else(|| internal_error(anyhow::anyhow!("没有待处理的权限请求")))?;
    if req.granted && req.always_allow {
        if let Some(tool_name) = pending
            .tool_calls
            .get(pending.next_index)
            .and_then(|v| v.get("function"))
            .and_then(|v| v.get("name"))
            .and_then(|v| v.as_str())
        {
            state
                .config_service
                .allow_tool_always(tool_name)
                .map_err(internal_error)?;
        }
    }

    let (event_tx, event_rx) = mpsc::channel::<serde_json::Value>(64);
    let (sse_tx, sse_rx) = mpsc::channel::<Result<Event, std::convert::Infallible>>(64);
    let executor = state.chat_executor.clone();

    tokio::spawn(relay_values_to_sse(event_rx, sse_tx));

    tokio::spawn(async move {
        let _run_guard = run_guard;
        if let Err(err) = executor
            .resume(pending, req.granted, event_tx.clone())
            .await
        {
            let _ = event_tx.send(run_error_to_sse(&err)).await;
        }
    });
    Ok(Sse::new(ReceiverStream::new(sse_rx)).keep_alive(KeepAlive::default()))
}
