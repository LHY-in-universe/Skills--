use crate::app::run_registry::RunRegistry;
use crate::app::services::chat::executor::ChatExecutor;
use crate::app::services::chat_service::ChatService;
use crate::app::services::clawhub_service::ClawhubService;
use crate::app::services::config_service::ConfigService;
use crate::app::services::conversation_service::ConversationService;
use crate::app::services::tool_service::ToolService;
use crate::app::services::voice_bridge::VoiceBridge;
use crate::infra::conversation_store::ConversationStore;
use std::path::PathBuf;

/// 应用全局状态。
///
/// 设计原则：
/// - 只保存可安全共享的只读快照或受控服务对象
/// - 不在这里塞业务临时状态
/// - 会话级运行态由 `RunRegistry` 统一管理，不再散落成多张 HashMap
#[derive(Clone)]
pub struct AppState {
    pub project_root: PathBuf,
    pub config_service: ConfigService,
    pub clawhub_service: ClawhubService,
    pub conversation_service: ConversationService,
    pub tool_service: ToolService,
    pub chat_service: ChatService,
    pub chat_executor: ChatExecutor,
    pub voice_bridge: VoiceBridge,
}

impl AppState {
    pub async fn bootstrap(project_root: PathBuf) -> anyhow::Result<Self> {
        let config_service = ConfigService::load(project_root.clone())?;

        // 启动时扫描 skills/ 目录，自动同步 skill_registry.json
        if let Err(e) = config_service.scan_and_sync_skills() {
            tracing::warn!("技能扫描失败（不影响启动）: {}", e);
        }

        let conversation_store = ConversationStore::bootstrap(project_root.clone())?;
        let conversation_service = ConversationService::new(conversation_store);
        let clawhub_service = ClawhubService::new(project_root.clone(), config_service.clone());
        let tool_service = ToolService::new(project_root.clone());
        let run_registry = RunRegistry::new();
        let chat_service = ChatService::new(
            config_service.clone(),
            conversation_service.clone(),
            tool_service.clone(),
            run_registry,
            config_service.token_store().clone(),
        );
        let chat_executor = ChatExecutor::new(chat_service.clone());
        let voice_bridge = VoiceBridge::new(chat_service.clone(), chat_executor.clone());
        Ok(Self {
            project_root,
            config_service,
            clawhub_service,
            conversation_service,
            tool_service,
            chat_service,
            chat_executor,
            voice_bridge,
        })
    }
}
