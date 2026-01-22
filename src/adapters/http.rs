use anyhow::{Result, anyhow};
use async_trait::async_trait;
use reqwest::Method;
use serde_json::Value;
use std::collections::HashMap;
use std::time::Duration;

use super::DataSourceAdapter;
use crate::config::schema::{HttpMethod, SingleDataSource};
use crate::data::provider::DataContext;
use crate::globals;
use crate::template::engine::{TemplateContext, TemplateEngine};

/// HTTP data adapter
pub struct HttpAdapter;

impl Default for HttpAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl HttpAdapter {
    pub fn new() -> Self {
        Self
    }

    /// Extract HTTP configuration from data source
    fn extract_config(source: &SingleDataSource) -> Result<HttpConfig> {
        let url = source
            .config
            .get("url")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing 'url' field for HTTP adapter"))?
            .to_string();

        let method = source
            .config
            .get("method")
            .and_then(|v| v.as_str())
            .and_then(|s| match s.to_uppercase().as_str() {
                "GET" => Some(HttpMethod::GET),
                "POST" => Some(HttpMethod::POST),
                "PUT" => Some(HttpMethod::PUT),
                "DELETE" => Some(HttpMethod::DELETE),
                "PATCH" => Some(HttpMethod::PATCH),
                _ => None,
            })
            .unwrap_or(HttpMethod::GET);

        let headers = source
            .config
            .get("headers")
            .and_then(|v| v.as_object())
            .map(|obj| {
                obj.iter()
                    .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                    .collect()
            })
            .unwrap_or_default();

        let params = source
            .config
            .get("params")
            .and_then(|v| v.as_object())
            .map(|obj| {
                obj.iter()
                    .filter_map(|(k, v)| {
                        // Handle both string and number values
                        let val = match v {
                            Value::String(s) => Some(s.clone()),
                            Value::Number(n) => Some(n.to_string()),
                            Value::Bool(b) => Some(b.to_string()),
                            _ => v.as_str().map(String::from),
                        };
                        val.map(|s| (k.clone(), s))
                    })
                    .collect()
            })
            .unwrap_or_default();

        let body = source
            .config
            .get("body")
            .and_then(|v| v.as_str())
            .map(String::from);

        let timeout = source.timeout.as_deref().unwrap_or("30s");
        let timeout_duration = parse_duration(timeout)?;

        Ok(HttpConfig {
            url,
            method,
            headers,
            params,
            body,
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
impl DataSourceAdapter for HttpAdapter {
    fn name(&self) -> &str {
        "http"
    }

    async fn fetch(&self, source: &SingleDataSource, ctx: &DataContext) -> Result<Value> {
        let config = Self::extract_config(source)?;
        let template_engine = TemplateEngine::new()?;
        let template_ctx = Self::to_template_context(ctx);

        // Render URL template
        let url = if TemplateEngine::is_template(&config.url) {
            template_engine.render_string(&config.url, &template_ctx)?
        } else {
            config.url.clone()
        };

        // Get HTTP client
        let client = globals::http_client();

        // Convert HttpMethod to reqwest::Method
        let method = match config.method {
            HttpMethod::GET => Method::GET,
            HttpMethod::POST => Method::POST,
            HttpMethod::PUT => Method::PUT,
            HttpMethod::DELETE => Method::DELETE,
            HttpMethod::PATCH => Method::PATCH,
        };

        let mut request = client.request(method, &url);

        // Add headers (with template rendering)
        for (key, value) in &config.headers {
            let rendered_value = if TemplateEngine::is_template(value) {
                template_engine.render_string(value, &template_ctx)?
            } else {
                value.clone()
            };
            request = request.header(key, rendered_value);
        }

        // Add query params (with template rendering)
        if !config.params.is_empty() {
            let rendered_params: Vec<(String, String)> = config
                .params
                .iter()
                .map(|(k, v)| {
                    let rendered_value = if TemplateEngine::is_template(v) {
                        template_engine
                            .render_string(v, &template_ctx)
                            .map_err(|e| anyhow!("{}", e))
                    } else {
                        Ok(v.clone())
                    };
                    rendered_value.map(|val| (k.clone(), val))
                })
                .collect::<Result<Vec<_>>>()?;

            request = request.query(&rendered_params);
        }

        // Add body (with template rendering)
        if let Some(body) = &config.body {
            let rendered_body = if TemplateEngine::is_template(body) {
                template_engine.render_string(body, &template_ctx)?
            } else {
                body.clone()
            };
            request = request.body(rendered_body);
        }

        // Set timeout
        request = request.timeout(config.timeout);

        // Execute request
        let response = request
            .send()
            .await
            .map_err(|e| anyhow!("HTTP request failed: {}", e))?;

        if !response.status().is_success() {
            return Err(anyhow!(
                "HTTP request failed with status: {}",
                response.status()
            ));
        }

        let text = response
            .text()
            .await
            .map_err(|e| anyhow!("Failed to read response body: {}", e))?;

        // Parse as JSON
        serde_json::from_str(&text).map_err(|e| anyhow!("Failed to parse response as JSON: {}", e))
    }
}

/// HTTP configuration extracted from data source
struct HttpConfig {
    url: String,
    method: HttpMethod,
    headers: HashMap<String, String>,
    params: HashMap<String, String>,
    body: Option<String>,
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
