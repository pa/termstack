use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use tera::{Context, Tera};

use super::filters;
use crate::error::{Result, TermStackError};

/// Template engine for rendering dynamic content (optimized with Arc<RwLock> for shared access)
#[derive(Debug, Clone)]
pub struct TemplateEngine {
    tera: Arc<RwLock<Tera>>,
}

impl TemplateEngine {
    pub fn new() -> Result<Self> {
        let mut tera = Tera::default();

        // Register custom filters
        tera.register_filter("timeago", filters::timeago);
        tera.register_filter("filesizeformat", filters::filesizeformat);
        tera.register_filter("status_color", filters::status_color);

        Ok(Self {
            tera: Arc::new(RwLock::new(tera)),
        })
    }

    /// Render a template string with the given context (optimized - no cloning!)
    pub fn render_string(&self, template: &str, context: &TemplateContext) -> Result<String> {
        let tera_context = context.to_tera_context();

        // Use write lock for rendering (only locks during actual rendering, not cloning)
        let mut tera = self.tera.write().map_err(|e| {
            TermStackError::Template(format!("Failed to acquire template lock: {}", e))
        })?;

        tera.render_str(template, &tera_context)
            .map_err(|e| TermStackError::Template(format!("Template rendering error: {}", e)))
    }

    /// Render a template and parse result as JSON value
    pub fn render_value(&self, template: &str, context: &TemplateContext) -> Result<Value> {
        let rendered = self.render_string(template, context)?;

        serde_json::from_str(&rendered).map_err(|e| {
            TermStackError::Template(format!("Failed to parse rendered template as JSON: {}", e))
        })
    }

    /// Check if a string contains template syntax
    pub fn is_template(s: &str) -> bool {
        s.contains("{{") && s.contains("}}")
    }
}

impl Default for TemplateEngine {
    fn default() -> Self {
        Self::new().expect("Failed to create default template engine")
    }
}

/// Context for template rendering
#[derive(Debug, Clone)]
pub struct TemplateContext {
    /// Global variables from config
    pub globals: HashMap<String, Value>,
    /// Page-specific contexts (previous page data)
    pub page_contexts: HashMap<String, Value>,
    /// Current row data (for inline rendering)
    pub current: Option<Value>,
    /// Environment variables (loaded from system environment)
    pub env: HashMap<String, Value>,
}

impl TemplateContext {
    pub fn new() -> Self {
        Self {
            globals: HashMap::new(),
            page_contexts: HashMap::new(),
            current: None,
            env: Self::load_env_vars(),
        }
    }

    /// Create a new context with pre-allocated capacity (optimization)
    pub fn with_capacity() -> Self {
        Self {
            globals: HashMap::with_capacity(10),
            page_contexts: HashMap::with_capacity(5),
            current: None,
            env: Self::load_env_vars(),
        }
    }

    /// Reset the context for reuse (optimization for pooling)
    pub fn reset(&mut self) {
        self.globals.clear();
        self.page_contexts.clear();
        self.current = None;
        self.env = Self::load_env_vars();
    }

    /// Load environment variables from the system
    fn load_env_vars() -> HashMap<String, Value> {
        std::env::vars()
            .map(|(k, v)| (k, Value::String(v)))
            .collect()
    }

    /// Reload environment variables (useful if env changes at runtime)
    pub fn reload_env(&mut self) {
        self.env = Self::load_env_vars();
    }

    pub fn with_globals(mut self, globals: HashMap<String, Value>) -> Self {
        self.globals = globals;
        self
    }

    pub fn with_page_context(mut self, page: String, data: Value) -> Self {
        self.page_contexts.insert(page, data);
        self
    }

    pub fn with_current(mut self, current: Value) -> Self {
        self.current = Some(current);
        self
    }

    /// Set globals in-place (optimization for reuse)
    pub fn set_globals(&mut self, globals: HashMap<String, Value>) {
        self.globals = globals;
    }

    /// Set current value in-place (optimization for reuse)
    pub fn set_current(&mut self, current: Option<Value>) {
        self.current = current;
    }

    /// Add page context in-place (optimization for reuse)
    pub fn add_page_context(&mut self, page: String, data: Value) {
        self.page_contexts.insert(page, data);
    }

    /// Convert to Tera context
    pub fn to_tera_context(&self) -> Context {
        let mut context = Context::new();

        // Add environment variables (first, so they can be overridden)
        context.insert("env", &self.env);

        // Add globals
        for (key, value) in &self.globals {
            context.insert(key, value);
        }

        // Add page contexts
        for (page, data) in &self.page_contexts {
            context.insert(page, data);
        }

        // Add current row data
        if let Some(current) = &self.current {
            context.insert("row", current);

            // Only insert "value" if not already set by page_contexts
            if !context.contains_key("value") {
                context.insert("value", current);
            }

            // Also flatten current object fields to top level for convenience
            if let Value::Object(map) = current {
                for (key, value) in map {
                    if !context.contains_key(key) {
                        context.insert(key, value);
                    }
                }
            }
        }

        context
    }
}

impl Default for TemplateContext {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_render_simple() {
        let engine = TemplateEngine::new().unwrap();
        let mut context = TemplateContext::new();
        context.globals.insert("name".to_string(), json!("World"));

        let result = engine.render_string("Hello {{ name }}!", &context).unwrap();
        assert_eq!(result, "Hello World!");
    }

