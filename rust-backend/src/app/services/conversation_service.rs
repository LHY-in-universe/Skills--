use crate::domain::conversation::{
    ConversationCreateResponse, ConversationHistoryResponse, ConversationMessage,
    ConversationSummary,
};
use crate::infra::conversation_store::ConversationStore;

/// 会话应用服务。
///
/// 这一层负责把“会话存储细节”包装成稳定的应用接口，避免路由直接碰 SQLite。
/// 后续如果引入会话 actor、运行态锁、chat run state，也会继续落在这里。
#[derive(Clone)]
pub struct ConversationService {
    store: ConversationStore,
}

impl ConversationService {
    pub fn new(store: ConversationStore) -> Self {
        Self { store }
    }

    pub fn list(&self) -> anyhow::Result<Vec<ConversationSummary>> {
        self.store.list_conversations()
    }

    pub fn create(&self) -> anyhow::Result<ConversationCreateResponse> {
        self.store.create_conversation()
    }

    pub fn activate(&self, conv_id: &str) -> anyhow::Result<()> {
        self.store.activate_conversation(conv_id)
    }

    pub fn rename(&self, conv_id: &str, name: &str) -> anyhow::Result<()> {
        self.store.rename_conversation(conv_id, name)
    }

    pub fn delete(&self, conv_id: &str) -> anyhow::Result<Vec<ConversationSummary>> {
        self.store.delete_conversation(conv_id)?;
        self.list()
    }

    pub fn history(&self, conv_id: Option<&str>) -> anyhow::Result<ConversationHistoryResponse> {
        let items: Vec<ConversationMessage> = self.store.history(conv_id)?;
        Ok(ConversationHistoryResponse { items })
    }

    pub fn resolve_or_create_active(&self, conv_id: Option<&str>) -> anyhow::Result<String> {
        self.store.resolve_or_create_active(conv_id)
    }

    pub fn append_message(&self, conv_id: &str, message: &ConversationMessage) -> anyhow::Result<()> {
        self.store.append_message(conv_id, message)
    }

    pub fn clear_history(&self, conv_id: Option<&str>) -> anyhow::Result<()> {
        self.store.clear_history(conv_id)
    }
}
