use serde_json::Value;
use std::collections::HashMap;
use tera::{Context, Tera};

use super::filters;
use crate::error::{Result, TermStackError};

/// Template engine for rendering dynamic content
#[derive(Debug, Clone)]
pub struct TemplateEngine {
    tera: Tera,
}

impl TemplateEngine {
    pub fn new() -> Result<Self> {
        let mut tera = Tera::default();

        // Register custom filters
        tera.register_filter("timeago", filters::timeago);
        tera.register_filter("filesizeformat", filters::filesizeformat);
        tera.register_filter("status_color", filters::status_color);

        Ok(Self { tera })
    }

    /// Render a template string with the given context
    pub fn render_string(&self, template: &str, context: &TemplateContext) -> Result<String> {
        let tera_context = context.to_tera_context();

        // Clone tera for rendering since render_str requires &mut self
        let mut tera = self.tera.clone();
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
}

impl TemplateContext {
    pub fn new() -> Self {
        Self {
            globals: HashMap::new(),
            page_contexts: HashMap::new(),
            current: None,
        }
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

    /// Convert to Tera context
    pub fn to_tera_context(&self) -> Context {
        let mut context = Context::new();

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
}
