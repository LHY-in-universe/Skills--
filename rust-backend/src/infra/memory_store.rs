//! 轻量记忆存储。
//!
//! 原本 `siliconflow/data/memory.json` 是一个扁平 kv dict（`user_name` / `user_location`
//! / `last_conversation_topic` 等），Python 侧 `memory_manager.py` 直接读写该文件。
//! 本模块把它迁到 `runtime.db` 的 `memories` 表；首启自动导入一次，后续以 SQLite 为准。

use anyhow::{Context, Result};
use rusqlite::{params, Connection};
use serde_json::Value;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone)]
pub struct MemoryStore {
    db_path: PathBuf,
    legacy_path: PathBuf,
}

impl MemoryStore {
    pub fn bootstrap(project_root: PathBuf) -> Result<Self> {
        let db_path = crate::infra::sqlite::runtime_db_path(&project_root);
        let legacy_path = project_root.join("siliconflow/data/memory.json");
        let store = Self {
            db_path,
            legacy_path,
        };
        store.init_schema()?;
        store.import_legacy_if_empty()?;
        Ok(store)
    }

    fn connect(&self) -> Result<Connection> {
        Connection::open(&self.db_path)
            .with_context(|| format!("打开 SQLite 失败: {}", self.db_path.display()))
    }

    fn init_schema(&self) -> Result<()> {
        let conn = self.connect()?;
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS memories (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                conv_id TEXT,
                kind TEXT NOT NULL,
                content_json TEXT NOT NULL,
                created_at TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_memories_conv ON memories(conv_id);
            CREATE INDEX IF NOT EXISTS idx_memories_kind ON memories(kind);
            "#,
        )?;
        Ok(())
    }

    fn import_legacy_if_empty(&self) -> Result<()> {
        let conn = self.connect()?;
        let count: i64 = conn.query_row("SELECT COUNT(1) FROM memories", [], |r| r.get(0))?;
        if count > 0 || !self.legacy_path.exists() {
            return Ok(());
        }
        let text = fs::read_to_string(&self.legacy_path)
            .with_context(|| format!("读取 {} 失败", self.legacy_path.display()))?;
        let raw: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
        let Some(obj) = raw.as_object() else {
            return Ok(());
        };
        let now = now_iso();
        for (key, value) in obj {
            let payload = serde_json::json!({ "key": key, "value": value });
            conn.execute(
                "INSERT INTO memories (conv_id, kind, content_json, created_at) VALUES (NULL, 'legacy_profile', ?1, ?2)",
                params![payload.to_string(), now],
            )?;
        }
        Ok(())
    }

    pub fn insert(&self, conv_id: Option<&str>, kind: &str, content: &Value) -> Result<()> {
        let conn = self.connect()?;
        conn.execute(
            "INSERT INTO memories (conv_id, kind, content_json, created_at) VALUES (?1, ?2, ?3, ?4)",
            params![conv_id, kind, content.to_string(), now_iso()],
        )?;
        Ok(())
    }

    pub fn list_by_conv(&self, conv_id: &str, limit: usize) -> Result<Vec<Value>> {
        let conn = self.connect()?;
        let mut stmt = conn.prepare(
            "SELECT kind, content_json, created_at FROM memories WHERE conv_id = ?1 \
             ORDER BY id DESC LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![conv_id, limit as i64], |row| {
            let content_json: String = row.get(1)?;
            Ok(serde_json::json!({
                "kind": row.get::<_, String>(0)?,
                "content": serde_json::from_str::<Value>(&content_json).unwrap_or(Value::Null),
                "created_at": row.get::<_, String>(2)?,
            }))
        })?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    pub fn recent(&self, limit: usize) -> Result<Vec<Value>> {
        let conn = self.connect()?;
        let mut stmt = conn.prepare(
            "SELECT conv_id, kind, content_json, created_at FROM memories \
             ORDER BY id DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit as i64], |row| {
            let content_json: String = row.get(2)?;
            Ok(serde_json::json!({
                "conv_id": row.get::<_, Option<String>>(0)?,
                "kind": row.get::<_, String>(1)?,
                "content": serde_json::from_str::<Value>(&content_json).unwrap_or(Value::Null),
                "created_at": row.get::<_, String>(3)?,
            }))
        })?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    pub fn count(&self) -> Result<i64> {
        let conn = self.connect()?;
        let n: i64 = conn.query_row("SELECT COUNT(1) FROM memories", [], |r| r.get(0))?;
        Ok(n)
    }
}

fn now_iso() -> String {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("{ts}")
}
