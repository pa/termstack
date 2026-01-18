use crate::{config::Config, error::Result, template::TemplateEngine};
use std::sync::OnceLock;

/// Global configuration instance
static CONFIG: OnceLock<Config> = OnceLock::new();

/// Global template engine instance
static TEMPLATE_ENGINE: OnceLock<TemplateEngine> = OnceLock::new();

/// Global HTTP client for all network requests
static HTTP_CLIENT: OnceLock<reqwest::Client> = OnceLock::new();

/// Initialize the global configuration
/// This should be called once at application startup
pub fn init_config(config: Config) -> Result<()> {
    CONFIG
        .set(config)
        .map_err(|_| anyhow::anyhow!("Config already initialized"))?;
    Ok(())
}

/// Get a reference to the global configuration
/// Panics if config hasn't been initialized
pub fn config() -> &'static Config {
    CONFIG
        .get()
        .expect("Config not initialized - call init_config first")
}

/// Initialize the global template engine
/// This should be called once at application startup
pub fn init_template_engine() -> Result<()> {
    let engine = TemplateEngine::new()?;
    TEMPLATE_ENGINE
        .set(engine)
        .map_err(|_| anyhow::anyhow!("Template engine already initialized"))?;
    Ok(())
}

/// Get a reference to the global template engine
/// Panics if template engine hasn't been initialized
pub fn template_engine() -> &'static TemplateEngine {
    TEMPLATE_ENGINE
        .get()
        .expect("Template engine not initialized - call init_template_engine first")
}

/// Get a reference to the global HTTP client
/// Lazily initialized on first access
pub fn http_client() -> &'static reqwest::Client {
    HTTP_CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .pool_max_idle_per_host(10)
            .build()
            .expect("Failed to create HTTP client")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_http_client_singleton() {
        let client1 = http_client();
        let client2 = http_client();
        assert!(std::ptr::eq(client1, client2));
    }
}
