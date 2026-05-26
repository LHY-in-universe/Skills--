use serde::Serialize;

/// Doctor 单条发现项。
///
/// 这里先保留最基础字段，确保前端和未来诊断中心可以稳定读取。
/// 后续会增加：
/// - active_surface
/// - provider
/// - fixable
/// - detail / metadata
#[derive(Debug, Clone, Serialize)]
pub struct DoctorFinding {
    pub severity: &'static str,
    pub code: String,
    pub message: String,
    pub fix_hint: String,
}

impl DoctorFinding {
    pub fn critical(
        code: impl Into<String>,
        message: impl Into<String>,
        fix_hint: impl Into<String>,
    ) -> Self {
        Self {
            severity: "critical",
            code: code.into(),
            message: message.into(),
            fix_hint: fix_hint.into(),
        }
    }

    pub fn warn(
        code: impl Into<String>,
        message: impl Into<String>,
        fix_hint: impl Into<String>,
    ) -> Self {
        Self {
            severity: "warn",
            code: code.into(),
            message: message.into(),
            fix_hint: fix_hint.into(),
        }
    }
}

/// Doctor 报告。
///
/// `runtime` 直接复用运行时策略快照，方便前端先接起来；
/// 后续可拆成更细的 health sections。
#[derive(Debug, Clone, Serialize)]
pub struct DoctorReport {
    pub ok: bool,
    pub findings: Vec<DoctorFinding>,
    pub runtime: crate::domain::models::RuntimeSettings,
}
