use crate::app::services::chat_service::PendingPermission;
use crate::domain::run::{ConvId, RunStatus};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tokio::sync::{Mutex as AsyncMutex, OwnedMutexGuard};

/// 单次会话运行的强类型句柄。
///
/// 把原先散落在 `AppState` 上的 abort 标志、串行锁、pending permission 聚合到同一个
/// 结构里。一个 `conv_id` 在任意时刻只对应一个 `RunHandle`，resume / abort /
/// permission_required 都通过这个句柄读写。
pub struct RunHandle {
    pub conv_id: ConvId,
    pub abort: Arc<AtomicBool>,
    pub lock: Arc<AsyncMutex<()>>,
    pub pending_permission: Mutex<Option<PendingPermission>>,
    pub status: Mutex<RunStatus>,
}

impl RunHandle {
    fn new(conv_id: ConvId) -> Self {
        Self {
            conv_id,
            abort: Arc::new(AtomicBool::new(false)),
            lock: Arc::new(AsyncMutex::new(())),
            pending_permission: Mutex::new(None),
            status: Mutex::new(RunStatus::Idle),
        }
    }

    pub fn set_status(&self, new_status: RunStatus) {
        if let Ok(mut g) = self.status.lock() {
            *g = new_status;
        }
    }
}

/// 会话级运行态注册表。
///
/// 所有会话级运行态（串行锁、abort 标志、pending 权限请求）都在这里登记，
/// 全局 failover 日志也搭车放在这里，方便观测接口统一读取。
#[derive(Clone)]
pub struct RunRegistry {
    handles: Arc<Mutex<HashMap<ConvId, Arc<RunHandle>>>>,
    failover_log: Arc<Mutex<Vec<Value>>>,
}

impl RunRegistry {
    pub fn new() -> Self {
        Self {
            handles: Arc::new(Mutex::new(HashMap::new())),
            failover_log: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// 按需获取或创建会话句柄。
    pub fn handle(&self, conv_id: &str) -> Arc<RunHandle> {
        let mut guard = self.handles.lock().expect("run registry 锁已中毒");
        let key = ConvId::from(conv_id);
        guard
            .entry(key.clone())
            .or_insert_with(|| Arc::new(RunHandle::new(key)))
            .clone()
    }

    /// 获取串行执行锁，返回 OwnedMutexGuard，调用方 drop 时释放。
    pub async fn acquire(&self, conv_id: &str) -> OwnedMutexGuard<()> {
        let handle = self.handle(conv_id);
        handle.lock.clone().lock_owned().await
    }

    /// 清除 abort 标志，通常在新一轮对话开始时调用。
    pub fn reset_abort(&self, conv_id: &str) {
        self.handle(conv_id).abort.store(false, Ordering::SeqCst);
    }

    /// 标记指定会话中断。
    pub fn abort(&self, conv_id: &str) {
        self.handle(conv_id).abort.store(true, Ordering::SeqCst);
    }

    /// 标记所有已登记会话中断（用于全局 stop）。
    pub fn abort_all(&self) {
        let guard = self.handles.lock().expect("run registry 锁已中毒");
        for handle in guard.values() {
            handle.abort.store(true, Ordering::SeqCst);
        }
    }

    pub fn is_aborted(&self, conv_id: &str) -> bool {
        self.handle(conv_id).abort.load(Ordering::SeqCst)
    }

    pub fn abort_flag_for(&self, conv_id: &str) -> Arc<AtomicBool> {
        self.handle(conv_id).abort.clone()
    }

    pub fn set_pending(&self, pending: PendingPermission) {
        let handle = self.handle(&pending.conversation_id);
        if let Ok(mut slot) = handle.pending_permission.lock() {
            *slot = Some(pending);
        }
        handle.set_status(RunStatus::AwaitingPermission);
    }

    pub fn take_pending(&self, conv_id: &str) -> Option<PendingPermission> {
        let handle = self.handle(conv_id);
        let taken = handle
            .pending_permission
            .lock()
            .ok()
            .and_then(|mut g| g.take());
        if taken.is_some() {
            handle.set_status(RunStatus::Running);
        }
        taken
    }

    /// 读取单个会话的状态快照。
    pub fn status_snapshot(&self, conv_id: &str) -> RunStatus {
        let handle = self.handle(conv_id);
        handle.status.lock().map(|g| *g).unwrap_or(RunStatus::Idle)
    }

    /// 列出所有「非 Idle」会话的 `(conv_id, status)`。
    pub fn active_runs(&self) -> Vec<(String, RunStatus)> {
        let Ok(guard) = self.handles.lock() else {
            return Vec::new();
        };
        guard
            .values()
            .filter_map(|handle| {
                let status = handle.status.lock().ok().map(|g| *g)?;
                if matches!(status, RunStatus::Idle) {
                    None
                } else {
                    Some((handle.conv_id.as_str().to_string(), status))
                }
            })
            .collect()
    }

    pub fn record_failover(&self, item: Value) {
        let mut guard = self.failover_log.lock().expect("failover log 锁已中毒");
        guard.push(item);
        if guard.len() > 100 {
            let excess = guard.len() - 100;
            guard.drain(0..excess);
        }
    }

    pub fn recent_failovers(&self, limit: usize) -> Vec<Value> {
        let guard = self.failover_log.lock().expect("failover log 锁已中毒");
        let keep = limit.max(1).min(200);
        let mut out: Vec<Value> = guard.iter().rev().take(keep).cloned().collect();
        out.reverse();
        out
    }
}

impl Default for RunRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::RunRegistry;
    use crate::app::services::chat::permission::PendingPermission;
    use crate::app::services::chat_service::{PreparedChatRun, ToolCallMode};
    use crate::domain::run::RunStatus;
    use serde_json::json;

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

    #[test]
    fn pending_permission_transitions_status_between_awaiting_and_running() {
        let registry = RunRegistry::new();
        let conversation_id = "conv-test";

        assert_eq!(registry.status_snapshot(conversation_id), RunStatus::Idle);

        registry.set_pending(sample_pending(conversation_id));
        assert_eq!(
            registry.status_snapshot(conversation_id),
            RunStatus::AwaitingPermission
        );

        let taken = registry.take_pending(conversation_id);
        assert!(taken.is_some());
        assert_eq!(taken.unwrap().conversation_id, conversation_id);
        assert_eq!(registry.status_snapshot(conversation_id), RunStatus::Running);
    }

    #[test]
    fn take_pending_returns_none_for_other_conversation() {
        let registry = RunRegistry::new();
        registry.set_pending(sample_pending("conv-a"));

        assert!(registry.take_pending("conv-b").is_none());
        assert_eq!(registry.status_snapshot("conv-a"), RunStatus::AwaitingPermission);
    }
}
