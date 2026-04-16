use anyhow::{anyhow, Context};
use serde_json::Value;
use shlex::split as shlex_split;
use std::path::PathBuf;
use std::process::Stdio;
use tokio::process::Command;
use tokio::time::{timeout, Duration};

/// 工具执行服务。
///
/// 当前策略：
/// - 继续复用现有 Python skill 脚本，避免在 Rust 重构阶段重复实现工具细节
/// - 先覆盖安全/只读类工具，危险工具统一返回受限信息
/// - tool-calls 状态机由聊天服务驱动，这里只负责“执行单个工具”
#[derive(Clone)]
pub struct ToolService {
    project_root: PathBuf,
    skills_root: PathBuf,
    registry_path: PathBuf,
}

impl ToolService {
    pub fn new(project_root: PathBuf) -> Self {
        Self {
            skills_root: project_root.join("skills"),
            registry_path: project_root.join("siliconflow/data/skill_registry.json"),
            project_root,
        }
    }

    /// 判断工具是否属于当前 Rust 后端允许自动执行的集合。
    ///
    /// 对危险工具先不直接执行，避免在 resume/审批状态机未落地时把风险扩散。
    pub fn is_auto_allowed(&self, name: &str) -> bool {
        matches!(
            name,
            "get_current_time"
                | "get_weather"
                | "get_system_info"
                | "monte_carlo_integration"
                | "summary_rules"
        )
    }

    /// 判断工具是否需要用户审批。
    ///
    /// 当前策略很保守：
    /// - 会改文件、跑命令、装包、读图等高风险能力先全部走审批
    pub fn needs_permission(&self, name: &str) -> bool {
        matches!(
            name,
            "run_terminal" | "file_editor" | "write_python" | "pip_venv" | "vision_analyze"
        )
    }

