use crate::config::schema::{Action, HttpAction, HttpMethod};
use crate::error::{Result, TermStackError};
use crate::template::engine::{TemplateContext, TemplateEngine};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub enum ActionResult {
    Success(Option<String>),
    Error(String),
    Refresh,
    Navigate(String, std::collections::HashMap<String, String>),
}

pub struct ActionExecutor {
    template_engine: Arc<TemplateEngine>,
    http_client: reqwest::Client,
}

impl ActionExecutor {
    pub fn new(template_engine: Arc<TemplateEngine>) -> Self {
        Self {
            template_engine,
            http_client: reqwest::Client::new(),
        }
    }

    pub async fn execute(
        &self,
        action: &Action,
        context: &HashMap<String, Value>,
    ) -> Result<ActionResult> {
        // Convert HashMap to TemplateContext
        let template_ctx = Self::hashmap_to_context(context);

        // Page navigation action
        if let Some(page) = &action.page {
            if !page.is_empty() {
                // Page navigation is handled by the app itself
                return Ok(ActionResult::Navigate(page.clone(), action.context.clone()));
            }
        }

        // CLI action (check for non-empty string)
        if let Some(command) = &action.command {
            if !command.is_empty() {
                return self.execute_cli(action, command, &template_ctx).await;
            }
        }

        // HTTP action
        if let Some(http) = &action.http {
            return self.execute_http(action, http, &template_ctx).await;
        }

        // TODO: Script action
        if let Some(script) = &action.script {
            if !script.is_empty() {
                return Err(TermStackError::Config(
                    "Script actions not yet implemented".to_string(),
                ));
            }
        }

        // TODO: Builtin action
        if let Some(builtin) = &action.builtin {
            if !builtin.is_empty() {
                return Err(TermStackError::Config(
                    "Builtin actions not yet implemented".to_string(),
                ));
            }
        }

        Err(TermStackError::Config(format!(
            "Action '{}' must have command, http, script, builtin, or page specified",
            action.name
        )))
    }

    fn hashmap_to_context(map: &HashMap<String, Value>) -> TemplateContext {
        let mut ctx = TemplateContext::new();

        // Try to extract globals, page contexts, and current row if they exist
        for (key, value) in map {
            if key == "row" || key == "value" {
                if let Some(v) = value.as_object() {
                    ctx = ctx.with_current(Value::Object(v.clone()));
                }
            } else {
                ctx.page_contexts.insert(key.clone(), value.clone());
            }
        }

        ctx
    }

    async fn execute_cli(
        &self,
        action: &Action,
        command: &str,
        context: &TemplateContext,
    ) -> Result<ActionResult> {
        // Render command with template
        let rendered_command = self
            .template_engine
            .render_string(command, context)
            .map_err(|e| TermStackError::Template(e.to_string()))?;

        // Render args with template
        let mut rendered_args = Vec::new();
        for arg in &action.args {
            let rendered_arg = self
                .template_engine
                .render_string(arg, context)
                .map_err(|e| TermStackError::Template(e.to_string()))?;
            rendered_args.push(rendered_arg);
        }

        // Execute command
        let output = tokio::process::Command::new(&rendered_command)
            .args(&rendered_args)
            .output()
            .await
            .map_err(|e| TermStackError::Io(e))?;

        if output.status.success() {
            let message = if let Some(msg) = &action.success_message {
                Some(
                    self.template_engine
                        .render_string(msg, context)
                        .map_err(|e| TermStackError::Template(e.to_string()))?,
                )
            } else {
                Some(format!("Command executed successfully"))
            };

            if action.refresh {
                Ok(ActionResult::Refresh)
            } else {
                Ok(ActionResult::Success(message))
            }
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            let message = if let Some(msg) = &action.error_message {
                self.template_engine
                    .render_string(msg, context)
                    .map_err(|e| TermStackError::Template(e.to_string()))?
            } else {
                format!("Command failed: {}", stderr)
            };
            Ok(ActionResult::Error(message))
        }
    }

    async fn execute_http(
        &self,
        action: &Action,
        http: &HttpAction,
        context: &TemplateContext,
    ) -> Result<ActionResult> {
        // Render URL with template
        let rendered_url = self
            .template_engine
            .render_string(&http.url, context)
            .map_err(|e| TermStackError::Template(e.to_string()))?;

        // Build request
        let mut request = match http.method {
            HttpMethod::GET => self.http_client.get(&rendered_url),
            HttpMethod::POST => self.http_client.post(&rendered_url),
            HttpMethod::PUT => self.http_client.put(&rendered_url),
            HttpMethod::DELETE => self.http_client.delete(&rendered_url),
            HttpMethod::PATCH => self.http_client.patch(&rendered_url),
        };

        // Add headers
        for (key, value) in &http.headers {
            let rendered_value = self
                .template_engine
                .render_string(value, context)
                .map_err(|e| TermStackError::Template(e.to_string()))?;
            request = request.header(key, rendered_value);
        }

        // Add body if present
        if let Some(body) = &http.body {
            let rendered_body = self
                .template_engine
                .render_string(body, context)
                .map_err(|e| TermStackError::Template(e.to_string()))?;
            request = request.body(rendered_body);
        }

        // Execute request
        let response = request.send().await.map_err(|e| TermStackError::Http(e))?;

        if response.status().is_success() {
            let message = if let Some(msg) = &action.success_message {
                Some(
                    self.template_engine
                        .render_string(msg, context)
                        .map_err(|e| TermStackError::Template(e.to_string()))?,
                )
            } else {
                Some(format!("HTTP request succeeded"))
            };

            if action.refresh {
                Ok(ActionResult::Refresh)
            } else {
                Ok(ActionResult::Success(message))
            }
        } else {
            let status = response.status();
            let body = response
                .text()
                .await
                .unwrap_or_else(|_| "Failed to read response body".to_string());
            let message = if let Some(msg) = &action.error_message {
                self.template_engine
                    .render_string(msg, context)
                    .map_err(|e| TermStackError::Template(e.to_string()))?
            } else {
                format!("HTTP request failed: {} - {}", status, body)
            };
            Ok(ActionResult::Error(message))
        }
    }
}

impl Default for ActionExecutor {
    fn default() -> Self {
        Self::new(Arc::new(
            TemplateEngine::new().expect("Failed to create template engine"),
        ))
    }
}
