//! 聊天执行器。
//!
//! 把原本塞在 `api/handlers/chat.rs` 里的 `stream_upstream_chat` /
//! `resume_pending_permission` 两段流程收口到 app 层。handler 只负责 HTTP
//! 入口与 SSE channel 封装，真正的工具循环、failover、audit retry、permission
//! 挂起全部下沉到 `ChatExecutor`。

use crate::app::services::chat::failover::classify_upstream_error;
use crate::app::services::chat::permission::PendingPermission;
use crate::app::services::chat::policy::{DefaultFailoverPolicy, FailoverPolicy};
use crate::app::services::chat::run_phase::{emit, RunPhase};
use crate::app::services::chat::tool_loop::{
    accumulate_tool_call_deltas, parse_tool_call, tool_call_id,
};
use crate::app::services::chat::usage::UsageSnapshot;
use crate::app::services::chat_service::{ChatService, PreparedChatRun};
use crate::domain::run::{RunError, RunStatus};
use std::sync::Arc;
use futures_util::StreamExt as FuturesStreamExt;
use serde_json::Value;
use std::collections::BTreeMap;
use tokio::sync::mpsc;

/// 执行器对外发事件的 sink。
///
/// 上层（HTTP SSE handler 或 WebSocket voice bridge）各自把 `Value` 再翻译成
/// 自己需要的传输格式（SSE Event / WS Message）。这样执行器不再绑定 axum::Event。
pub type EventTx = mpsc::Sender<Value>;

/// 把内部 `anyhow::Error` 归并成 `RunError`。
///
/// 保留 `classify_upstream_error` 的分类结果，以便 handler 层按变体平铺到 SSE。
fn to_run_error(err: anyhow::Error) -> RunError {
    let msg = err.to_string();
    let class = classify_upstream_error(&msg);
    RunError::Upstream(class.to_string())
}

#[derive(Clone)]
pub struct ChatExecutor {
    chat_service: ChatService,
    failover_policy: Arc<dyn FailoverPolicy>,
}

impl ChatExecutor {
    pub fn new(chat_service: ChatService) -> Self {
        Self {
            chat_service,
            failover_policy: Arc::new(DefaultFailoverPolicy::default()),
        }
    }

    /// 单次聊天主链路：tool_calls 循环 + failover + audit retry。
    pub async fn stream_once(
        &self,
        mut prepared: PreparedChatRun,
        tx: EventTx,
    ) -> Result<(), RunError> {
        let chat_service = &self.chat_service;
        let conv_id = prepared.conversation_id.clone();
        chat_service.set_run_status(&conv_id, RunStatus::Running);
        let result = self.run_loop(&mut prepared, tx).await;
        let next_status = match &result {
            Ok(_) => {
                if chat_service.is_aborted(&conv_id) {
                    RunStatus::Aborted
                } else if chat_service.run_status(&conv_id) == RunStatus::AwaitingPermission {
                    RunStatus::AwaitingPermission
                } else {
                    RunStatus::Done
                }
            }
            Err(_) => RunStatus::Done,
        };
        chat_service.set_run_status(&conv_id, next_status);
        result.map_err(to_run_error)
    }

