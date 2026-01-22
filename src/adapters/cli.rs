use anyhow::{Result, anyhow};
use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;
use tokio::process::Command;

use super::DataSourceAdapter;
use crate::config::schema::SingleDataSource;
use crate::data::provider::DataContext;
use crate::template::engine::{TemplateContext, TemplateEngine};

/// CLI command data adapter
pub struct CliAdapter;

impl Default for CliAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl CliAdapter {
    pub fn new() -> Self {
        Self
    }

    /// Extract CLI configuration from data source
    fn extract_config(source: &SingleDataSource) -> Result<CliConfig> {
        let command = source
            .config
            .get("command")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing 'command' field for CLI adapter"))?
            .to_string();

        let args = source
            .config
            .get("args")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str())
                    .map(String::from)
                    .collect()
            })
            .unwrap_or_default();

        let shell = source
            .config
            .get("shell")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let working_dir = source
            .config
            .get("working_dir")
            .and_then(|v| v.as_str())
            .map(PathBuf::from);

        let env = source
            .config
            .get("env")
            .and_then(|v| v.as_object())
            .map(|obj| {
                obj.iter()
                    .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                    .collect()
            })
            .unwrap_or_default();

        let timeout = source.timeout.as_deref().unwrap_or("30s");
        let timeout_duration = parse_duration(timeout)?;

        Ok(CliConfig {
            command,
            args,
            shell,
            working_dir,
            env,
            timeout: timeout_duration,
        })
    }

    /// Convert DataContext to TemplateContext
    fn to_template_context(ctx: &DataContext) -> TemplateContext {
        let mut template_ctx = TemplateContext::new().with_globals(ctx.globals.clone());

        // Add each page context individually
        for (page, data) in &ctx.page_contexts {
            template_ctx = template_ctx.with_page_context(page.clone(), data.clone());
        }

        template_ctx
    }
}

#[async_trait]
impl DataSourceAdapter for CliAdapter {
    fn name(&self) -> &str {
        "cli"
    }

    async fn fetch(&self, source: &SingleDataSource, ctx: &DataContext) -> Result<Value> {
        let config = Self::extract_config(source)?;
        let template_engine = TemplateEngine::new()?;
        let template_ctx = Self::to_template_context(ctx);

        // Render templates in args
        let rendered_args: Vec<String> = config
            .args
            .iter()
            .map(|arg| {
                if TemplateEngine::is_template(arg) {
                    template_engine
                        .render_string(arg, &template_ctx)
                        .map_err(|e| anyhow!("{}", e))
                } else {
                    Ok(arg.clone())
                }
            })
            .collect::<Result<Vec<_>>>()?;

        // Execute command
        let output = if config.shell {
            // Run in shell
            let shell_cmd = if cfg!(target_os = "windows") {
                "cmd"
            } else {
                "sh"
            };

            let shell_arg = if cfg!(target_os = "windows") {
                "/C"
            } else {
                "-c"
            };

            let full_command = format!("{} {}", config.command, rendered_args.join(" "));

            let mut cmd = Command::new(shell_cmd);
            cmd.arg(shell_arg).arg(full_command);

            if let Some(dir) = &config.working_dir {
                cmd.current_dir(dir);
            }

            for (key, value) in &config.env {
                cmd.env(key, value);
            }

            tokio::time::timeout(config.timeout, cmd.output())
                .await
                .map_err(|_| anyhow!("Command timed out after {:?}", config.timeout))??
        } else {
            // Direct execution
            let mut cmd = Command::new(&config.command);
            cmd.args(&rendered_args);

            if let Some(dir) = &config.working_dir {
                cmd.current_dir(dir);
            }

            for (key, value) in &config.env {
                cmd.env(key, value);
            }

            tokio::time::timeout(config.timeout, cmd.output())
                .await
                .map_err(|_| anyhow!("Command timed out after {:?}", config.timeout))??
        };

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow!(
                "Command failed with status {}: {}",
                output.status,
                stderr
            ));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);

        // Try to parse as JSON first
        match serde_json::from_str(&stdout) {
            Ok(json) => Ok(json),
            Err(_) => {
                // If JSON parsing fails, wrap the raw text as a JSON string value
                Ok(Value::String(stdout.to_string()))
            }
        }
    }
}

/// CLI configuration extracted from data source
struct CliConfig {
    command: String,
    args: Vec<String>,
    shell: bool,
    working_dir: Option<PathBuf>,
    env: HashMap<String, String>,
    timeout: Duration,
}

/// Parse duration string (e.g., "30s", "5m", "1h")
fn parse_duration(s: &str) -> Result<Duration> {
    let s = s.trim();
    if s.is_empty() {
        return Err(anyhow!("Empty duration string"));
    }

    let (num_str, unit) = if let Some(stripped) = s.strip_suffix("ms") {
        (stripped, "ms")
    } else if let Some(stripped) = s.strip_suffix('s') {
        (stripped, "s")
    } else if let Some(stripped) = s.strip_suffix('m') {
        (stripped, "m")
    } else if let Some(stripped) = s.strip_suffix('h') {
        (stripped, "h")
    } else {
        // Default to seconds if no unit
        (s, "s")
    };

    let num: u64 = num_str
        .parse()
        .map_err(|_| anyhow!("Invalid duration number: {}", num_str))?;

    let duration = match unit {
        "ms" => Duration::from_millis(num),
        "s" => Duration::from_secs(num),
        "m" => Duration::from_secs(num * 60),
        "h" => Duration::from_secs(num * 3600),
        _ => return Err(anyhow!("Invalid duration unit: {}", unit)),
    };

    Ok(duration)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_duration() {
        assert_eq!(parse_duration("30s").unwrap(), Duration::from_secs(30));
        assert_eq!(parse_duration("5m").unwrap(), Duration::from_secs(300));
        assert_eq!(parse_duration("1h").unwrap(), Duration::from_secs(3600));
        assert_eq!(parse_duration("500ms").unwrap(), Duration::from_millis(500));
        assert_eq!(parse_duration("10").unwrap(), Duration::from_secs(10));
    }
}
