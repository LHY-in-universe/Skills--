//! `execution_events` 持久化。
//!
//! 记录每次 tool 执行的开始/结束、参数、返回内容及耗时；供
//! `/api/observability/events` 读取。

use anyhow::{Context, Result};
use rusqlite::{params, Connection};
use serde_json::Value;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone)]
pub struct ExecutionStore {
    db_path: PathBuf,
}

pub struct ExecutionEventInput<'a> {
    pub conv_id: Option<&'a str>,
    pub tool_name: &'a str,
    pub status: &'a str,
    pub request: Option<&'a Value>,
    pub response: Option<&'a str>,
    pub elapsed_ms: Option<i64>,
}

impl ExecutionStore {
    pub fn bootstrap(project_root: PathBuf) -> Result<Self> {
        let db_path = crate::infra::sqlite::runtime_db_path(&project_root);
        let store = Self { db_path };
        store.init_schema()?;
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
            CREATE TABLE IF NOT EXISTS execution_events (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                conv_id TEXT,
                tool_name TEXT NOT NULL,
                status TEXT NOT NULL,
                request_json TEXT,
                response_json TEXT,
                elapsed_ms INTEGER,
                created_at TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_exec_events_conv ON execution_events(conv_id);
            CREATE INDEX IF NOT EXISTS idx_exec_events_created ON execution_events(created_at);
            "#,
        )?;
        Ok(())
    }

    pub fn insert(&self, input: ExecutionEventInput<'_>) -> Result<()> {
        let conn = self.connect()?;
        let req = input.request.map(|v| v.to_string());
        conn.execute(
            "INSERT INTO execution_events (conv_id, tool_name, status, request_json, response_json, elapsed_ms, created_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                input.conv_id,
                input.tool_name,
                input.status,
                req,
                input.response,
                input.elapsed_ms,
                now_iso(),
            ],
        )?;
        Ok(())
    }

    pub fn recent(&self, limit: usize) -> Result<Vec<Value>> {
        let conn = self.connect()?;
        let mut stmt = conn.prepare(
            "SELECT conv_id, tool_name, status, request_json, response_json, elapsed_ms, created_at \
             FROM execution_events ORDER BY id DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit as i64], |row| {
            let request_json: Option<String> = row.get(3)?;
            let response_json: Option<String> = row.get(4)?;
            Ok(serde_json::json!({
                "conv_id": row.get::<_, Option<String>>(0)?,
                "tool_name": row.get::<_, String>(1)?,
                "status": row.get::<_, String>(2)?,
                "request": request_json
                    .as_deref()
                    .and_then(|s| serde_json::from_str::<Value>(s).ok())
                    .unwrap_or(Value::Null),
                "response": response_json.unwrap_or_default(),
                "elapsed_ms": row.get::<_, Option<i64>>(5)?,
                "created_at": row.get::<_, String>(6)?,
            }))
        })?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }
}

fn now_iso() -> String {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("{}", ts)
}