    async fn run_loop(
        &self,
        prepared: &mut PreparedChatRun,
        tx: EventTx,
    ) -> anyhow::Result<()> {
        let chat_service = &self.chat_service;
        let mut request_messages = prepared.messages.clone();
        let mut round = 0usize;
        let mut failover_idx = 0usize;
        let mut audit_retry_count = 0usize;

        loop {
            round += 1;
            if round > 4 {
                emit(
                    &tx,
                    RunPhase::Error {
                        content: "工具循环超过上限，已终止".to_string(),
                        error_class: "tool_loop_limit",
                    },
                )
                .await;
                return Ok(());
            }

            let response = match chat_service
                .send_stream_request(&prepared, &request_messages)
                .await
            {
                Ok(resp) => resp,
                Err(err) => {
                    let err_class = classify_upstream_error(&err.to_string());
                    let run_err = RunError::Upstream(err_class.to_string());
                    let picked = self.failover_policy.next_candidate(
                        &run_err,
                        &prepared.fallback_chain,
                        failover_idx,
                    );
                    let Some((next_idx, fb)) = picked else {
                        if failover_idx > 0 {
                            emit(
                                &tx,
                                RunPhase::FailoverExhausted {
                                    model: prepared.model_name.clone(),
                                    reason: err.to_string(),
                                    error_class: err_class,
                                },
                            )
                            .await;
                        }
                        return Err(err);
                    };
                    failover_idx = next_idx;
                    let failover_event = RunPhase::FailoverStep {
                        from_model: prepared.model_name.clone(),
                        to_model: fb.model_name.clone(),
                        provider: fb.provider.clone(),
                        reason: err.to_string(),
                        error_class: err_class,
                    }
                    .to_event();
                    chat_service.record_failover(failover_event.clone());
                    let _ = tx.send(failover_event).await;
                    prepared.model_name = fb.model_name;
                    prepared.model_id = fb.model_id;
                    prepared.provider = fb.provider;
                    prepared.api_url = fb.api_url;
                    prepared.api_key = fb.api_key;
                    prepared.route = "failover".to_string();
                    continue;
                }
            };
            let mut stream = response.bytes_stream();
            let mut buffer = String::new();
            let mut rendered = String::new();
            let mut usage = UsageSnapshot::default();
            let mut aborted = false;
            let mut tool_calls_acc: BTreeMap<usize, Value> = BTreeMap::new();
            let mut done_seen = false;

            while let Some(item) = FuturesStreamExt::next(&mut stream).await {
                let bytes = item?;
                if chat_service.is_aborted(&prepared.conversation_id) {
                    aborted = true;
                    break;
                }

                buffer.push_str(&String::from_utf8_lossy(&bytes));
                while let Some(idx) = buffer.find('\n') {
                    let line = buffer[..idx].trim().to_string();
                    buffer = buffer[idx + 1..].to_string();
                    if !line.starts_with("data:") {
                        continue;
                    }
                    let payload = line.trim_start_matches("data:").trim();
                    if payload.is_empty() {
                        continue;
                    }
                    if payload == "[DONE]" {
                        done_seen = true;
                        break;
                    }

                    let Ok(data) = serde_json::from_str::<Value>(payload) else {
                        continue;
                    };

                    if let Some(text) = data
                        .get("choices")
                        .and_then(|v| v.as_array())
                        .and_then(|arr| arr.first())
                        .and_then(|v| v.get("delta"))
                        .and_then(|v| v.get("content"))
                        .and_then(|v| v.as_str())
                    {
                        if !text.is_empty() {
                            rendered.push_str(text);
                            emit(
                                &tx,
                                RunPhase::Text {
                                    content: text.to_string(),
                                },
                            )
                            .await;
                        }
                    }

                    if let Some(tool_deltas) = data
                        .get("choices")
                        .and_then(|v| v.as_array())
                        .and_then(|arr| arr.first())
                        .and_then(|v| v.get("delta"))
                        .and_then(|v| v.get("tool_calls"))
                        .and_then(|v| v.as_array())
                    {
                        accumulate_tool_call_deltas(&mut tool_calls_acc, tool_deltas);
                    }

                    if let Some(usage_obj) = data.get("usage") {
                        usage.absorb(usage_obj);
                    }
                }
                if done_seen {
                    break;
                }
            }

            let tool_calls = tool_calls_acc.into_values().collect::<Vec<_>>();

            if aborted {
                chat_service.finalize_chat(&prepared, rendered, usage, true)?;
                emit(&tx, RunPhase::Aborted).await;
                return Ok(());
            }

            let call_type = if round == 1 { "chat" } else { "tool_followup" };
            emit(
                &tx,
                RunPhase::Usage {
                    prompt: usage.prompt_tokens,
                    completion: usage.completion_tokens,
                    total: usage.total_tokens,
                    call_type,
                    model: prepared.model_id.clone(),
                    provider: prepared.provider.clone(),
                },
            )
            .await;

            if tool_calls.is_empty() {
                let (audit_ok, audit_reason) =
                    chat_service.audit_answer(&prepared.query, &rendered);
                emit(
                    &tx,
                    RunPhase::Audit {
                        ok: audit_ok,
                        reason: audit_reason.clone(),
                        retry_count: audit_retry_count,
                    },
                )
                .await;
                if !audit_ok && audit_retry_count < prepared.audit_retry_cap {
                    audit_retry_count += 1;
                    request_messages.push(serde_json::json!({
                        "role": "user",
                        "content": format!("请自我纠正上一版回答：{}。原问题：{}", audit_reason, prepared.query)
                    }));
                    continue;
                }
                if !audit_ok
                    && audit_retry_count >= prepared.audit_retry_cap
                    && prepared.audit_retry_cap > 0
                {
                    emit(
                        &tx,
                        RunPhase::Error {
                            content: format!(
                                "回答质量检查未通过且超过重试上限：{}",
                                audit_reason
                            ),
                            error_class: "regenerate_exhausted",
                        },
                    )
                    .await;
                    return Ok(());
                }
                chat_service.finalize_chat(&prepared, rendered, usage, false)?;
                if prepared.plan_enabled {
                    emit(
                        &tx,
                        RunPhase::StepDone {
                            step: "执行计划".to_string(),
                        },
                    )
                    .await;
                }
                emit(
                    &tx,
                    RunPhase::Done {
                        model_id: prepared.model_id.clone(),
                        tier: prepared.tier.clone(),
                        route: prepared.route.clone(),
                    },
                )
                .await;
                return Ok(());
            }

            let assistant_tool_msg = serde_json::json!({
                "role": "assistant",
                "content": rendered,
                "tool_calls": tool_calls
            });
            request_messages.push(assistant_tool_msg.clone());
            chat_service.append_assistant_tool_call_message(
                &prepared.conversation_id,
                rendered.clone(),
                &prepared.model_id,
                assistant_tool_msg
                    .get("tool_calls")
                    .cloned()
                    .unwrap_or_default(),
            )?;

            for (tool_idx, tool_call) in tool_calls.iter().cloned().enumerate() {
                let (name, args) = parse_tool_call(&tool_call);

                if chat_service.requires_permission(&name) {
                    chat_service.set_pending_permission(PendingPermission {
                        conversation_id: prepared.conversation_id.clone(),
                        prepared: prepared.clone(),
                        request_messages: request_messages.clone(),
                        tool_calls: tool_calls.clone(),
                        next_index: tool_idx,
                    });
                    let description = format!(
                        "工具 `{}` 需要用户审批。参数：{}",
                        name,
                        serde_json::to_string_pretty(&args).unwrap_or_default()
                    );
                    emit(
                        &tx,
                        RunPhase::PermissionRequired {
                            tool_name: name.clone(),
                            args: args.clone(),
                            description,
                        },
                    )
                    .await;
                    return Ok(());
                }

                emit(&tx, RunPhase::ToolStart { name: name.clone() }).await;

                let result = chat_service
                    .execute_tool_tracked(Some(&prepared.conversation_id), &name, &args)
                    .await
                    .unwrap_or_else(|err| err.to_string());
                let call_id = tool_call_id(&tool_call);

                request_messages.push(serde_json::json!({
                    "role": "tool",
                    "tool_call_id": call_id,
                    "content": result
                }));
                chat_service.append_tool_message(
                    &prepared.conversation_id,
                    &call_id,
                    &name,
                    &args,
                    result.clone(),
                )?;

                emit(&tx, RunPhase::ToolDone { name }).await;
            }
        }
    }

