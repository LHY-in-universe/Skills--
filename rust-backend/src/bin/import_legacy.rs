//! 旧 Python 持久化数据一次性导入工具。
//!
//! 用法：`cargo run --bin import_legacy --release`
//!
//! 本工具只负责幂等导入 + 行数报告：
//! - `memory.json` → `memories` 表（首启未入库时自动导入，这里做补漏/校验）
//! - `permission_settings.json` / `token_usage.json` 已在首启自动导入，这里只做校验报告
//!
//! 不会删除 JSON 源文件；回滚路径：备份 `runtime.db` 后恢复即可。

use anyhow::Result;
use rusqlite::Connection;
use skills_rust_backend::infra::memory_store::MemoryStore;
use skills_rust_backend::infra::permission_store::PermissionStore;
use skills_rust_backend::infra::sqlite::runtime_db_path;
use skills_rust_backend::infra::token_store::TokenStore;
use std::path::PathBuf;

fn main() -> Result<()> {
    let project_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("rust-backend 必须位于仓库根目录下一层")
        .to_path_buf();

    println!("[import_legacy] project_root = {}", project_root.display());
    println!(
        "[import_legacy] runtime.db = {}",
        runtime_db_path(&project_root).display()
    );

    let _permissions = PermissionStore::bootstrap(project_root.clone())?;
    let _tokens = TokenStore::bootstrap(project_root.clone())?;
    let memories = MemoryStore::bootstrap(project_root.clone())?;

    let conn = Connection::open(runtime_db_path(&project_root))?;
    let perm_count: i64 =
        conn.query_row("SELECT COUNT(1) FROM permission_grants", [], |r| r.get(0))?;
    let token_rows: i64 = conn.query_row("SELECT COUNT(1) FROM token_usage", [], |r| r.get(0))?;
    let exec_rows: i64 =
        conn.query_row("SELECT COUNT(1) FROM execution_events", [], |r| r.get(0))?;
    let mem_rows = memories.count()?;

    println!("[import_legacy] counts after bootstrap:");
    println!("  permission_grants  = {}", perm_count);
    println!("  token_usage        = {}", token_rows);
    println!("  execution_events   = {}", exec_rows);
    println!("  memories           = {}", mem_rows);
    println!("[import_legacy] done; re-running is idempotent.");
    Ok(())
}
