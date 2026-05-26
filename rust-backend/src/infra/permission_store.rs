//! 危险工具的 `always_allow` 持久化。
//!
//! 先前落在 `siliconflow/config/permission_settings.json`，本模块把它迁到
//! `runtime.db` 的 `permission_grants` 表；JSON 仅作首启导入源。

use anyhow::{Context, Result};
use rusqlite::{params, Connection};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone)]
pub struct PermissionStore {
    db_path: PathBuf,
    legacy_path: PathBuf,
}

impl PermissionStore {
    pub fn bootstrap(project_root: PathBuf) -> Result<Self> {
        let db_path = crate::infra::sqlite::runtime_db_path(&project_root);
        let legacy_path = project_root.join("siliconflow/config/permission_settings.json");
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
            CREATE TABLE IF NOT EXISTS permission_grants (
                tool_name TEXT PRIMARY KEY,
                always_allow INTEGER NOT NULL,
                source TEXT,
                updated_at TEXT NOT NULL
            );
            "#,
        )?;
        Ok(())
    }

    fn import_legacy_if_empty(&self) -> Result<()> {
        let conn = self.connect()?;
        let count: i64 =
            conn.query_row("SELECT COUNT(1) FROM permission_grants", [], |r| r.get(0))?;
        if count > 0 || !self.legacy_path.exists() {
            return Ok(());
        }
        let text = fs::read_to_string(&self.legacy_path)
            .with_context(|| format!("读取 {} 失败", self.legacy_path.display()))?;
        let raw: serde_json::Value = serde_json::from_str(&text).unwrap_or_default();
        let arr = raw.get("always_allow_tools").and_then(|v| v.as_array());
        let Some(arr) = arr else {
            return Ok(());
        };
        let now = now_iso();
        for item in arr {
            if let Some(name) = item.as_str() {
                conn.execute(
                    "INSERT OR REPLACE INTO permission_grants (tool_name, always_allow, source, updated_at) VALUES (?1, 1, 'legacy_json', ?2)",
                    params![name, now],
                )?;
            }
        }
        Ok(())
    }

    pub fn is_always_allowed(&self, tool_name: &str) -> bool {
        let Ok(conn) = self.connect() else {
            return false;
        };
        conn.query_row(
            "SELECT always_allow FROM permission_grants WHERE tool_name = ?1",
            params![tool_name],
            |row| row.get::<_, i64>(0),
        )
        .map(|v| v == 1)
        .unwrap_or(false)
    }

    pub fn upsert(&self, tool_name: &str, always_allow: bool, source: &str) -> Result<()> {
        let conn = self.connect()?;
        conn.execute(
            "INSERT INTO permission_grants (tool_name, always_allow, source, updated_at) VALUES (?1, ?2, ?3, ?4) \
             ON CONFLICT(tool_name) DO UPDATE SET always_allow = excluded.always_allow, source = excluded.source, updated_at = excluded.updated_at",
            params![tool_name, if always_allow { 1i64 } else { 0i64 }, source, now_iso()],
        )?;
        Ok(())
    }

    pub fn list_always_allowed(&self) -> Result<Vec<String>> {
        let conn = self.connect()?;
        let mut stmt = conn.prepare(
            "SELECT tool_name FROM permission_grants WHERE always_allow = 1 ORDER BY tool_name",
        )?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
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
