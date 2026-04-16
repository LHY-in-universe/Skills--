//! 路由配置读写。

use crate::app::services::config_service::ConfigService;
use serde_json::Value;

impl ConfigService {
    pub fn routing_config(&self) -> anyhow::Result<Value> {
        self.read_json_file(
            "siliconflow/config/routing_config.json",
            serde_json::json!({
                "enabled": false,
                "router_model": "",
                "summary_model": "",
                "tiers": { "easy": "", "medium": "", "hard": "" }
            }),
        )
    }

    pub fn save_routing_config(&self, value: &Value) -> anyhow::Result<Value> {
        self.write_json_file("siliconflow/config/routing_config.json", value)?;
        self.routing_config()
    }
}
