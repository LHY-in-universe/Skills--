//! Failover 决策策略。
//!
//! 原先在 `ChatExecutor::run_loop` 里内联：
//! - 根据 `classify_upstream_error` 字符串判断可否回退
//! - 从 `prepared.fallback_chain` 按下标顺序取下一条
//!
//! 抽成 trait 后，executor 不再看 `error_class` 字面值，便于未来按 provider /
//! 业务域定制策略。

use crate::app::services::chat::failover::ModelRuntime;
use crate::domain::run::RunError;

pub trait FailoverPolicy: Send + Sync {
    /// 从候选链中挑下一条；返回的下标用于 executor 自增游标。
    fn next_candidate(
        &self,
        err: &RunError,
        chain: &[ModelRuntime],
        next_index: usize,
    ) -> Option<(usize, ModelRuntime)>;
}

/// 默认策略：按顺序回退，且仅对 `model_not_found / provider_5xx / network_timeout`
/// 触发。保留与改造前 executor 内联实现完全相同的行为。
#[derive(Debug, Clone, Default)]
pub struct DefaultFailoverPolicy;

impl FailoverPolicy for DefaultFailoverPolicy {
    fn next_candidate(
        &self,
        err: &RunError,
        chain: &[ModelRuntime],
        next_index: usize,
    ) -> Option<(usize, ModelRuntime)> {
        if next_index >= chain.len() {
            return None;
        }
        let class = match err {
            RunError::Upstream { class, .. } => class.as_str(),
            _ => return None,
        };
        if !matches!(
            class,
            "model_not_found" | "provider_5xx" | "network_timeout"
        ) {
            return None;
        }
        Some((next_index + 1, chain[next_index].clone()))
    }
}
