use crate::app::run_registry::RunRegistry;
use crate::app::services::chat::{failover, planner};
use crate::app::services::config_service::ConfigService;
use crate::app::services::conversation_service::ConversationService;
use crate::app::services::tool_service::ToolService;
use crate::domain::conversation::ConversationMessage;
use crate::domain::models::ModelSpec;
use crate::infra::auth_profile::{AuthProfileResolver, ConfigAuthResolver};
use crate::infra::providers::{OpenAiCompatDriver, ProviderDriver};
use crate::infra::token_store::TokenStore;
use anyhow::{anyhow, Context};
use reqwest::Response;
use serde_json::{json, Value};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tokio::sync::OwnedMutexGuard;

// 对外继续暴露这几个结构的旧路径，避免上层代码一起改动。
pub use crate::app::services::chat::failover::ModelRuntime;
pub use crate::app::services::chat::permission::PendingPermission;
pub use crate::app::services::chat::usage::UsageSnapshot;

/// 聊天服务门面。
///
/// 这一轮重构把 ChatService 收缩成一层薄薄的协调入口：
/// - 业务流程（准备/发送/结束、工具循环的数据写入）仍然在这里
/// - planner / failover / permission / usage 等算法下沉到 `chat/*` 子模块
/// - 会话运行态（abort、串行锁、pending permission、failover log）下沉到 `RunRegistry`
///
/// 对 handler 层和 router 的 API 保持不变，只是实现依赖关系更清晰。
#[derive(Clone)]
pub struct ChatService {
    config_service: ConfigService,
    conversation_service: ConversationService,
    tool_service: ToolService,
    run_registry: RunRegistry,
    token_store: TokenStore,
    http_client: reqwest::Client,
    provider_driver: Arc<dyn ProviderDriver>,
    auth_resolver: Arc<dyn AuthProfileResolver>,
}

