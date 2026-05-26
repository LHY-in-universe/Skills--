use crate::app::services::config_service::ConfigService;
use anyhow::{anyhow, Context};
use serde_json::{json, Value};
use std::io::ErrorKind;
use std::path::PathBuf;
use tokio::process::Command;

#[derive(Clone)]
pub struct ClawhubService {
    project_root: PathBuf,
    config_service: ConfigService,
}

struct ClawhubCommandOutput {
    stdout: String,
    stderr: String,
}

impl ClawhubService {
    pub fn new(project_root: PathBuf, config_service: ConfigService) -> Self {
        Self {
            project_root,
            config_service,
        }
    }

    pub async fn runtime_info(&self) -> Value {
        let version = self.version().await;
        let auth = self.auth_status().await;
        let installed_count = self.clawhub_installed_count();
        let skills_dir = self.project_root.join("skills");
        let lock_path = self.project_root.join(".clawhub/lock.json");

        match version {
            Ok(version) => json!({
                "available": true,
                "version": version,
                "logged_in": auth.get("logged_in").cloned().unwrap_or(Value::Bool(false)),
                "auth_error": auth.get("auth_error").cloned().unwrap_or(Value::Null),
                "auth_user": auth.get("auth_user").cloned().unwrap_or(Value::Null),
                "workdir": self.project_root.display().to_string(),
                "skills_dir": skills_dir.display().to_string(),
                "lock_path": lock_path.display().to_string(),
                "installed_count": installed_count,
            }),
            Err(err) => json!({
                "available": false,
                "version": Value::Null,
                "logged_in": false,
                "auth_error": Value::Null,
                "auth_user": Value::Null,
                "workdir": self.project_root.display().to_string(),
                "skills_dir": skills_dir.display().to_string(),
                "lock_path": lock_path.display().to_string(),
                "installed_count": installed_count,
                "error": err.to_string(),
            }),
        }
    }

    pub async fn catalog(
        &self,
        limit: Option<usize>,
        sort: Option<&str>,
        query: Option<&str>,
    ) -> anyhow::Result<Value> {
        self.ensure_available().await?;
        let limit = limit.unwrap_or(25).clamp(1, 200);
        let search_query = query.map(str::trim).filter(|s| !s.is_empty());

        if let Some(search_query) = search_query {
            let args = vec![
                "search".to_string(),
                search_query.to_string(),
                "--limit".to_string(),
                limit.to_string(),
            ];
            let output = self.run_command(&args).await?;
            let items = parse_search_output(&output.stdout, search_query);
            return Ok(json!({
                "source": "clawhub_search",
                "limit": limit,
                "sort": Value::Null,
                "query": search_query,
                "items": items,
            }));
        }

        let mut args = vec![
            "explore".to_string(),
            "--json".to_string(),
            "--limit".to_string(),
            limit.to_string(),
        ];
        if let Some(sort) = sort.filter(|s| !s.trim().is_empty()) {
            let normalized = sort.trim().to_ascii_lowercase();
            let normalized = match normalized.as_str() {
                "newest" => "newest",
                "downloads" => "downloads",
                "rating" => "rating",
                "installs" => "installs",
                "installsalltime" | "installs_all_time" => "installsAllTime",
                "trending" => "trending",
                _ => {
                    return Err(anyhow!("invalid_skill_catalog_sort"));
                }
            };
            if normalized.is_empty() {
                return Err(anyhow!("invalid_skill_catalog_sort"));
            }
            args.push("--sort".to_string());
            args.push(normalized.to_string());
        }

        let output = self.run_command(&args).await?;
        let parsed: Value = serde_json::from_str(&output.stdout)
            .with_context(|| format!("invalid_catalog_response: {}", output.stdout))?;
        let items = if parsed.is_array() {
            parsed
        } else {
            parsed
                .get("items")
                .cloned()
                .unwrap_or_else(|| Value::Array(vec![]))
        };

        Ok(json!({
            "source": "clawhub",
            "limit": limit,
            "sort": sort.unwrap_or("newest"),
            "query": Value::Null,
            "items": items,
        }))
    }

    pub async fn install_skill(&self, slug: &str) -> anyhow::Result<Value> {
        self.ensure_available().await?;
        let slug = slug.trim();
        if slug.is_empty() {
            return Err(anyhow!("invalid_skill_slug"));
        }

        let output = self
            .run_command(&["install".to_string(), slug.to_string()])
            .await
            .with_context(|| format!("skill_install_failed: {slug}"))?;

        self.config_service.scan_and_sync_skills()?;
        let skills = self
            .config_service
            .skills_view()?
            .into_iter()
            .filter(|item| {
                item.get("skill_dir").and_then(|v| v.as_str()) == Some(slug)
                    || item
                        .get("origin")
                        .and_then(|v| v.get("slug"))
                        .and_then(|v| v.as_str())
                        == Some(slug)
            })
            .collect::<Vec<_>>();

        Ok(json!({
            "ok": true,
            "slug": slug,
            "stdout": output.stdout,
            "stderr": output.stderr,
            "skills": skills,
            "runtime": self.runtime_info().await,
        }))
    }