    pub async fn execute(&self, name: &str, args: &Value) -> anyhow::Result<String> {
        match name {
            "get_current_time" => self.run_simple_script("clock/scripts/get_time.py").await,
            "get_system_info" => self.run_simple_script("system_monitor/scripts/get_sys_info.py").await,
            "get_weather" => {
                let city = args.get("city").and_then(|v| v.as_str()).unwrap_or("上海");
                self.run_script_args("weather/scripts/get_weather.py", &[city.to_string()])
                    .await
            }
            "summary_rules" => self
                .run_script_args(
                    "summary_rules/scripts/manage_rules.py",
                    &[format!("--args={}", serde_json::to_string(args)?)]
                )
                .await,
            "monte_carlo_integration" => {
                let mut cli_args = Vec::new();
                if let Some(method) = args.get("method").and_then(|v| v.as_str()) {
                    cli_args.push("--method".to_string());
                    cli_args.push(method.to_string());
                }
                if let Some(func) = args.get("func").and_then(|v| v.as_str()) {
                    cli_args.push("--func".to_string());
                    cli_args.push(func.to_string());
                }
                if let Some(n) = args.get("n").and_then(|v| v.as_i64()) {
                    cli_args.push("--n".to_string());
                    cli_args.push(n.to_string());
                }
                if let Some(seed) = args.get("seed").and_then(|v| v.as_i64()) {
                    cli_args.push("--seed".to_string());
                    cli_args.push(seed.to_string());
                }
                self.run_script_args("monte_carlo/scripts/monte_carlo.py", &cli_args).await
            }
            "file_editor" => {
                let mut cli_args = Vec::new();
                if let Some(op) = args.get("op").and_then(|v| v.as_str()) {
                    cli_args.push("--op".to_string());
                    cli_args.push(op.to_string());
                }
                if let Some(folder) = args.get("folder").and_then(|v| v.as_str()) {
                    cli_args.push("--folder".to_string());
                    cli_args.push(folder.to_string());
                }
                if let Some(file) = args.get("file").and_then(|v| v.as_str()) {
                    cli_args.push("--file".to_string());
                    cli_args.push(file.to_string());
                }
                if let Some(content) = args.get("content").and_then(|v| v.as_str()) {
                    cli_args.push("--content".to_string());
                    cli_args.push(content.to_string());
                }
                if let Some(old) = args.get("old").and_then(|v| v.as_str()) {
                    cli_args.push("--old".to_string());
                    cli_args.push(old.to_string());
                }
                if let Some(newv) = args.get("new").and_then(|v| v.as_str()) {
                    cli_args.push("--new".to_string());
                    cli_args.push(newv.to_string());
                }
                self.run_script_args("file_editor/scripts/edit_file.py", &cli_args).await
            }
            "write_python" => {
                let folder = args.get("folder").and_then(|v| v.as_str()).unwrap_or_default();
                let file = args.get("file").and_then(|v| v.as_str()).unwrap_or_default();
                let content = args.get("content").and_then(|v| v.as_str()).unwrap_or_default();
                self.run_script_args(
                    "python_writer/scripts/write_python.py",
                    &[
                        format!("--folder={folder}"),
                        format!("--file={file}"),
                        format!("--content={content}"),
                    ],
                )
                .await
            }
            "run_terminal" => {
                let command = args
                    .get("command")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow!("run_terminal 缺少 command 参数"))?;
                self.execute_terminal_command(command).await
            }
            _ => Err(anyhow!("unknown_tool")),
        }
    }

    /// 返回可注入到 OpenAI 兼容请求中的工具 schema。
    ///
    /// 当前只暴露“自动安全执行”的工具，避免模型先调用了一个 Rust 后端暂时不能恢复审批的工具。
    pub fn tool_schemas(&self, enabled_names: &[String]) -> anyhow::Result<Vec<Value>> {
        if !self.registry_path.exists() {
            return Ok(Vec::new());
        }
        let text = std::fs::read_to_string(&self.registry_path)
            .with_context(|| format!("读取技能注册表失败: {}", self.registry_path.display()))?;
        let raw: Value = serde_json::from_str(&text)?;
        let items = raw.as_array().cloned().unwrap_or_default();
        let mut out = Vec::new();
        for item in items {
            let Some(name) = item.get("tool_name").and_then(|v| v.as_str()) else {
                continue;
            };
            if !enabled_names.iter().any(|n| n == name) {
                continue;
            }
            out.push(serde_json::json!({
                "type": "function",
                "function": {
                    "name": name,
                    "description": item.get("description").cloned().unwrap_or(Value::String(String::new())),
                    "parameters": item.get("parameters").cloned().unwrap_or_else(|| serde_json::json!({
                        "type": "object",
                        "properties": {}
                    }))
                }
            }));
        }
        Ok(out)
    }

    async fn run_simple_script(&self, relative: &str) -> anyhow::Result<String> {
        self.run_script_args(relative, &[]).await
    }

    async fn run_script_args(&self, relative: &str, args: &[String]) -> anyhow::Result<String> {
        let script_path = self.skills_root.join(relative);
        let output = Command::new("python3")
            .arg(&script_path)
            .args(args)
            .current_dir(&self.project_root)
            .output()
            .await
            .with_context(|| format!("执行工具脚本失败: {}", script_path.display()))?;

        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        if !stdout.is_empty() {
            return Ok(stdout);
        }
        if !stderr.is_empty() {
            return Ok(stderr);
        }
        Ok(String::new())
    }

    /// Rust 原生终端命令执行器。
    ///
    /// 这不是一个通用 shell，而是一个受限命令执行器：
    /// - 固定工作目录
    /// - 命令白名单
    /// - 禁止管道、重定向、命令替换和路径穿越
    /// - 最长执行 15 秒
    async fn execute_terminal_command(&self, raw_command: &str) -> anyhow::Result<String> {
        const BLOCKED_PATTERNS: [&str; 8] = ["$(", "`", "&&", "||", ";", ">", "<", "|"];
        const ALLOWED_COMMANDS: [&str; 18] = [
            "ls", "ll", "cat", "head", "tail", "wc", "grep", "find", "echo", "printf", "pwd",
            "mkdir", "touch", "cp", "mv", "rm", "sort", "uniq",
        ];

        let raw = raw_command.trim();
        if raw.is_empty() {
            return Err(anyhow!("命令为空"));
        }
        for pattern in BLOCKED_PATTERNS {
            if raw.contains(pattern) {
                return Err(anyhow!("命令包含禁止字符或操作: '{pattern}'"));
            }
        }
        if raw.contains("../") {
            return Err(anyhow!("命令包含禁止路径穿越: '../'"));
        }

        let mut parts = shlex_split(raw).ok_or_else(|| anyhow!("命令解析失败"))?;
        if parts.is_empty() {
            return Err(anyhow!("命令为空"));
        }
        if parts[0] == "ll" {
            parts.remove(0);
            parts.insert(0, "-la".to_string());
            parts.insert(0, "ls".to_string());
        }

        let base_cmd = parts[0].as_str();
        if !ALLOWED_COMMANDS.contains(&base_cmd) && base_cmd != "python3" {
            return Err(anyhow!(
                "命令 '{}' 不在允许列表中。允许命令: {}",
                base_cmd,
                ALLOWED_COMMANDS.join(", ")
            ));
        }
        if base_cmd == "python3" {
            for arg in parts.iter().skip(1) {
                if arg.starts_with('-') {
                    return Err(anyhow!("python3 不允许使用 '{}' 参数，只能直接运行 .py 文件", arg));
                }
            }
            if parts.get(1).map(|s| s.ends_with(".py")) != Some(true) {
                return Err(anyhow!("python3 只能运行 .py 文件，如: python3 script.py"));
            }
        }

        let sandbox_dir = self.terminal_sandbox_dir()?;
        for arg in parts.iter().skip(1) {
            let candidate = std::path::Path::new(arg);
            if arg.contains("../") {
                return Err(anyhow!("参数 '{}' 包含路径穿越", arg));
            }
            if candidate.is_absolute() {
                let resolved = candidate
                    .canonicalize()
                    .with_context(|| format!("无法解析绝对路径参数: {}", candidate.display()))?;
                if !resolved.starts_with(&sandbox_dir) {
                    return Err(anyhow!(
                        "绝对路径 '{}' 超出沙箱范围 ({})",
                        candidate.display(),
                        sandbox_dir.display()
                    ));
                }
            }
        }

        let mut cmd = Command::new(base_cmd);
        cmd.kill_on_drop(true);
        cmd.args(parts.iter().skip(1));
        cmd.current_dir(&sandbox_dir);
        cmd.stdin(Stdio::null());
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        let child = cmd
            .spawn()
            .with_context(|| format!("启动受限命令失败: {}", raw))?;
        let output = timeout(Duration::from_secs(15), child.wait_with_output())
            .await
            .map_err(|_| anyhow!("命令执行超时（限制 15 秒）"))??;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        Ok(serde_json::to_string_pretty(&serde_json::json!({
            "ok": output.status.success(),
            "command": raw,
            "cwd": sandbox_dir.to_string_lossy().to_string(),
            "stdout": stdout,
            "stderr": stderr,
            "returncode": output.status.code().unwrap_or(-1)
        }))?)
    }

    /// 解析终端工具的工作目录。
    ///
    /// 优先使用前端“终端目录”设置；如果用户还没配置，则退回到项目内 `test/`
    /// 目录，和旧 Python 终端技能的默认行为保持一致。
    fn terminal_sandbox_dir(&self) -> anyhow::Result<PathBuf> {
        let default_dir = self.project_root.join("test");
        let config_path = self.project_root.join("siliconflow/config/terminal.json");
        if !config_path.exists() {
            return Ok(default_dir);
        }
        let text = std::fs::read_to_string(&config_path)
            .with_context(|| format!("读取终端配置失败: {}", config_path.display()))?;
        let value: Value = serde_json::from_str(&text)?;
        let cwd = value
            .get("cwd")
            .and_then(|v| v.as_str())
            .filter(|v| !v.trim().is_empty())
            .map(PathBuf::from)
            .unwrap_or(default_dir);
        let resolved = cwd.canonicalize().unwrap_or(cwd);
        if !resolved.is_dir() {
            return Err(anyhow!("终端工作目录不存在: {}", resolved.display()));
        }
        Ok(resolved)
    }
}
