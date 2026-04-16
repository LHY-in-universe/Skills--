//! `AuthProfileResolver`：把 provider 的 API Key 与默认 URL 解析抽成 trait。
//!
//! 抽象动机：`chat_service` / `chat/failover.rs` 之前都直接调用 `ConfigService::resolve_api_key`
//! 与 `default_api_url`，使聊天执行层与配置层紧耦合。这里把两个解析方法提到 trait，
//! 默认实现仍复用 `ConfigService`，后续要接入密钥管家（Vault / keychain）只需换个 impl。

use crate::app::services::config_service::ConfigService;
use std::sync::Arc;

pub trait AuthProfileResolver: Send + Sync {
    fn resolve_api_key(&self, provider_id: &str) -> Option<String>;
    fn default_api_url(&self, provider_id: Option<&str>) -> String;
}

/// 默认实现：委托给 `ConfigService`，保持与 P1 第一批之前完全一致的解析逻辑。
#[derive(Clone)]
pub struct ConfigAuthResolver {
    config_service: ConfigService,
}

impl ConfigAuthResolver {
    pub fn new(config_service: ConfigService) -> Self {
        Self { config_service }
    }

    pub fn into_arc(self) -> Arc<dyn AuthProfileResolver> {
        Arc::new(self)
    }
}

impl AuthProfileResolver for ConfigAuthResolver {
    fn resolve_api_key(&self, provider_id: &str) -> Option<String> {
        self.config_service.resolve_api_key(provider_id)
    }

    fn default_api_url(&self, provider_id: Option<&str>) -> String {
        self.config_service.default_api_url(provider_id)
    }
}
