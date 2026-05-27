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
    accumulate_tool_call_deltas, extract_prompt_tool_calls, parse_tool_call, tool_call_id,
};
use crate::app::services::chat::usage::UsageSnapshot;
use crate::app::services::chat_service::{ChatService, PreparedChatRun, ToolCallMode};
use crate::domain::run::{RunError, RunStatus};
use futures_util::StreamExt as FuturesStreamExt;
use serde_json::Value;
use std::collections::BTreeMap;
use std::sync::Arc;
use tokio::sync::mpsc;

/// 执行器对外发事件的 sink。
///
/// 上层（HTTP SSE handler 或 WebSocket voice bridge）各自把 `Value` 再翻译成
/// 自己需要的传输格式（SSE Event / WS Message）。这样执行器不再绑定 axum::Event。
pub type EventTx = mpsc::Sender<Value>;

/// 把内部 `anyhow::Error` 归并成 `RunError`。
///
/// 保留 `classify_upstream_error` 的分类结果，以便 handler 层按变体平铺到 SSE。
fn to_run_error(err: anyhow::Error, prepared: &PreparedChatRun) -> RunError {
    let msg = err.to_string();
    let class = classify_upstream_error(&msg);
    RunError::Upstream {
        class: class.to_string(),
        message: msg,
        provider: prepared.provider.clone(),
        model: prepared.model_id.clone(),
        api_url: prepared.api_url.clone(),
    }
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
        let started = std::time::Instant::now();
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
        tracing::debug!(
            conversation_id = %conv_id,
            model = %prepared.model_id,
            provider = %prepared.provider,
            final_status = %next_status.as_str(),
            elapsed_ms = started.elapsed().as_millis() as i64,
            "chat run finished"
        );
        result.map_err(|err| to_run_error(err, &prepared))
    }

    async fn run_loop(&self, prepared: &mut PreparedChatRun, tx: EventTx) -> anyhow::Result<()> {
        let chat_service = &self.chat_service;
        let mut request_messages = prepared.messages.clone();
        let mut round = 0usize;
        let mut failover_idx = 0usize;
        let mut audit_retry_count = 0usize;

        loop {
            round += 1;
            let round_started = std::time::Instant::now();
            let tool_call_mode = match prepared.tool_call_mode {
                ToolCallMode::Function => "function",
                ToolCallMode::Prompt => "prompt",
            };
            tracing::debug!(
                conversation_id = %prepared.conversation_id,
                model = %prepared.model_id,
                provider = %prepared.provider,
                round = round,
                tool_call_mode = %tool_call_mode,
                audit_retry_count = audit_retry_count,
                failover_idx = failover_idx,
                request_message_count = request_messages.len(),
                "chat round started"
            );
            if round > 4 {
                tracing::warn!(
                    conversation_id = %prepared.conversation_id,
                    model = %prepared.model_id,
                    provider = %prepared.provider,
                    round = round,
                    audit_retry_count = audit_retry_count,
                    prompt_fallback_attempted = prepared.prompt_fallback_attempted,
                    tool_call_mode = %tool_call_mode,
                    request_message_count = request_messages.len(),
                    "tool loop limit reached"
                );
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
                .send_stream_request(prepared, &request_messages)
                .await
            {
                Ok(resp) => resp,
                Err(err) => {
                    let err_class = classify_upstream_error(&err.to_string());
                    tracing::warn!(
                        model = %prepared.model_id,
                        provider = %prepared.provider,
                        api_url = %prepared.api_url,
                        error_class = %err_class,
                        error = %err,
                        "upstream request failed"
                    );
                    let run_err = RunError::Upstream {
                        class: err_class.to_string(),
                        message: err.to_string(),
                        provider: prepared.provider.clone(),
                        model: prepared.model_id.clone(),
                        api_url: prepared.api_url.clone(),
                    };
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
                        tracing::error!(
                            model = %prepared.model_id,
                            provider = %prepared.provider,
                            api_url = %prepared.api_url,
                            error_class = %err_class,
                            error = %err,
                            "upstream request failed and no failover candidate succeeded"
                        );
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
            let mut raw_chunk_debug_count = 0usize;

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
                    if prepared.provider == "nvidia" && raw_chunk_debug_count < 3 {
                        raw_chunk_debug_count += 1;
                        let compact =
                            serde_json::to_string(&data).unwrap_or_else(|_| payload.to_string());
                        let preview = compact.chars().take(1200).collect::<String>();
                        tracing::debug!(
                            conversation_id = %prepared.conversation_id,
                            model = %prepared.model_id,
                            provider = %prepared.provider,
                            round = round,
                            chunk_index = raw_chunk_debug_count,
                            chunk_preview = %preview,
                            "nvidia raw sse chunk"
                        );
                    }

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

            let mut tool_calls = tool_calls_acc.into_values().collect::<Vec<_>>();
            let (prompt_tool_calls, visible_rendered) = extract_prompt_tool_calls(&rendered);
            if tool_calls.is_empty() && !prompt_tool_calls.is_empty() {
                tracing::debug!(
                    conversation_id = %prepared.conversation_id,
                    model = %prepared.model_id,
                    provider = %prepared.provider,
                    round = round,
                    prompt_tool_call_count = prompt_tool_calls.len(),
                    "prompt-injected tool calls detected"
                );
                tool_calls = prompt_tool_calls;
                rendered = visible_rendered;
            }
            tracing::debug!(
                conversation_id = %prepared.conversation_id,
                model = %prepared.model_id,
                provider = %prepared.provider,
                round = round,
                tool_call_count = tool_calls.len(),
                rendered_chars = rendered.chars().count(),
                prompt_tokens = usage.prompt_tokens,
                completion_tokens = usage.completion_tokens,
                total_tokens = usage.total_tokens,
                elapsed_ms = round_started.elapsed().as_millis() as i64,
                "chat round finished"
            );

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
                let empty_response = rendered.trim().is_empty();
                if prepared.tool_call_mode == ToolCallMode::Function
                    && !prepared.prompt_fallback_attempted
                    && empty_response
                    && prepared
                        .routed_tool_names
                        .as_ref()
                        .map(|v| !v.is_empty())
                        .unwrap_or(false)
                {
                    prepared.tool_call_mode = ToolCallMode::Prompt;
                    prepared.prompt_fallback_attempted = true;
                    tracing::warn!(
                        conversation_id = %prepared.conversation_id,
                        model = %prepared.model_id,
                        provider = %prepared.provider,
                        round = round,
                        request_message_count = request_messages.len(),
                        "function calling produced no usable response, retrying with prompt tool calling"
                    );
                    continue;
                }
                let (audit_ok, audit_reason) = if prepared.tier == "easy" {
                    (true, "skipped".to_string())
                } else {
                    chat_service.audit_answer(&prepared.query, &rendered)
                };
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
                    tracing::warn!(
                        conversation_id = %prepared.conversation_id,
                        model = %prepared.model_id,
                        provider = %prepared.provider,
                        round = round,
                        next_audit_retry_count = audit_retry_count,
                        audit_retry_cap = prepared.audit_retry_cap,
                        audit_reason = %audit_reason,
                        rendered_chars = rendered.chars().count(),
                        "audit failed, retrying with self-correction prompt"
                    );
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
                            content: format!("回答质量检查未通过且超过重试上限：{}", audit_reason),
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
            tracing::debug!(
                conversation_id = %prepared.conversation_id,
                model = %prepared.model_id,
                provider = %prepared.provider,
                round = round,
                tool_call_count = tool_calls.len(),
                tool_names = %tool_calls
                    .iter()
                    .map(|tool_call| parse_tool_call(tool_call).0)
                    .collect::<Vec<_>>()
                    .join(","),
                "tool calls detected, continuing to tool execution"
            );

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

                let result = match chat_service
                    .execute_tool_tracked(Some(&prepared.conversation_id), &name, &args)
                    .await
                {
                    Ok(text) => text,
                    Err(err) => {
                        tracing::warn!(
                            conversation_id = %prepared.conversation_id,
                            tool_name = %name,
                            tool_args = %serde_json::to_string(&args).unwrap_or_default(),
                            error = %err,
                            "tool execution failed"
                        );
                        emit(
                            &tx,
                            RunPhase::Error {
                                content: format!("工具 `{}` 执行失败: {}", name, err),
                                error_class: "tool_error",
                            },
                        )
                        .await;
                        err.to_string()
                    }
                };
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

            tracing::debug!(
                conversation_id = %prepared.conversation_id,
                model = %prepared.model_id,
                provider = %prepared.provider,
                round = round,
                request_message_count = request_messages.len(),
                "tool execution round completed, continuing with tool follow-up request"
            );
        }
    }

    /// 权限确认后，从挂起点继续工具循环并进入下一轮上游请求。
    pub async fn resume(
        &self,
        pending: PendingPermission,
        granted: bool,
        tx: EventTx,
    ) -> Result<(), RunError> {
        let mut request_messages = pending.request_messages.clone();

        for tool_call in pending.tool_calls.iter().skip(pending.next_index) {
            let should_continue = self
                .resume_single_tool_call(
                    &pending,
                    &mut request_messages,
                    tool_call,
                    granted,
                    &tx,
                )
                .await?;
            if !should_continue {
                break;
            }
        }

        let mut prepared = pending.prepared;
        prepared.messages = request_messages;
        self.stream_once(prepared, tx).await
    }

    async fn resume_single_tool_call(
        &self,
        pending: &PendingPermission,
        request_messages: &mut Vec<Value>,
        tool_call: &Value,
        granted: bool,
        tx: &EventTx,
    ) -> Result<bool, RunError> {
        let chat_service = &self.chat_service;
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
                .append_tool_message(&pending.conversation_id, &call_id, &name, &args, denied)
                .map_err(|err| to_run_error(err, &pending.prepared))?;
            return Ok(false);
        }

        emit(tx, RunPhase::ToolStart { name: name.clone() }).await;
        let result = match chat_service
            .execute_tool_tracked_with_permission(Some(&pending.conversation_id), &name, &args)
            .await
        {
            Ok(text) => text,
            Err(err) => {
                tracing::warn!(
                    conversation_id = %pending.conversation_id,
                    tool_name = %name,
                    tool_args = %serde_json::to_string(&args).unwrap_or_default(),
                    error = %err,
                    "tool execution failed after permission resume"
                );
                emit(
                    tx,
                    RunPhase::Error {
                        content: format!("工具 `{}` 执行失败: {}", name, err),
                        error_class: "tool_error",
                    },
                )
                .await;
                err.to_string()
            }
        };
        request_messages.push(serde_json::json!({
            "role": "tool",
            "tool_call_id": call_id,
            "content": result
        }));
        chat_service
            .append_tool_message(&pending.conversation_id, &call_id, &name, &args, result)
            .map_err(|err| to_run_error(err, &pending.prepared))?;
        emit(tx, RunPhase::ToolDone { name }).await;
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::{ChatExecutor, EventTx};
    use crate::app::run_registry::RunRegistry;
    use crate::app::services::chat::permission::PendingPermission;
    use crate::app::services::chat_service::{ChatService, PreparedChatRun, ToolCallMode};
    use crate::app::services::config_service::ConfigService;
    use crate::app::services::conversation_service::ConversationService;
    use crate::app::services::tool_service::ToolService;
    use crate::infra::conversation_store::ConversationStore;
    use serde_json::json;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};
    use tokio::sync::mpsc;

    fn make_project_root(name: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after unix epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("skills_executor_{name}_{unique}"));
        fs::create_dir_all(root.join("siliconflow/data")).expect("create siliconflow/data");
        fs::create_dir_all(root.join("siliconflow/config")).expect("create siliconflow/config");
        fs::create_dir_all(root.join("test")).expect("create test");
        fs::write(
            root.join("siliconflow/config/providers.json"),
            "{\"providers\":[]}",
        )
        .ok();
        root
    }

    fn make_chat_service(project_root: PathBuf) -> (ChatService, ConversationService) {
        let config_service = ConfigService::load(project_root.clone()).expect("load config");
        let conversation_store =
            ConversationStore::bootstrap(project_root.clone()).expect("bootstrap store");
        let conversation_service = ConversationService::new(conversation_store);
        let tool_service = ToolService::new(project_root);
        let run_registry = RunRegistry::new();
        let chat_service = ChatService::new(
            config_service.clone(),
            conversation_service.clone(),
            tool_service,
            run_registry,
            config_service.token_store().clone(),
        );
        (chat_service, conversation_service)
    }

    fn create_conversation_id(conversation_service: &ConversationService) -> String {
        conversation_service
            .create()
            .expect("create conversation")
            .id
    }

    fn sample_pending(conversation_id: &str) -> PendingPermission {
        PendingPermission {
            conversation_id: conversation_id.to_string(),
            prepared: PreparedChatRun {
                conversation_id: conversation_id.to_string(),
                model_name: "model".to_string(),
                model_id: "provider/model".to_string(),
                provider: "provider".to_string(),
                api_url: "http://localhost".to_string(),
                api_key: "test-key".to_string(),
                messages: vec![],
                route: "manual".to_string(),
                tier: "easy".to_string(),
                query: "hello".to_string(),
                plan_enabled: false,
                plan_steps: vec![],
                audit_retry_cap: 0,
                fallback_chain: vec![],
                routed_tool_names: None,
                tool_call_mode: ToolCallMode::Function,
                prompt_fallback_attempted: false,
            },
            request_messages: vec![],
            tool_calls: vec![json!({
                "id": "call_1",
                "type": "function",
                "function": {
                    "name": "run_terminal",
                    "arguments": "{\"command\":\"pwd\"}"
                }
            })],
            next_index: 0,
        }
    }

    fn drain_event(rx: &mut tokio::sync::mpsc::Receiver<serde_json::Value>) -> serde_json::Value {
        rx.try_recv().expect("expected event")
    }

    #[tokio::test]
    async fn resume_single_tool_call_emits_denied_tool_message_when_not_granted() {
        let project_root = make_project_root("denied");
        let (chat_service, conversation_service) = make_chat_service(project_root);
        let conversation_id = create_conversation_id(&conversation_service);
        let executor = ChatExecutor::new(chat_service.clone());
        let pending = sample_pending(&conversation_id);
        let tool_call = pending.tool_calls[0].clone();
        let mut request_messages = Vec::new();
        let (tx, mut rx): (EventTx, _) = mpsc::channel(8);

        let should_continue = executor
            .resume_single_tool_call(&pending, &mut request_messages, &tool_call, false, &tx)
            .await
            .expect("resume single tool call");

        assert!(!should_continue);
        assert_eq!(request_messages.len(), 1);
        assert_eq!(request_messages[0]["role"], "tool");
        assert_eq!(
            request_messages[0]["content"],
            "用户拒绝了此操作，已取消执行。"
        );
        assert!(rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn resume_single_tool_call_executes_approved_tool_and_emits_events() {
        let project_root = make_project_root("approved");
        let (chat_service, conversation_service) = make_chat_service(project_root);
        let conversation_id = create_conversation_id(&conversation_service);
        let executor = ChatExecutor::new(chat_service.clone());
        let pending = sample_pending(&conversation_id);
        let tool_call = pending.tool_calls[0].clone();
        let mut request_messages = Vec::new();
        let (tx, mut rx): (EventTx, _) = mpsc::channel(8);

        let should_continue = executor
            .resume_single_tool_call(&pending, &mut request_messages, &tool_call, true, &tx)
            .await
            .expect("resume single tool call");

        assert!(should_continue);
        assert_eq!(request_messages.len(), 1);
        assert_eq!(request_messages[0]["role"], "tool");
        assert!(request_messages[0]["content"]
            .as_str()
            .unwrap_or_default()
            .contains("\"ok\": true"));

        let start = drain_event(&mut rx);
        let done = drain_event(&mut rx);
        assert_eq!(start["type"], "tool_start");
        assert_eq!(done["type"], "tool_done");
    }
}
