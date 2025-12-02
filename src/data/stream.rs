use crate::error::Result;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc;

/// Messages sent from the streaming task to the main app
#[derive(Debug, Clone)]
pub enum StreamMessage {
    /// New line of data received
    Data(String),
    /// Stream connected and started
    Connected,
    /// Stream ended normally
    End,
    /// Stream encountered an error
    Error(String),
}

/// Stream provider for CLI command streaming
pub struct StreamProvider {
    command: String,
    args: Vec<String>,
    shell: bool,
    working_dir: Option<String>,
    env: std::collections::HashMap<String, String>,
}

impl StreamProvider {
    pub fn new(command: String) -> Self {
        Self {
            command,
            args: Vec::new(),
            shell: false,
            working_dir: None,
            env: std::collections::HashMap::new(),
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

    pub fn with_working_dir(mut self, dir: String) -> Self {
        self.working_dir = Some(dir);
        self
    }

    pub fn with_env(mut self, env: std::collections::HashMap<String, String>) -> Self {
        self.env = env;
        self
    }

    /// Start streaming command output line by line
    /// Returns a receiver that will get StreamMessage updates
    pub fn start_stream(self) -> Result<mpsc::Receiver<StreamMessage>> {
        let (tx, rx) = mpsc::channel(1000); // Bounded channel to prevent memory issues

        // Spawn background task to handle streaming
        tokio::spawn(async move {
            if let Err(e) = Self::stream_task(self, tx.clone()).await {
                let _ = tx.send(StreamMessage::Error(e.to_string())).await;
            }
        });

        Ok(rx)
    }

    async fn stream_task(provider: StreamProvider, tx: mpsc::Sender<StreamMessage>) -> Result<()> {
        // Build command
        let mut cmd = if provider.shell {
            let mut shell_cmd = if cfg!(target_os = "windows") {
                Command::new("cmd")
            } else {
                Command::new("sh")
            };

            if cfg!(target_os = "windows") {
                shell_cmd.arg("/C");
            } else {
                shell_cmd.arg("-c");
            }

            let full_command = if provider.args.is_empty() {
                provider.command
            } else {
                format!("{} {}", provider.command, provider.args.join(" "))
            };

            shell_cmd.arg(full_command);
            shell_cmd
        } else {
            let mut cmd = Command::new(&provider.command);
            cmd.args(&provider.args);
            cmd
        };

        // Set working directory
        if let Some(working_dir) = &provider.working_dir {
            cmd.current_dir(working_dir);
        }

        // Set environment variables
        for (key, value) in &provider.env {
            cmd.env(key, value);
        }

        // Configure stdio
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        // Spawn the process
        let mut child = cmd.spawn()?;

        // Send connected message
        let _ = tx.send(StreamMessage::Connected).await;

        // Get stdout handle
        let stdout = child.stdout.take().ok_or_else(|| {
            crate::error::TermStackError::DataProvider("Failed to get stdout".to_string())
        })?;

        let reader = BufReader::new(stdout);
        let mut lines = reader.lines();

        // Read lines as they come
        while let Ok(Some(line)) = lines.next_line().await {
            // Send line to app
            if tx.send(StreamMessage::Data(line)).await.is_err() {
                // Receiver dropped, kill the process
                let _ = child.kill().await;
                break;
            }
        }

        // Wait for process to finish
        let status = child.wait().await?;

        if status.success() {
            let _ = tx.send(StreamMessage::End).await;
        } else {
            let _ = tx
                .send(StreamMessage::Error(format!(
                    "Command exited with status: {}",
                    status
                )))
                .await;
        }

        Ok(())
    }
}
