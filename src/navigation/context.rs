use serde_json::Value;
use std::collections::HashMap;

/// Context for navigation and template rendering
#[derive(Debug, Clone, Default)]
pub struct NavigationContext {
    /// Global variables from config
    pub globals: HashMap<String, Value>,
    /// Page-specific contexts (selected row data from previous pages)
    pub page_contexts: HashMap<String, Value>,
}

impl NavigationContext {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_globals(mut self, globals: HashMap<String, Value>) -> Self {
        self.globals = globals;
        self
    }

    pub fn set_page_context(&mut self, page: String, data: Value) {
        self.page_contexts.insert(page, data);
    }

    pub fn get_page_context(&self, page: &str) -> Option<&Value> {
        self.page_contexts.get(page)
    }

    pub fn get_global(&self, key: &str) -> Option<&Value> {
        self.globals.get(key)
    }
}
