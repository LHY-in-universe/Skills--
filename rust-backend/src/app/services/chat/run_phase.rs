//! 执行器事件状态机。
//!
//! `ChatExecutor::run_loop` 之前把 `tx.send(serde_json!(...))` 散落在十余处内联，
//! plan / step / audit / retry / failover / tool / permission 的 payload 字段都
//! 是在各处手写。本模块把每一种「可对外 SSE 的状态点」收敛成 `RunPhase` 枚举，
//! `to_event()` 一处构造 payload，执行器只负责 transition。
//!
//! **强约束**：枚举变体产出的 JSON 字段必须与 P1 第二批改造前逐字段一致（`type`
//! / 字段名 / 顺序对前端无影响，但字段集合必须等价）。这样前端零感知。

use super::executor::EventTx;
use serde_json::{json, Value};

pub enum RunPhase {
    /// 流式上游返回的内容增量。
    Text { content: String },
    /// 单轮结束后上报的 token 用量。
    Usage {
        prompt: i64,
        completion: i64,
        total: i64,
        call_type: &'static str,
        model: String,
        provider: String,
    },
    /// 工具调用即将执行。
    ToolStart { name: String },
    /// 工具调用执行完毕。
    ToolDone { name: String },
    /// 危险工具需要用户审批，执行挂起。
    PermissionRequired {
        tool_name: String,
        args: Value,
        description: String,
    },
    /// 模型级自动回退一步。
    FailoverStep {
        from_model: String,
        to_model: String,
        provider: String,
        reason: String,
        error_class: &'static str,
    },
    /// 回退链耗尽。
    FailoverExhausted {
        model: String,
        reason: String,
        error_class: &'static str,
    },
    /// Plan 步骤完成。
    StepDone { step: String },
    /// 回答审查结果。
    Audit {
        ok: bool,
        reason: String,
        retry_count: usize,
    },
    /// 用户主动中断。
    Aborted,
    /// 本轮正常完成。
    Done {
        model_id: String,
        tier: String,
        route: String,
    },
    /// 执行层错误事件。
    Error {
        content: String,
        error_class: &'static str,
    },
}

impl RunPhase {
    pub fn to_event(self) -> Value {
        match self {
            RunPhase::Text { content } => json!({ "type": "text", "content": content }),
            RunPhase::Usage {
                prompt,
                completion,
                total,
                call_type,
                model,
                provider,
            } => json!({
                "type": "usage",
                "prompt": prompt,
                "completion": completion,
                "total": total,
                "call_type": call_type,
                "model": model,
                "provider": provider
            }),
            RunPhase::ToolStart { name } => json!({ "type": "tool_start", "name": name }),
            RunPhase::ToolDone { name } => json!({ "type": "tool_done", "name": name }),
            RunPhase::PermissionRequired {
                tool_name,
                args,
                description,
            } => json!({
                "type": "permission_required",
                "tool_name": tool_name,
                "args": args,
                "description": description
            }),
            RunPhase::FailoverStep {
                from_model,
                to_model,
                provider,
                reason,
                error_class,
            } => json!({
                "type": "failover_step",
                "failover_type": "model_fallback",
                "from_model": from_model,
                "to_model": to_model,
                "provider": provider,
                "reason": reason,
                "error_class": error_class
            }),
            RunPhase::FailoverExhausted {
                model,
                reason,
                error_class,
            } => json!({
                "type": "failover_exhausted",
                "failover_type": "model_fallback",
                "model": model,
                "reason": reason,
                "error_class": error_class
            }),
            RunPhase::StepDone { step } => json!({ "type": "step_done", "step": step }),
            RunPhase::Audit {
                ok,
                reason,
                retry_count,
            } => json!({
                "type": "audit",
                "ok": ok,
                "reason": reason,
                "retry_count": retry_count
            }),
            RunPhase::Aborted => json!({ "type": "aborted" }),
            RunPhase::Done {
                model_id,
                tier,
                route,
            } => json!({
                "type": "done",
                "_model": model_id,
                "_tier": tier,
                "_route": route
            }),
            RunPhase::Error {
                content,
                error_class,
            } => json!({
                "type": "error",
                "content": content,
                "error_class": error_class
            }),
        }
    }
}

/// 把一个 `RunPhase` 转成 JSON 并推入事件 sink。发送失败（下游已关闭）不报错，
/// 与原执行器内联的 `let _ = tx.send(...).await` 语义一致。
pub async fn emit(tx: &EventTx, phase: RunPhase) {
    let _ = tx.send(phase.to_event()).await;
}

#[cfg(test)]
mod tests {
    use super::RunPhase;
    use serde_json::json;

    #[test]
    fn permission_required_event_contains_expected_fields() {
        let event = RunPhase::PermissionRequired {
            tool_name: "run_terminal".to_string(),
            args: json!({ "command": "pwd" }),
            description: "工具需要用户审批".to_string(),
        }
        .to_event();

        assert_eq!(event["type"], "permission_required");
        assert_eq!(event["tool_name"], "run_terminal");
        assert_eq!(event["args"], json!({ "command": "pwd" }));
        assert_eq!(event["description"], "工具需要用户审批");
    }
}
