use serde::{Deserialize, Serialize};
use std::borrow::Borrow;
use std::fmt;

/// 会话 ID 的强类型封装。
///
/// 原先代码里会话 ID 以裸 `String` 在 handler / service / registry 之间流转，
/// 既容易和普通字符串混淆，也无法让类型系统保证 hash/eq 语义一致。
/// 这里把它收口成 newtype，对外仍然以字符串形式对接现有 JSON / SQLite 协议。
#[derive(Debug, Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct ConvId(pub String);

impl ConvId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_string(self) -> String {
        self.0
    }
}

impl From<String> for ConvId {
    fn from(v: String) -> Self {
        Self(v)
    }
}

impl From<&str> for ConvId {
    fn from(v: &str) -> Self {
        Self(v.to_string())
    }
}

impl AsRef<str> for ConvId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl Borrow<str> for ConvId {
    fn borrow(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ConvId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// 单次会话运行的状态机。
///
/// 只是描述 RunHandle 当前所处阶段，不直接改变执行逻辑；
/// 后续 planner / tool loop 的状态机化会继续在这里扩展。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunStatus {
    Idle,
    Running,
    AwaitingPermission,
    Aborted,
    Done,
}

impl Default for RunStatus {
    fn default() -> Self {
        RunStatus::Idle
    }
}

impl RunStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            RunStatus::Idle => "idle",
            RunStatus::Running => "running",
            RunStatus::AwaitingPermission => "awaiting_permission",
            RunStatus::Aborted => "aborted",
            RunStatus::Done => "done",
        }
    }
}

/// 运行期错误的粗粒度分类。
///
/// 目前用于 failover 日志和后续的 failover policy；
/// handler 层暂时仍走 anyhow::Error，本枚举只在 registry 内部使用。
#[derive(Debug, Clone, thiserror::Error)]
pub enum RunError {
    #[error("会话已被中断")]
    Aborted,
    #[error("上游请求失败[{class}]: {message}")]
    Upstream {
        class: String,
        message: String,
        provider: String,
        model: String,
        api_url: String,
    },
    #[error("工具执行失败: {0}")]
    Tool(String),
    #[error("权限未授予: {0}")]
    PermissionDenied(String),
    #[error("运行态不一致: {0}")]
    InvalidState(String),
}