    pub async fn uninstall_skill(&self, slug: &str) -> anyhow::Result<Value> {
        self.ensure_available().await?;
        let slug = slug.trim();
        if slug.is_empty() {
            return Err(anyhow!("invalid_skill_slug"));
        }

        let removed_tool_names = self
            .config_service
            .skills_view()?
            .into_iter()
            .filter(|item| {
                item.get("skill_dir").and_then(|v| v.as_str()) == Some(slug)
                    || item
                        .get("origin")
                        .and_then(|v| v.get("slug"))
                        .and_then(|v| v.as_str())
                        == Some(slug)
            })
            .filter_map(|item| {
                item.get("name")
                    .and_then(|v| v.as_str())
                    .map(str::to_string)
            })
            .collect::<Vec<_>>();

        let output = self
            .run_command(&[
                "uninstall".to_string(),
                slug.to_string(),
                "--yes".to_string(),
            ])
            .await
            .with_context(|| format!("skill_uninstall_failed: {slug}"))?;

        self.config_service.scan_and_sync_skills()?;

        Ok(json!({
            "ok": true,
            "slug": slug,
            "removed_tool_names": removed_tool_names,
            "stdout": output.stdout,
            "stderr": output.stderr,
            "runtime": self.runtime_info().await,
        }))
    }

    async fn ensure_available(&self) -> anyhow::Result<String> {
        self.version().await
    }

    async fn version(&self) -> anyhow::Result<String> {
        let mut command = Command::new("clawhub");
        command.arg("--cli-version");
        command.current_dir(&self.project_root);
        command.kill_on_drop(true);

        let output = command.output().await.map_err(|err| {
            if err.kind() == ErrorKind::NotFound {
                anyhow!("clawhub_not_installed")
            } else {
                anyhow!("clawhub_check_failed: {}", err)
            }
        })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            return Err(anyhow!("clawhub_check_failed: {}", stderr));
        }

        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        Ok(if stdout.is_empty() {
            "unknown".to_string()
        } else {
            stdout
        })
    }

    async fn auth_status(&self) -> Value {
        let mut command = Command::new("clawhub");
        command.arg("whoami");
        command.current_dir(&self.project_root);
        command.kill_on_drop(true);

        match command.output().await {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
                let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
                if output.status.success() {
                    json!({
                        "logged_in": true,
                        "auth_error": Value::Null,
                        "auth_user": if stdout.is_empty() { Value::Null } else { Value::String(stdout) }
                    })
                } else {
                    let message = if stderr.is_empty() { stdout } else { stderr };
                    json!({
                        "logged_in": false,
                        "auth_error": if message.is_empty() { Value::Null } else { Value::String(message) },
                        "auth_user": Value::Null
                    })
                }
            }
            Err(err) => json!({
                "logged_in": false,
                "auth_error": Value::String(format!("clawhub auth check failed: {}", err)),
                "auth_user": Value::Null
            }),
        }
    }

    async fn run_command(&self, args: &[String]) -> anyhow::Result<ClawhubCommandOutput> {
        let mut command = Command::new("clawhub");
        command.arg("--no-input");
        command.arg("--workdir").arg(&self.project_root);
        command.arg("--dir").arg("skills");
        command.args(args);
        command.current_dir(&self.project_root);
        command.kill_on_drop(true);

        let output = command.output().await.map_err(|err| {
            if err.kind() == ErrorKind::NotFound {
                anyhow!("clawhub_not_installed")
            } else {
                anyhow!("clawhub_command_failed: {}", err)
            }
        })?;

        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();

        if !output.status.success() {
            let message = if stderr.is_empty() {
                stdout.clone()
            } else {
                stderr.clone()
            };
            return Err(anyhow!("clawhub_command_failed: {}", message));
        }

        Ok(ClawhubCommandOutput { stdout, stderr })
    }

    fn clawhub_installed_count(&self) -> usize {
        let skills_dir = self.project_root.join("skills");
        std::fs::read_dir(skills_dir)
            .ok()
            .into_iter()
            .flat_map(|entries| entries.filter_map(|entry| entry.ok()))
            .filter(|entry| entry.path().join(".clawhub/origin.json").exists())
            .count()
    }
}

fn parse_search_output(stdout: &str, query: &str) -> Vec<Value> {
    stdout
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('-') {
                return None;
            }
            let left = trimmed
                .split_once('(')
                .map(|(head, _)| head.trim_end())
                .unwrap_or(trimmed);
            let tokens = left
                .split_whitespace()
                .map(str::trim)
                .filter(|part| !part.is_empty())
                .collect::<Vec<_>>();
            if tokens.is_empty() {
                return None;
            }
            let slug = tokens.first().copied().unwrap_or_default().to_string();
            let title = if tokens.len() > 1 {
                tokens[1..].join(" ")
            } else {
                slug.clone()
            };
            Some(json!({
                "slug": slug,
                "name": title,
                "title": title,
                "summary": format!("Search result for `{}`", query),
            }))
        })
        .collect()
}
