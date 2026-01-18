use crate::config::schema::SingleDataSource;
use crate::data::provider::DataContext;
use anyhow::Result;
use async_trait::async_trait;
use serde_json::Value;

pub mod cli;
pub mod http;
pub mod registry;
pub mod script;

/// Trait for data source adapters
///
/// Adapters are responsible for fetching data from various sources (CLI, HTTP, databases, etc.)
/// and returning it as JSON Value that can be processed by the rest of the application.
#[async_trait]
pub trait DataSourceAdapter: Send + Sync {
    /// Returns the unique name of this adapter (e.g., "cli", "http", "script", "postgres")
    fn name(&self) -> &str;

    /// Fetches data from the source
    ///
    /// # Arguments
    /// * `source` - The data source configuration
    /// * `ctx` - The data context containing globals and page contexts for template rendering
    ///
    /// # Returns
    /// A JSON Value containing the fetched data
    async fn fetch(&self, source: &SingleDataSource, ctx: &DataContext) -> Result<Value>;
}
