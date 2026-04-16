use std::path::PathBuf;

/// SQLite 相关的公共路径帮助。
///
/// 先把数据库路径统一封装出来，避免后续 storage 模块各自拼接路径。
pub fn runtime_db_path(project_root: &PathBuf) -> PathBuf {
    project_root.join("siliconflow/data/runtime.db")
}
