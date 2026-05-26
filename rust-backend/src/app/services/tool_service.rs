use crate::infra::memory_store::MemoryStore;
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
    memory_path: PathBuf,
    memory_store: MemoryStore,
}

impl ToolService {
    pub fn new(project_root: PathBuf) -> Self {
        let memory_store =
            MemoryStore::bootstrap(project_root.clone()).expect("memory store 初始化失败");
        Self {
            skills_root: project_root.join("skills"),
            registry_path: project_root.join("siliconflow/data/skill_registry.json"),
            memory_path: project_root.join("siliconflow/data/memory.json"),
            memory_store,
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
        if matches!(
            name,
            "run_terminal" | "file_editor" | "write_python" | "pip_venv" | "vision_analyze"
        ) {
            return true;
        }
        if self.is_auto_allowed(name) {
            return false;
        }
        self.registry_item(name)
            .ok()
            .flatten()
            .map(|item| {
                item.get("managed_by").and_then(|v| v.as_str()) == Some("clawhub")
                    || item.get("risk_level").and_then(|v| v.as_str()) != Some("low")
            })
            .unwrap_or(false)
    }

    pub async fn execute(&self, name: &str, args: &Value) -> anyhow::Result<String> {
        match name {
            "get_current_time" => self.run_simple_script("clock/scripts/get_time.py").await,
            "get_system_info" => {
                self.run_simple_script("system_monitor/scripts/get_sys_info.py")
                    .await
            }
            "summary_rules" => {
                self.run_script_args(
                    "summary_rules/scripts/manage_rules.py",
                    &[format!("--args={}", serde_json::to_string(args)?)],
                )
                .await
            }
            "memory_save" => {
                let key = args
                    .get("key")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow!("memory_save 缺少 key 参数"))?;
                let value = args
                    .get("value")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow!("memory_save 缺少 value 参数"))?;
                self.save_memory_key(key, value)
            }
            "memory_forget" => {
                let key = args
                    .get("key")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow!("memory_forget 缺少 key 参数"))?;
                self.forget_memory_key(key)
            }
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
                self.run_script_args("monte_carlo/scripts/monte_carlo.py", &cli_args)
                    .await
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
                self.run_script_args("file_editor/scripts/edit_file.py", &cli_args)
                    .await
            }
            "write_python" => {
                let folder = args
                    .get("folder")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default();
                let file = args
                    .get("file")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default();
                let content = args
                    .get("content")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default();
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
            _ => self.execute_registry_tool(name, args).await,
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
            if item
                .get("runtime")
                .and_then(|v| v.get("executable"))
                .and_then(|v| v.as_bool())
                == Some(false)
            {
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

    /// 构造 prompt 注入版的工具调用说明。
    ///
    /// 与 function calling 并行使用：模型既可以返回原生 `tool_calls`，
    /// 也可以按这里约定的 `<tool_call>JSON</tool_call>` 文本块发起调用。
    pub fn prompt_injection_system_prompt(
        &self,
        enabled_names: &[String],
    ) -> anyhow::Result<Option<String>> {
        if enabled_names.is_empty() || !self.registry_path.exists() {
            return Ok(None);
        }
        let text = std::fs::read_to_string(&self.registry_path)
            .with_context(|| format!("读取技能注册表失败: {}", self.registry_path.display()))?;
        let raw: Value = serde_json::from_str(&text)?;
        let items = raw.as_array().cloned().unwrap_or_default();
        let catalog = items
            .into_iter()
            .filter_map(|item| {
                let name = item.get("tool_name").and_then(|v| v.as_str())?;
                if !enabled_names.iter().any(|n| n == name) {
                    return None;
                }
                let description = item
                    .get("description")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default();
                let parameters = item.get("parameters").cloned().unwrap_or_else(|| {
                    serde_json::json!({
                        "type": "object",
                        "properties": {}
                    })
                });
                Some(format!(
                    "- {}: {}\n  parameters={}",
                    name,
                    description,
                    serde_json::to_string(&parameters).unwrap_or_else(|_| "{}".to_string())
                ))
            })
            .collect::<Vec<_>>();
        if catalog.is_empty() {
            return Ok(None);
        }
        Ok(Some(format!(
            "可用技能如下：\n{}\n\n你可以同时使用两种技能调用方式：\n1. 优先使用原生 function calling / tool_calls。\n2. 若当前模型对原生 tools 支持不稳定，可以直接输出一个或多个如下文本块，由后端执行：\n<tool_call>{{\"name\":\"技能名\",\"arguments\":{{...}}}}</tool_call>\n要求：\n- 只能使用上面列出的技能。\n- 若输出 <tool_call> 块，则该轮不要输出其他解释性正文。\n- arguments 必须是合法 JSON 对象。\n- 如无需调用技能，直接正常回答即可。",
            catalog.join("\n")
        )))
    }

    fn load_memory_file(&self) -> anyhow::Result<serde_json::Map<String, Value>> {
        if !self.memory_path.exists() {
            return Ok(serde_json::Map::new());
        }
        let text = std::fs::read_to_string(&self.memory_path)
            .with_context(|| format!("读取记忆文件失败: {}", self.memory_path.display()))?;
        let value = serde_json::from_str::<Value>(&text)
            .unwrap_or_else(|_| Value::Object(Default::default()));
        Ok(value.as_object().cloned().unwrap_or_default())
    }

    fn save_memory_file(&self, map: &serde_json::Map<String, Value>) -> anyhow::Result<()> {
        if let Some(parent) = self.memory_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(
            &self.memory_path,
            serde_json::to_string_pretty(&Value::Object(map.clone()))?,
        )
        .with_context(|| format!("写入记忆文件失败: {}", self.memory_path.display()))?;
        Ok(())
    }

    fn save_memory_key(&self, key: &str, value: &str) -> anyhow::Result<String> {
        let mut map = self.load_memory_file()?;
        map.insert(key.to_string(), Value::String(value.to_string()));
        self.save_memory_file(&map)?;
        self.memory_store.insert(
            None,
            "memory_save",
            &serde_json::json!({ "key": key, "value": value }),
        )?;
        Ok(serde_json::json!({ "ok": true, "key": key, "value": value }).to_string())
    }

    fn forget_memory_key(&self, key: &str) -> anyhow::Result<String> {
        let mut map = self.load_memory_file()?;
        if map.remove(key).is_some() {
            self.save_memory_file(&map)?;
            self.memory_store
                .insert(None, "memory_forget", &serde_json::json!({ "key": key }))?;
            Ok(serde_json::json!({ "ok": true, "deleted": key }).to_string())
        } else {
            Ok(
                serde_json::json!({ "ok": false, "error": format!("记忆中没有键 '{}'", key) })
                    .to_string(),
            )
        }
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

    async fn execute_registry_tool(&self, name: &str, args: &Value) -> anyhow::Result<String> {
        let item = self
            .registry_item(name)?
            .ok_or_else(|| anyhow!("unknown_tool"))?;
        if item
            .get("runtime")
            .and_then(|v| v.get("executable"))
            .and_then(|v| v.as_bool())
            == Some(false)
        {
            return Err(anyhow!("tool_not_executable"));
        }

        let skill_dir = item
            .get("skill_dir")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("registry_missing_skill_dir"))?;
        let script = item
            .get("script")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("registry_missing_script"))?;
        let script_path = self.skills_root.join(skill_dir).join(script);
        self.run_script_path(&script_path, args).await
    }

    fn registry_item(&self, name: &str) -> anyhow::Result<Option<Value>> {
        if !self.registry_path.exists() {
            return Ok(None);
        }
        let text = std::fs::read_to_string(&self.registry_path)
            .with_context(|| format!("读取技能注册表失败: {}", self.registry_path.display()))?;
        let raw: Value = serde_json::from_str(&text)?;
        Ok(raw
            .as_array()
            .and_then(|items| {
                items
                    .iter()
                    .find(|item| item.get("tool_name").and_then(|v| v.as_str()) == Some(name))
            })
            .cloned())
    }

    async fn run_script_path(&self, script_path: &PathBuf, args: &Value) -> anyhow::Result<String> {
        let script = script_path.to_string_lossy();
        let mut command = if script.ends_with(".py") {
            let mut command = Command::new("python3");
            command.arg(script_path);
            command
        } else if script.ends_with(".ts") {
            self.ensure_node_dependencies(script_path).await?;
            let mut command = Command::new("node");
            command.arg("--experimental-strip-types");
            command.arg(script_path);
            command
        } else if script.ends_with(".js") || script.ends_with(".mjs") || script.ends_with(".cjs") {
            self.ensure_node_dependencies(script_path).await?;
            let mut command = Command::new("node");
            command.arg(script_path);
            command
        } else if script.ends_with(".sh") || script.ends_with(".bash") || script.ends_with(".zsh") {
            let mut command = Command::new("bash");
            command.arg(script_path);
            command
        } else {
            return Err(anyhow!("unsupported_script_type"));
        };

        if !args.is_null() && args != &serde_json::json!({}) {
            command.arg(format!("--args={}", serde_json::to_string(args)?));
            command.env("SKILL_ARGS_JSON", serde_json::to_string(args)?);
        }

        let output = command
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

    async fn ensure_node_dependencies(&self, script_path: &PathBuf) -> anyhow::Result<()> {
        let Some(skill_dir) = script_path.parent() else {
            return Ok(());
        };
        let package_json = skill_dir.join("package.json");
        if !package_json.exists() {
            return Ok(());
        }
        let node_modules = skill_dir.join("node_modules");
        if node_modules.exists() {
            return Ok(());
        }

        tracing::info!(
            skill_dir = %skill_dir.display(),
            "installing node dependencies for skill"
        );
        let output = Command::new("npm")
            .arg("install")
            .arg("--no-fund")
            .arg("--no-audit")
            .current_dir(skill_dir)
            .output()
            .await
            .with_context(|| format!("安装 Node 依赖失败: {}", skill_dir.display()))?;

        if !output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            let message = if stderr.is_empty() { stdout } else { stderr };
            return Err(anyhow!("npm install failed: {}", message));
        }

        Ok(())
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
                    return Err(anyhow!(
                        "python3 不允许使用 '{}' 参数，只能直接运行 .py 文件",
                        arg
                    ));
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