    #[test]
    fn test_render_with_page_context() {
        let engine = TemplateEngine::new().unwrap();
        let context =
            TemplateContext::new().with_page_context("pods".to_string(), json!({"name": "my-pod"}));

        let result = engine
            .render_string("Pod: {{ pods.name }}", &context)
            .unwrap();
        assert_eq!(result, "Pod: my-pod");
    }

    #[test]
    fn test_render_with_current() {
        let engine = TemplateEngine::new().unwrap();
        let context = TemplateContext::new().with_current(json!({"status": "running"}));

        let result = engine
            .render_string("Status: {{ status }}", &context)
            .unwrap();
        assert_eq!(result, "Status: running");
    }

    #[test]
    fn test_is_template() {
        assert!(TemplateEngine::is_template("{{ var }}"));
        assert!(TemplateEngine::is_template("Hello {{ name }}!"));
        assert!(!TemplateEngine::is_template("Just a string"));
    }

    #[test]
    fn test_env_var_interpolation() {
        // Set test environment variable
        unsafe {
            std::env::set_var("TEST_VAR", "test_value");
            std::env::set_var("TEST_API_KEY", "secret123");
        }

        let engine = TemplateEngine::new().unwrap();
        let context = TemplateContext::new();

        // Test basic env var
        let result = engine
            .render_string("Value: {{ env.TEST_VAR }}", &context)
            .unwrap();
        assert_eq!(result, "Value: test_value");

        // Test API key pattern
        let result = engine
            .render_string("Bearer {{ env.TEST_API_KEY }}", &context)
            .unwrap();
        assert_eq!(result, "Bearer secret123");

        // Cleanup
        unsafe {
            std::env::remove_var("TEST_VAR");
            std::env::remove_var("TEST_API_KEY");
        }
    }

    #[test]
    fn test_env_var_in_header() {
        unsafe {
            std::env::set_var("GITHUB_TOKEN", "ghp_test_token");
        }

        let engine = TemplateEngine::new().unwrap();
        let context = TemplateContext::new();

        let result = engine
            .render_string("Authorization: Bearer {{ env.GITHUB_TOKEN }}", &context)
            .unwrap();
        assert_eq!(result, "Authorization: Bearer ghp_test_token");

        unsafe {
            std::env::remove_var("GITHUB_TOKEN");
        }
    }

    #[test]
    fn test_env_var_with_default() {
        let engine = TemplateEngine::new().unwrap();
        let context = TemplateContext::new();

        // Test with non-existent var using default filter
        let result = engine
            .render_string(
                "{{ env.NONEXISTENT | default(value='fallback') }}",
                &context,
            )
            .unwrap();
        assert_eq!(result, "fallback");

        // Test with existing var
        unsafe {
            std::env::set_var("EXISTING_VAR", "real_value");
        }
        let context = TemplateContext::new(); // Reload env
        let result = engine
            .render_string(
                "{{ env.EXISTING_VAR | default(value='fallback') }}",
                &context,
            )
            .unwrap();
        assert_eq!(result, "real_value");

        unsafe {
            std::env::remove_var("EXISTING_VAR");
        }
    }

    #[test]
    fn test_env_var_reload() {
        unsafe {
            std::env::set_var("DYNAMIC_VAR", "initial");
        }

        let engine = TemplateEngine::new().unwrap();
        let mut context = TemplateContext::new();

        let result = engine
            .render_string("{{ env.DYNAMIC_VAR }}", &context)
            .unwrap();
        assert_eq!(result, "initial");

        // Change environment variable
        unsafe {
            std::env::set_var("DYNAMIC_VAR", "updated");
        }

        // Reload environment variables
        context.reload_env();

        let result = engine
            .render_string("{{ env.DYNAMIC_VAR }}", &context)
            .unwrap();
        assert_eq!(result, "updated");

        unsafe {
            std::env::remove_var("DYNAMIC_VAR");
        }
    }

    #[test]
    fn test_env_vars_with_globals() {
        unsafe {
            std::env::set_var("API_BASE", "https://api.example.com");
            std::env::set_var("API_KEY", "secret");
        }

        let engine = TemplateEngine::new().unwrap();
        let mut context = TemplateContext::new();

        // Globals can reference env vars (would happen during config load)
        context
            .globals
            .insert("api_base".to_string(), json!("https://api.example.com"));
        context
            .globals
            .insert("endpoint".to_string(), json!("/users"));

        // Template can use both env vars and globals
        let result = engine
            .render_string(
                "{{ api_base }}{{ endpoint }}?key={{ env.API_KEY }}",
                &context,
            )
            .unwrap();
        assert_eq!(result, "https://api.example.com/users?key=secret");

        unsafe {
            std::env::remove_var("API_BASE");
            std::env::remove_var("API_KEY");
        }
    }

    #[test]
    fn test_env_vars_in_url_template() {
        unsafe {
            std::env::set_var("TEST_GITHUB_API", "https://api.github.com");
            std::env::set_var("TEST_AUTH_TOKEN", "ghp_token123");
        }

        let engine = TemplateEngine::new().unwrap();
        let context = TemplateContext::new();

        // Simulate URL template
        let result = engine
            .render_string("{{ env.TEST_GITHUB_API }}/user/repos", &context)
            .unwrap();
        assert_eq!(result, "https://api.github.com/user/repos");

        // Simulate header template
        let result = engine
            .render_string("Bearer {{ env.TEST_AUTH_TOKEN }}", &context)
            .unwrap();
        assert_eq!(result, "Bearer ghp_token123");

        unsafe {
            std::env::remove_var("TEST_GITHUB_API");
            std::env::remove_var("TEST_AUTH_TOKEN");
        }
    }
}
