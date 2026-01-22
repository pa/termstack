use anyhow::{Result, anyhow};
use async_trait::async_trait;
use serde_json::Value;
use std::path::Path;
use std::time::Duration;
use tokio::process::Command;

use super::DataSourceAdapter;
use crate::config::schema::SingleDataSource;
use crate::data::provider::DataContext;
use crate::template::engine::{TemplateContext, TemplateEngine};

/// Script data adapter
///
/// Executes shell scripts that output JSON data.
/// This allows users to integrate custom data sources without writing Rust code.
pub struct ScriptAdapter;

impl Default for ScriptAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl ScriptAdapter {
    pub fn new() -> Self {
        Self
    }

    /// Extract script configuration from data source
    fn extract_config(source: &SingleDataSource) -> Result<ScriptConfig> {
        let script = source
            .config
            .get("script")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing 'script' field for script adapter"))?
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

        let timeout = source.timeout.as_deref().unwrap_or("30s");
        let timeout_duration = parse_duration(timeout)?;

        Ok(ScriptConfig {
            script,
            args,
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
impl DataSourceAdapter for ScriptAdapter {
    fn name(&self) -> &str {
        "script"
    }

    async fn fetch(&self, source: &SingleDataSource, ctx: &DataContext) -> Result<Value> {
        let config = Self::extract_config(source)?;
        let template_engine = TemplateEngine::new()?;
        let template_ctx = Self::to_template_context(ctx);

        // Validate script exists
        if !Path::new(&config.script).exists() {
            return Err(anyhow!("Script not found: {}", config.script));
        }

        // Render template args
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

        // Serialize context as JSON for script to use
        let context_json = serde_json::to_string(ctx)
            .map_err(|e| anyhow!("Failed to serialize context: {}", e))?;

        // Execute script with timeout
        let output = tokio::time::timeout(
            config.timeout,
            Command::new(&config.script)
                .args(&rendered_args)
                .env("TERMSTACK_CONTEXT", context_json)
                .output(),
        )
        .await
        .map_err(|_| anyhow!("Script timed out after {:?}", config.timeout))??;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow!(
                "Script failed (exit code {}): {}",
                output.status.code().unwrap_or(-1),
                stderr
            ));
        }

        // Parse JSON output
        let stdout = String::from_utf8_lossy(&output.stdout);
        serde_json::from_str(&stdout).map_err(|e| {
            anyhow!(
                "Script did not output valid JSON: {}. Output: {}",
                e,
                stdout
            )
        })
    }
}

/// Script configuration extracted from data source
struct ScriptConfig {
    script: String,
    args: Vec<String>,
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
