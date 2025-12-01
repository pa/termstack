use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;

use crate::error::Result;

/// Context passed to data providers for variable interpolation
#[derive(Debug, Clone, Default)]
pub struct DataContext {
    /// Global variables from config
    pub globals: HashMap<String, Value>,
    /// Page-specific context (selected row data from previous pages)
    pub page_contexts: HashMap<String, Value>,
}

impl DataContext {
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

/// Trait for data providers (CLI, HTTP, etc.)
#[async_trait]
pub trait DataProvider: Send + Sync {
    /// Fetch data from the provider
    async fn fetch(&self, context: &DataContext) -> Result<Value>;
}

/// Result of data fetching with optional caching metadata
#[derive(Debug, Clone)]
pub struct DataResult {
    pub data: Value,
    pub cached: bool,
    pub timestamp: std::time::SystemTime,
}

impl DataResult {
    pub fn new(data: Value) -> Self {
        Self {
            data,
            cached: false,
            timestamp: std::time::SystemTime::now(),
        }
    }

    pub fn with_cached(mut self, cached: bool) -> Self {
        self.cached = cached;
        self
    }
}
