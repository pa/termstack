use async_trait::async_trait;
use reqwest::Method;
use serde_json::Value;
use std::collections::HashMap;
use std::time::Duration;

use super::provider::{DataContext, DataProvider};
use crate::config::HttpMethod;
use crate::error::{Result, TermStackError};
use crate::globals;

/// HTTP data provider
#[derive(Debug, Clone)]
pub struct HttpProvider {
    pub url: String,
    pub method: HttpMethod,
    pub headers: HashMap<String, String>,
    pub body: Option<String>,
    pub timeout: Duration,
}

impl HttpProvider {
    pub fn new(url: String) -> Self {
        Self {
            url,
            method: HttpMethod::GET,
            headers: HashMap::new(),
            body: None,
            timeout: Duration::from_secs(30),
        }
    }

    pub fn with_method(mut self, method: HttpMethod) -> Self {
        self.method = method;
        self
    }

    pub fn with_headers(mut self, headers: HashMap<String, String>) -> Self {
        self.headers = headers;
        self
    }

    pub fn with_body(mut self, body: String) -> Self {
        self.body = Some(body);
        self
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }
}

#[async_trait]
impl DataProvider for HttpProvider {
    async fn fetch(&self, _context: &DataContext) -> Result<Value> {
        let client = globals::http_client();

        let method = match self.method {
            HttpMethod::GET => Method::GET,
            HttpMethod::POST => Method::POST,
            HttpMethod::PUT => Method::PUT,
            HttpMethod::DELETE => Method::DELETE,
            HttpMethod::PATCH => Method::PATCH,
        };

        let mut request = client.request(method, &self.url);

        // Add headers
        for (key, value) in &self.headers {
            request = request.header(key, value);
        }

        // Add body if present
        if let Some(body) = &self.body {
            request = request.body(body.clone());
        }

        let response = request
            .send()
            .await
            .map_err(|e| TermStackError::DataProvider(format!("HTTP request failed: {}", e)))?;

        if !response.status().is_success() {
            return Err(TermStackError::DataProvider(format!(
                "HTTP request failed with status: {}",
                response.status()
            )));
        }

        let text = response.text().await.map_err(|e| {
            TermStackError::DataProvider(format!("Failed to read response body: {}", e))
        })?;

        // Try to parse as JSON
        serde_json::from_str(&text).map_err(|e| {
            TermStackError::DataProvider(format!("Failed to parse response as JSON: {}", e))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore] // Requires network access
    async fn test_http_provider_get() {
        let provider = HttpProvider::new("https://httpbin.org/json".to_string());

        let context = DataContext::new();
        let result = provider.fetch(&context).await;

        assert!(result.is_ok());
    }
}
