//! `token_usage` 持久化。
//!
//! 行粒度写入 SQLite，由调用方选择是否做聚合查询。原先的 JSON
//! `siliconflow/data/token_usage.json` 仍由 Python 端维护，只在 Rust 侧提供一个
//! 与之并行、但以 SQL 为权威源的 `/api/token-usage` 入口。

use anyhow::{Context, Result};
use rusqlite::{params, Connection};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone)]
pub struct TokenStore {
    db_path: PathBuf,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct TokenTotals {
    pub calls: i64,
    pub prompt: i64,
    pub completion: i64,
    pub total: i64,
}

impl TokenStore {
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
            CREATE TABLE IF NOT EXISTS token_usage (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                conv_id TEXT,
                model TEXT NOT NULL,
                prompt INTEGER NOT NULL,
                completion INTEGER NOT NULL,
                total INTEGER NOT NULL,
                created_at TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_token_usage_conv ON token_usage(conv_id);
            CREATE INDEX IF NOT EXISTS idx_token_usage_model ON token_usage(model);
            "#,
        )?;
        Ok(())
    }

    pub fn insert(
        &self,
        conv_id: Option<&str>,
        model: &str,
        prompt: i64,
        completion: i64,
        total: i64,
    ) -> Result<()> {
        if prompt == 0 && completion == 0 && total == 0 {
            return Ok(());
        }
        let conn = self.connect()?;
        conn.execute(
            "INSERT INTO token_usage (conv_id, model, prompt, completion, total, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![conv_id, model, prompt, completion, total, now_iso()],
        )?;
        Ok(())
    }

    pub fn aggregate_total(&self) -> Result<TokenTotals> {
        let conn = self.connect()?;
        let row = conn.query_row(
            "SELECT COUNT(1), COALESCE(SUM(prompt),0), COALESCE(SUM(completion),0), COALESCE(SUM(total),0) FROM token_usage",
            [],
            |r| {
                Ok(TokenTotals {
                    calls: r.get::<_, i64>(0)?,
                    prompt: r.get::<_, i64>(1)?,
                    completion: r.get::<_, i64>(2)?,
                    total: r.get::<_, i64>(3)?,
                })
            },
        )?;
        Ok(row)
    }

    pub fn aggregate_by_model(&self) -> Result<Vec<(String, TokenTotals)>> {
        let conn = self.connect()?;
        let mut stmt = conn.prepare(
            "SELECT model, COUNT(1), COALESCE(SUM(prompt),0), COALESCE(SUM(completion),0), COALESCE(SUM(total),0) \
             FROM token_usage GROUP BY model ORDER BY model",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                TokenTotals {
                    calls: r.get::<_, i64>(1)?,
                    prompt: r.get::<_, i64>(2)?,
                    completion: r.get::<_, i64>(3)?,
                    total: r.get::<_, i64>(4)?,
                },
            ))
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
