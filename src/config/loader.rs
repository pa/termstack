use anyhow::{Context, Result};
use std::path::Path;

use super::schema::Config;

pub struct ConfigLoader;

impl ConfigLoader {
    pub fn load_from_file<P: AsRef<Path>>(path: P) -> Result<Config> {
        let content = std::fs::read_to_string(path.as_ref())
            .with_context(|| format!("Failed to read config file: {:?}", path.as_ref()))?;

        Self::load_from_string(&content)
    }

    pub fn load_from_string(content: &str) -> Result<Config> {
        let config: Config =
            serde_yaml::from_str(content).context("Failed to parse YAML config")?;

        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_minimal_config() {
        let yaml = r#"
version: v1
app:
  name: "Test App"
start: main
pages:
  main:
    title: "Main Page"
    data:
      type: cli
      command: "echo"
      args: ["hello"]
    view:
      layout: table
      columns:
        - path: "$.name"
          display: "Name"
"#;

        let result = ConfigLoader::load_from_string(yaml);
        assert!(result.is_ok());

        let config = result.unwrap();
        assert_eq!(config.version, "v1");
        assert_eq!(config.app.name, "Test App");
        assert_eq!(config.start, "main");
        assert!(config.pages.contains_key("main"));
    }
}