impl ChatService {
    pub fn new(
        config_service: ConfigService,
        conversation_service: ConversationService,
        tool_service: ToolService,
        run_registry: RunRegistry,
        token_store: TokenStore,
    ) -> Self {
        let auth_resolver: Arc<dyn AuthProfileResolver> =
            Arc::new(ConfigAuthResolver::new(config_service.clone()));
        Self {
            config_service,
            conversation_service,
            tool_service,
            run_registry,
            token_store,
            http_client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(180))
                .build()
                .expect("reqwest client 初始化失败"),
            provider_driver: Arc::new(OpenAiCompatDriver::default()),
            auth_resolver,
        }
    }

    pub fn auth_resolver(&self) -> Arc<dyn AuthProfileResolver> {
        self.auth_resolver.clone()
    }

    pub fn provider_driver(&self) -> Arc<dyn ProviderDriver> {
        self.provider_driver.clone()
    }

    pub fn token_store(&self) -> &TokenStore {
        &self.token_store
    }

    /// 准备单次聊天请求。
    ///
    /// 这里会先将用户消息写入会话，再生成上游兼容 OpenAI Chat Completions 的请求体。
    pub fn prepare_chat(
        &self,
        user_input: &str,
        conv_id: Option<&str>,
    ) -> anyhow::Result<PreparedChatRun> {
        let snapshot = self.config_service.snapshot();
        let current_model_name = snapshot.current_model_name().to_string();
        let current_model = snapshot
            .current_model()
            .ok_or_else(|| anyhow!("未配置可用模型"))?;
        let (tier, routed_model_name, routed_model) =
            self.select_model_for_query(user_input, &snapshot, &current_model_name);
        let selected_model_name = routed_model_name
            .clone()
            .unwrap_or_else(|| current_model_name.clone());
        let selected_model = routed_model.as_ref().unwrap_or(current_model);
        let api_key = self
            .auth_resolver
            .resolve_api_key(&selected_model.provider)
            .ok_or_else(|| anyhow!("当前模型缺少 API Key"))?;
        let api_url = selected_model.api_url_override.clone().unwrap_or_else(|| {
            self.auth_resolver
                .default_api_url(Some(&selected_model.provider))
        });
        let _ = self
            .auth_resolver
            .resolve_api_key(&current_model.provider)
            .ok_or_else(|| anyhow!("当前默认模型缺少 API Key"))?;

        let conversation_id = self.conversation_service.resolve_or_create_active(conv_id)?;
        self.reset_abort(&conversation_id);

        self.conversation_service.append_message(
            &conversation_id,
            &ConversationMessage {
                role: "user".to_string(),
                content: user_input.to_string(),
                extra: Value::Object(Default::default()),
            },
        )?;

        let history = self.conversation_service.history(Some(&conversation_id))?;
        let mut messages = history
            .items
            .into_iter()
            .map(|item| {
                let mut msg = json!({
                    "role": item.role,
                    "content": item.content,
                });
                if let Some(tool_call_id) = item.extra.get("tool_call_id").and_then(|v| v.as_str())
                {
                    msg["tool_call_id"] = Value::String(tool_call_id.to_string());
                }
                if let Some(tool_calls) = item.extra.get("tool_calls") {
                    msg["tool_calls"] = tool_calls.clone();
                }
                msg
            })
            .collect::<Vec<_>>();

        let plan_enabled = self.should_plan(user_input, &tier);
        let plan_steps = if plan_enabled {
            self.build_plan_steps(user_input)
        } else {
            Vec::new()
        };
        if plan_enabled && !plan_steps.is_empty() {
            messages.push(json!({
                "role": "system",
                "content": "请按以下计划执行并在答案中明确给出结果：\n".to_string()
                    + &plan_steps
                        .iter()
                        .enumerate()
                        .map(|(i, step)| format!("{}. {}", i + 1, step))
                        .collect::<Vec<_>>()
                        .join("\n")
            }));
        }

        let fallback_chain = failover::build_fallback_chain(
            self.auth_resolver.as_ref(),
            &snapshot,
            &selected_model_name,
            &current_model_name,
        );

        Ok(PreparedChatRun {
            conversation_id,
            model_name: selected_model_name,
            model_id: selected_model.id.clone(),
            provider: selected_model.provider.clone(),
            api_url,
            api_key,
            messages,
            route: if routed_model_name.is_some() {
                "router".to_string()
            } else {
                "manual".to_string()
            },
            tier,
            query: user_input.to_string(),
            plan_enabled,
            plan_steps,
            audit_retry_cap: self
                .config_service
                .runtime_settings()
                .self_correction_max_retries
                .max(0) as usize,
            fallback_chain,
        })
    }

    /// 发送真正的上游流式请求。
    ///
    /// 目前默认假设 provider 兼容 OpenAI 风格的 SSE：每条事件使用 `data: ...`
    /// 包裹，文本增量位于 `choices[0].delta.content`。
    pub async fn send_stream_request(
        &self,
        prepared: &PreparedChatRun,
        messages: &[Value],
    ) -> anyhow::Result<Response> {
        let enabled_tool_names = self
            .config_service
            .skills_view()
            .unwrap_or_default()
            .into_iter()
            .filter(|item| {
                item.get("enabled")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false)
            })
            .filter_map(|item| {
                item.get("name")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
            })
            .collect::<Vec<_>>();
        let tools = self
            .tool_service
            .tool_schemas(&enabled_tool_names)
            .unwrap_or_default();
        let payload =
            self.provider_driver
                .build_payload(&prepared.model_id, messages, &tools);
        let resp = self
            .http_client
            .post(&prepared.api_url)
            .bearer_auth(&prepared.api_key)
            .json(&payload)
            .send()
            .await
            .with_context(|| format!("请求上游模型失败: {}", prepared.api_url))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(anyhow!("上游模型返回错误 {}: {}", status, body));
        }

        Ok(resp)
    }

    /// 聊天结束后写回 assistant 消息。
    ///
    /// 即便是用户中途中断，也会把已经生成的部分结果落库，避免前端刷新后直接丢失。
    pub fn finalize_chat(
        &self,
        prepared: &PreparedChatRun,
        content: String,
        usage: UsageSnapshot,
        aborted: bool,
    ) -> anyhow::Result<()> {
        if content.trim().is_empty() && !aborted {
            return Ok(());
        }

        self.conversation_service.append_message(
            &prepared.conversation_id,
            &ConversationMessage {
                role: "assistant".to_string(),
                content,
                extra: json!({
                    "_model": prepared.model_id,
                    "_provider": prepared.provider,
                    "_aborted": aborted,
                    "_tokens": {
                        "prompt": usage.prompt_tokens,
                        "completion": usage.completion_tokens,
                        "total": usage.total_tokens
                    }
                }),
            },
        )?;
        let _ = self.token_store.insert(
            Some(&prepared.conversation_id),
            &prepared.model_id,
            usage.prompt_tokens,
            usage.completion_tokens,
            usage.total_tokens,
        );
        Ok(())
    }

    pub fn reset_abort(&self, conv_id: &str) {
        self.run_registry.reset_abort(conv_id);
    }

    /// 显式推进会话运行状态。
    pub fn set_run_status(&self, conv_id: &str, status: crate::domain::run::RunStatus) {
        self.run_registry.handle(conv_id).set_status(status);
    }

    /// 读取单会话当前状态，给观测接口用。
    pub fn run_status(&self, conv_id: &str) -> crate::domain::run::RunStatus {
        self.run_registry.status_snapshot(conv_id)
    }

    /// 列出非 Idle 会话，供 `/api/runtime-health` 展示。
    pub fn active_runs(&self) -> Vec<(String, crate::domain::run::RunStatus)> {
        self.run_registry.active_runs()
    }

    /// 中断当前会话；如果没有提供会话 ID，则中断全部正在运行的会话。
    pub fn abort(&self, conv_id: Option<&str>) {
        match conv_id {
            Some(id) => self.run_registry.abort(id),
            None => self.run_registry.abort_all(),
        }
    }

    pub fn is_aborted(&self, conv_id: &str) -> bool {
        self.run_registry.is_aborted(conv_id)
    }

    pub fn abort_flag_for(&self, conv_id: &str) -> Arc<AtomicBool> {
        self.run_registry.abort_flag_for(conv_id)
    }

    /// 获取单会话执行锁。
    ///
    /// 同一个 `conv_id` 的聊天请求必须串行执行，避免：
    /// - 两个请求同时向同一会话追加消息
    /// - 一个请求中断另一个请求却共用同一段历史
    /// - SSE 与 SQLite 写入顺序错乱
    pub async fn acquire_run_slot(&self, conv_id: &str) -> OwnedMutexGuard<()> {
        self.run_registry.acquire(conv_id).await
    }

    pub async fn execute_tool(&self, name: &str, args: &Value) -> anyhow::Result<String> {
        self.execute_tool_tracked(None, name, args).await
    }

    /// 执行工具并把一次调用记录写进 `execution_events` 表。
    pub async fn execute_tool_tracked(
        &self,
        conv_id: Option<&str>,
        name: &str,
        args: &Value,
    ) -> anyhow::Result<String> {
        let started = std::time::Instant::now();
        let result = self.tool_service.execute(name, args).await;
        let elapsed_ms = started.elapsed().as_millis() as i64;
        let (status, response_text) = match &result {
            Ok(text) => ("ok", text.clone()),
            Err(err) => ("error", err.to_string()),
        };
        let _ = self
            .config_service
            .execution_store()
            .insert(crate::infra::execution_store::ExecutionEventInput {
                conv_id,
                tool_name: name,
                status,
                request: Some(args),
                response: Some(response_text.as_str()),
                elapsed_ms: Some(elapsed_ms),
            });
        result
    }

    pub fn needs_permission(&self, name: &str) -> bool {
        self.tool_service.needs_permission(name)
    }

    /// 判断本次调用是否真的需要弹出权限确认。
    ///
    /// 工具本身是否危险由 `ToolService` 决定；
    /// 用户是否已经长期放行由 `ConfigService` 决定。
    pub fn requires_permission(&self, name: &str) -> bool {
        self.tool_service.needs_permission(name)
            && !self.config_service.is_tool_always_allowed(name)
    }

    pub fn set_pending_permission(&self, pending: PendingPermission) {
        self.run_registry.set_pending(pending);
    }

    pub fn take_pending_permission(&self, conv_id: &str) -> Option<PendingPermission> {
        self.run_registry.take_pending(conv_id)
    }

    pub fn append_assistant_tool_call_message(
        &self,
        conversation_id: &str,
        content: String,
        model_id: &str,
        tool_calls: Value,
    ) -> anyhow::Result<()> {
        self.conversation_service.append_message(
            conversation_id,
            &ConversationMessage {
                role: "assistant".to_string(),
                content,
                extra: json!({
                    "_model": model_id,
                    "tool_calls": tool_calls
                }),
            },
        )
    }

    pub fn append_tool_message(
        &self,
        conversation_id: &str,
        tool_call_id: &str,
        tool_name: &str,
        args: &Value,
        content: String,
    ) -> anyhow::Result<()> {
        self.conversation_service.append_message(
            conversation_id,
            &ConversationMessage {
                role: "tool".to_string(),
                content,
                extra: json!({
                    "tool_call_id": tool_call_id,
                    "name": tool_name,
                    "args": args
                }),
            },
        )
    }

    /// 记录最近一次 failover，供诊断中心和前端状态栏读取。
    pub fn record_failover(&self, item: Value) {
        self.run_registry.record_failover(item);
    }

    pub fn recent_failovers(&self, limit: usize) -> Vec<Value> {
        self.run_registry.recent_failovers(limit)
    }

    /// 轻量版路由决策。
    ///
    /// 当前先不迁移 Python 的本地 Ollama 分类器，只保留稳定且容易解释的规则：
    /// - 路由关闭：直接使用当前模型
    /// - 短且非技术问题：easy
    /// - 明显复杂/工程类请求：hard
    /// - 其余默认：medium
    fn select_model_for_query(
        &self,
        user_input: &str,
        snapshot: &crate::domain::models::RuntimeSnapshot,
        current_model_name: &str,
    ) -> (String, Option<String>, Option<ModelSpec>) {
        let routing = self
            .config_service
            .routing_config()
            .unwrap_or_else(|_| json!({}));
        if !routing
            .get("enabled")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
        {
            return ("".to_string(), None, None);
        }

        let tier = self.classify_tier(user_input);
        let model_ref = routing
            .get("tiers")
            .and_then(|v| v.get(&tier))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_string();
        if model_ref.is_empty() {
            return (tier, None, None);
        }
        let resolved = self.resolve_model_ref(snapshot, &model_ref);
        match resolved {
            Some((name, spec)) if name != current_model_name => (tier, Some(name), Some(spec)),
            Some(_) => (tier, None, None),
            None => (tier, None, None),
        }
    }

    fn resolve_model_ref(
        &self,
        snapshot: &crate::domain::models::RuntimeSnapshot,
        model_ref: &str,
    ) -> Option<(String, ModelSpec)> {
        if let Some(spec) = snapshot.models.get(model_ref) {
            return Some((model_ref.to_string(), spec.clone()));
        }
        snapshot
            .models
            .iter()
            .find(|(_, spec)| spec.id == model_ref)
            .map(|(name, spec)| (name.clone(), spec.clone()))
    }

    fn classify_tier(&self, user_input: &str) -> String {
        let q = user_input.trim().to_lowercase();
        let tech_keywords = [
            "代码", "算法", "设计", "实现", "分析", "证明", "调试", "函数", "架构", "系统",
            "数据库", "网络", "部署", "模拟", "计算", "程序", "项目", "写一个", "实现一个",
            "debug", "error", "bug", "api", "code", "script", "python", "java", "rust",
        ];
        let hard_keywords = [
            "重构", "架构", "系统设计", "完整项目", "规划", "plan", "implement the plan",
            "多步骤", "大改", "替代", "迁移", "agent", "planner", "failover",
        ];
        let short_non_tech =
            q.chars().count() <= 20 && !tech_keywords.iter().any(|kw| q.contains(kw));
        if short_non_tech {
            return "easy".to_string();
        }
        let hard = q.chars().count() >= 120 || hard_keywords.iter().any(|kw| q.contains(kw));
        if hard {
            return "hard".to_string();
        }
        "medium".to_string()
    }

    /// planner 启用条件，委托到子模块。
    pub fn should_plan(&self, query: &str, tier: &str) -> bool {
        planner::should_plan(&self.config_service.runtime_settings(), query, tier)
    }

    /// planner 计划步骤生成，委托到子模块。
    pub fn build_plan_steps(&self, query: &str) -> Vec<String> {
        planner::build_plan_steps(&self.config_service.runtime_settings(), query)
    }

    /// 轻量质量审计，委托到子模块。
    pub fn audit_answer(&self, query: &str, answer: &str) -> (bool, String) {
        planner::audit_answer(query, answer)
    }

    /// 构造 failover 候选链，委托到子模块。
    pub fn build_fallback_chain(
        &self,
        snapshot: &crate::domain::models::RuntimeSnapshot,
        current_selected_name: &str,
        current_default_name: &str,
    ) -> Vec<ModelRuntime> {
        failover::build_fallback_chain(
            self.auth_resolver.as_ref(),
            snapshot,
            current_selected_name,
            current_default_name,
        )
    }
}

/// 已准备好的聊天运行。
#[derive(Debug, Clone)]
pub struct PreparedChatRun {
    pub conversation_id: String,
    pub model_name: String,
    pub model_id: String,
    pub provider: String,
    pub api_url: String,
    pub api_key: String,
    pub messages: Vec<Value>,
    pub route: String,
    pub tier: String,
    pub query: String,
    pub plan_enabled: bool,
    pub plan_steps: Vec<String>,
    pub audit_retry_cap: usize,
    pub fallback_chain: Vec<ModelRuntime>,
}
