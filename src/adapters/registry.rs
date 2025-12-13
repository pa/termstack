use super::DataSourceAdapter;
use super::cli::CliAdapter;
use super::http::HttpAdapter;
use super::script::ScriptAdapter;
use crate::config::schema::SingleDataSource;
use crate::data::provider::DataContext;
use anyhow::{Result, anyhow};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;

/// Registry for data source adapters
///
/// The registry manages all available adapters and routes fetch requests
/// to the appropriate adapter based on the data source configuration.
pub struct AdapterRegistry {
    adapters: HashMap<String, Arc<dyn DataSourceAdapter>>,
}

impl AdapterRegistry {
    /// Creates a new empty adapter registry
    pub fn new() -> Self {
        Self {
            adapters: HashMap::new(),
        }
    }

    /// Creates a registry with default built-in adapters registered
    pub fn with_defaults() -> Self {
        let mut registry = Self::new();

        // Register built-in adapters
        registry.register(Arc::new(CliAdapter::new()));
        registry.register(Arc::new(HttpAdapter::new()));
        registry.register(Arc::new(ScriptAdapter::new()));

        registry
    }

    /// Registers a new adapter
    ///
    /// # Arguments
    /// * `adapter` - The adapter to register
    pub fn register(&mut self, adapter: Arc<dyn DataSourceAdapter>) {
        self.adapters.insert(adapter.name().to_string(), adapter);
    }

    /// Fetches data using the appropriate adapter
    ///
    /// # Arguments
    /// * `source` - The data source configuration
    /// * `ctx` - The data context for template rendering
    ///
    /// # Returns
    /// The fetched data as a JSON Value
    ///
    /// # Errors
    /// Returns an error if:
    /// - No adapter is specified in the data source
    /// - The specified adapter is not registered
    /// - The adapter fails to fetch data
    pub async fn fetch(&self, source: &SingleDataSource, ctx: &DataContext) -> Result<Value> {
        let adapter_name = source
            .get_adapter_name()
            .ok_or_else(|| anyhow!("No adapter specified in data source"))?;

        let adapter = self.adapters.get(&adapter_name).ok_or_else(|| {
            let available: Vec<String> = self.adapters.keys().cloned().collect();
            anyhow!(
                "Unknown adapter: '{}'. Available adapters: {}",
                adapter_name,
                available.join(", ")
            )
        })?;

        adapter.fetch(source, ctx).await
    }

    /// Returns the list of registered adapter names
    pub fn list_adapters(&self) -> Vec<String> {
        self.adapters.keys().cloned().collect()
    }
}

impl Default for AdapterRegistry {
    fn default() -> Self {
        Self::with_defaults()
    }
}
