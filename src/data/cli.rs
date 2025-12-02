use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;
use tokio::process::Command;

use super::provider::{DataContext, DataProvider};
use crate::error::{Result, TermStackError};

/// CLI command data provider
#[derive(Debug, Clone)]
pub struct CliProvider {
    pub command: String,
    pub args: Vec<String>,
    pub shell: bool,
    pub working_dir: Option<PathBuf>,
    pub env: HashMap<String, String>,
    pub timeout: Duration,
}

impl CliProvider {
    pub fn new(command: String) -> Self {
        Self {
            command,
            args: Vec::new(),
            shell: false,
            working_dir: None,
            env: HashMap::new(),
            timeout: Duration::from_secs(30),
        }
    }

    pub fn with_args(mut self, args: Vec<String>) -> Self {
        self.args = args;
        self
    }

    pub fn with_shell(mut self, shell: bool) -> Self {
        self.shell = shell;
        self
    }

    pub fn with_working_dir(mut self, dir: PathBuf) -> Self {
        self.working_dir = Some(dir);
        self
    }

    pub fn with_env(mut self, env: HashMap<String, String>) -> Self {
        self.env = env;
        self
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }
}

#[async_trait]
impl DataProvider for CliProvider {
    async fn fetch(&self, _context: &DataContext) -> Result<Value> {
        let output = if self.shell {
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

            let full_command = format!("{} {}", self.command, self.args.join(" "));

            let mut cmd = Command::new(shell_cmd);
            cmd.arg(shell_arg).arg(full_command);

            if let Some(dir) = &self.working_dir {
                cmd.current_dir(dir);
            }

            for (key, value) in &self.env {
                cmd.env(key, value);
            }

            tokio::time::timeout(self.timeout, cmd.output())
                .await
                .map_err(|_| TermStackError::DataProvider("Command timed out".to_string()))?
                .map_err(|e| {
                    TermStackError::DataProvider(format!("Failed to execute command: {}", e))
                })?
        } else {
            // Direct execution
            let mut cmd = Command::new(&self.command);
            cmd.args(&self.args);

            if let Some(dir) = &self.working_dir {
                cmd.current_dir(dir);
            }

            for (key, value) in &self.env {
                cmd.env(key, value);
            }

            tokio::time::timeout(self.timeout, cmd.output())
                .await
                .map_err(|_| TermStackError::DataProvider("Command timed out".to_string()))?
                .map_err(|e| {
                    TermStackError::DataProvider(format!("Failed to execute command: {}", e))
                })?
        };

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(TermStackError::DataProvider(format!(
                "Command failed with status {}: {}",
                output.status, stderr
            )));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);

        // Try to parse as JSON first
        match serde_json::from_str(&stdout) {
            Ok(json) => Ok(json),
            Err(_) => {
                // If JSON parsing fails, wrap the raw text as a JSON string value
                // This allows text views to display raw command output (like kubectl describe, yaml, etc.)
                Ok(Value::String(stdout.to_string()))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_cli_provider_echo() {
        let provider = CliProvider::new("echo".to_string())
            .with_args(vec![r#"{"test": "value"}"#.to_string()]);

        let context = DataContext::new();
        let result = provider.fetch(&context).await;

        assert!(result.is_ok());
        let data = result.unwrap();
        assert_eq!(data["test"], "value");
    }
}
