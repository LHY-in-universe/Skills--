use crate::domain::conversation::{
    ConversationCreateResponse, ConversationMessage, ConversationSummary,
};
use anyhow::{anyhow, Context};
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::Value;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

/// SQLite 会话存储。
///
/// 第一阶段目标：
/// - 建库
/// - 从旧 `conversations.json` 自动导入
/// - 提供 conversations / history 的最小可用读写接口
///
/// 这里暂时采用“每次操作新开连接”的方式，优先保证实现简单、线程安全且易于演进。
#[derive(Clone)]
pub struct ConversationStore {
    db_path: PathBuf,
    legacy_path: PathBuf,
}

impl ConversationStore {
    pub fn bootstrap(project_root: PathBuf) -> anyhow::Result<Self> {
        let db_path = project_root.join("siliconflow/data/runtime.db");
        let legacy_path = project_root.join("siliconflow/data/conversations.json");
        let store = Self { db_path, legacy_path };
        store.init_schema()?;
        store.import_legacy_if_needed()?;
        Ok(store)
    }

    fn connect(&self) -> anyhow::Result<Connection> {
        let conn = Connection::open(&self.db_path)
            .with_context(|| format!("打开 SQLite 失败: {}", self.db_path.display()))?;
        Ok(conn)
    }

    fn init_schema(&self) -> anyhow::Result<()> {
        let conn = self.connect()?;
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS conversations (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                created_at REAL NOT NULL,
                model TEXT NOT NULL,
                is_active INTEGER NOT NULL DEFAULT 0
            );

            CREATE TABLE IF NOT EXISTS conversation_messages (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                conversation_id TEXT NOT NULL,
                seq INTEGER NOT NULL,
                role TEXT NOT NULL,
                content TEXT NOT NULL DEFAULT '',
                extra_json TEXT NOT NULL DEFAULT '{}',
                FOREIGN KEY(conversation_id) REFERENCES conversations(id) ON DELETE CASCADE
            );

            CREATE INDEX IF NOT EXISTS idx_conversation_messages_conv_seq
            ON conversation_messages(conversation_id, seq);
            "#,
        )?;
        Ok(())
    }

    fn import_legacy_if_needed(&self) -> anyhow::Result<()> {
        let conn = self.connect()?;
        let existing_count: i64 = conn.query_row(
            "SELECT COUNT(1) FROM conversations",
            [],
            |row| row.get(0),
        )?;
        if existing_count > 0 || !self.legacy_path.exists() {
            return Ok(());
        }

        let text = fs::read_to_string(&self.legacy_path)
            .with_context(|| format!("读取旧会话文件失败: {}", self.legacy_path.display()))?;
        let raw: Value = serde_json::from_str(&text)?;
        let active_id = raw.get("active_id").and_then(|v| v.as_str()).unwrap_or_default().to_string();
        let map = raw
            .get("conversations")
            .and_then(|v| v.as_object())
            .ok_or_else(|| anyhow!("旧会话文件格式无效：缺少 conversations 对象"))?;

        for (conv_id, item) in map {
            let name = item.get("name").and_then(|v| v.as_str()).unwrap_or("新对话");
            let created_at = item.get("created_at").and_then(|v| v.as_f64()).unwrap_or_else(now_ts);
            let model = item.get("model").and_then(|v| v.as_str()).unwrap_or_default();
            let is_active = if conv_id == &active_id { 1 } else { 0 };

            conn.execute(
                "INSERT INTO conversations (id, name, created_at, model, is_active) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![conv_id, name, created_at, model, is_active],
            )?;

            if let Some(messages) = item.get("messages").and_then(|v| v.as_array()) {
                for (idx, msg) in messages.iter().enumerate() {
                    let role = msg.get("role").and_then(|v| v.as_str()).unwrap_or_default();
                    let content = msg.get("content").and_then(|v| v.as_str()).unwrap_or_default();
                    let mut cloned = msg.clone();
                    if let Some(obj) = cloned.as_object_mut() {
                        obj.remove("role");
                        obj.remove("content");
                    }
                    conn.execute(
                        "INSERT INTO conversation_messages (conversation_id, seq, role, content, extra_json) VALUES (?1, ?2, ?3, ?4, ?5)",
                        params![conv_id, idx as i64, role, content, serde_json::to_string(&cloned)?],
                    )?;
                }
            }
        }

        if active_id.is_empty() {
            conn.execute(
                "UPDATE conversations SET is_active = 1 WHERE id = (SELECT id FROM conversations ORDER BY created_at DESC LIMIT 1)",
                [],
            )?;
        }

        Ok(())
    }

    pub fn list_conversations(&self) -> anyhow::Result<Vec<ConversationSummary>> {
        let conn = self.connect()?;
        let mut stmt = conn.prepare(
            r#"
            SELECT
                c.id,
                c.name,
                c.created_at,
                c.model,
                c.is_active,
                COUNT(m.id) AS message_count
            FROM conversations c
            LEFT JOIN conversation_messages m ON m.conversation_id = c.id AND m.role != 'system'
            GROUP BY c.id, c.name, c.created_at, c.model, c.is_active
            ORDER BY c.is_active DESC, c.created_at DESC
            "#,
        )?;

        let rows = stmt.query_map([], |row| {
            Ok(ConversationSummary {
                id: row.get(0)?,
                name: row.get(1)?,
                created_at: row.get(2)?,
                model: row.get(3)?,
                active: row.get::<_, i64>(4)? != 0,
                message_count: row.get::<_, i64>(5)? as usize,
            })
        })?;

        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    pub fn create_conversation(&self) -> anyhow::Result<ConversationCreateResponse> {
        let conn = self.connect()?;
        let id = generate_short_id();
        let name = "新对话".to_string();
        let created_at = now_ts();
        let model = current_default_model();

        conn.execute("UPDATE conversations SET is_active = 0", [])?;
        conn.execute(
            "INSERT INTO conversations (id, name, created_at, model, is_active) VALUES (?1, ?2, ?3, ?4, 1)",
            params![id, name, created_at, model],
        )?;

        Ok(ConversationCreateResponse {
            id,
            name,
            created_at,
            model,
            active: true,
            message_count: 0,
        })
    }

    pub fn activate_conversation(&self, conv_id: &str) -> anyhow::Result<()> {
        let conn = self.connect()?;
        let exists: Option<String> = conn
            .query_row(
                "SELECT id FROM conversations WHERE id = ?1",
                params![conv_id],
                |row| row.get(0),
            )
            .optional()?;
        if exists.is_none() {
            return Err(anyhow!("conversation_not_found"));
        }
        conn.execute("UPDATE conversations SET is_active = 0", [])?;
        conn.execute(
            "UPDATE conversations SET is_active = 1 WHERE id = ?1",
            params![conv_id],
        )?;
        Ok(())
    }

    pub fn rename_conversation(&self, conv_id: &str, name: &str) -> anyhow::Result<()> {
        let conn = self.connect()?;
        let changed = conn.execute(
            "UPDATE conversations SET name = ?2 WHERE id = ?1",
            params![conv_id, name],
        )?;
        if changed == 0 {
            return Err(anyhow!("conversation_not_found"));
        }
        Ok(())
    }

    pub fn delete_conversation(&self, conv_id: &str) -> anyhow::Result<()> {
        let conn = self.connect()?;
        let was_active: Option<i64> = conn
            .query_row(
                "SELECT is_active FROM conversations WHERE id = ?1",
                params![conv_id],
                |row| row.get(0),
            )
            .optional()?;
        if was_active.is_none() {
            return Err(anyhow!("conversation_not_found"));
        }

        conn.execute(
            "DELETE FROM conversation_messages WHERE conversation_id = ?1",
            params![conv_id],
        )?;
        conn.execute("DELETE FROM conversations WHERE id = ?1", params![conv_id])?;

        if was_active == Some(1) {
            conn.execute(
                "UPDATE conversations SET is_active = 1 WHERE id = (SELECT id FROM conversations ORDER BY created_at DESC LIMIT 1)",
                [],
            )?;
        }

        Ok(())
    }

    pub fn history(&self, conv_id: Option<&str>) -> anyhow::Result<Vec<ConversationMessage>> {
        let conn = self.connect()?;
        let target_id = if let Some(id) = conv_id {
            id.to_string()
        } else {
            conn.query_row(
                "SELECT id FROM conversations WHERE is_active = 1 LIMIT 1",
                [],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .unwrap_or_default()
        };

        if target_id.is_empty() {
            return Ok(Vec::new());
        }

        let mut stmt = conn.prepare(
            r#"
            SELECT role, content, extra_json
            FROM conversation_messages
            WHERE conversation_id = ?1
            ORDER BY seq ASC
            "#,
        )?;
        let rows = stmt.query_map(params![target_id], |row| {
            let extra_json: String = row.get(2)?;
            let extra = serde_json::from_str(&extra_json).unwrap_or(Value::Object(Default::default()));
            Ok(ConversationMessage {
                role: row.get(0)?,
                content: row.get(1)?,
                extra,
            })
        })?;

        let mut out = Vec::new();
        for row in rows {
            let msg = row?;
            if msg.role != "system" {
                out.push(msg);
            }
        }
        Ok(out)
    }

    pub fn resolve_or_create_active(&self, conv_id: Option<&str>) -> anyhow::Result<String> {
        let conn = self.connect()?;
        if let Some(id) = conv_id {
            let exists: Option<String> = conn
                .query_row(
                    "SELECT id FROM conversations WHERE id = ?1",
                    params![id],
                    |row| row.get(0),
                )
                .optional()?;
            if exists.is_some() {
                return Ok(id.to_string());
            }
            return Err(anyhow!("conversation_not_found"));
        }

        let active: Option<String> = conn
            .query_row(
                "SELECT id FROM conversations WHERE is_active = 1 LIMIT 1",
                [],
                |row| row.get(0),
            )
            .optional()?;

        if let Some(id) = active {
            return Ok(id);
        }

        let created = self.create_conversation()?;
        Ok(created.id)
    }

    pub fn append_message(&self, conv_id: &str, message: &ConversationMessage) -> anyhow::Result<()> {
        let conn = self.connect()?;
        let seq: i64 = conn.query_row(
            "SELECT COALESCE(MAX(seq), -1) + 1 FROM conversation_messages WHERE conversation_id = ?1",
            params![conv_id],
            |row| row.get(0),
        )?;
        conn.execute(
            "INSERT INTO conversation_messages (conversation_id, seq, role, content, extra_json) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                conv_id,
                seq,
                message.role,
                message.content,
                serde_json::to_string(&message.extra)?
            ],
        )?;
        Ok(())
    }

    pub fn clear_history(&self, conv_id: Option<&str>) -> anyhow::Result<()> {
        let conn = self.connect()?;
        let target_id = self.resolve_or_create_active(conv_id)?;
        conn.execute(
            "DELETE FROM conversation_messages WHERE conversation_id = ?1",
            params![target_id],
        )?;
        Ok(())
    }
}

fn now_ts() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}

fn generate_short_id() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let hex = format!("{:x}", nanos);
    hex.chars().rev().take(8).collect::<String>().chars().rev().collect()
}

fn current_default_model() -> String {
    "DeepSeek-Chat".to_string()
}