    /// 权限确认后，从挂起点继续工具循环并进入下一轮上游请求。
    pub async fn resume(
        &self,
        pending: PendingPermission,
        granted: bool,
        tx: EventTx,
    ) -> Result<(), RunError> {
        let chat_service = &self.chat_service;
        let mut request_messages = pending.request_messages.clone();

        for tool_call in pending.tool_calls.iter().skip(pending.next_index) {
            let (name, args) = parse_tool_call(tool_call);
            let call_id = tool_call_id(tool_call);

            if chat_service.requires_permission(&name) && !granted {
                let denied = "用户拒绝了此操作，已取消执行。".to_string();
                request_messages.push(serde_json::json!({
                    "role": "tool",
                    "tool_call_id": call_id,
                    "content": denied
                }));
                chat_service
                    .append_tool_message(
                        &pending.conversation_id,
                        &call_id,
                        &name,
                        &args,
                        denied,
                    )
                    .map_err(to_run_error)?;
                break;
            }

            emit(&tx, RunPhase::ToolStart { name: name.clone() }).await;
            let result = chat_service
                .execute_tool_tracked(Some(&pending.conversation_id), &name, &args)
                .await
                .unwrap_or_else(|err| err.to_string());
            request_messages.push(serde_json::json!({
                "role": "tool",
                "tool_call_id": call_id,
                "content": result
            }));
            chat_service
                .append_tool_message(
                    &pending.conversation_id,
                    &call_id,
                    &name,
                    &args,
                    result,
                )
                .map_err(to_run_error)?;
            emit(&tx, RunPhase::ToolDone { name }).await;
        }

        let mut prepared = pending.prepared;
        prepared.messages = request_messages;
        self.stream_once(prepared, tx).await
    }
}
