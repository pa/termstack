use anyhow::{anyhow, Context, Result};
use std::collections::HashSet;

use super::schema::{Config, DataSource, DataSourceType, SingleDataSource};

pub struct ConfigValidator;

impl ConfigValidator {
    pub fn validate(config: &Config) -> Result<()> {
        // Validate version
        if config.version != "v1" {
            return Err(anyhow!(
                "Unsupported config version: {}. Expected: v1",
                config.version
            ));
        }

        // Validate app name
        if config.app.name.trim().is_empty() {
            return Err(anyhow!("App name cannot be empty"));
        }

        // Validate pages exist
        if config.pages.is_empty() {
            return Err(anyhow!("No pages defined in config"));
        }

        // Validate start page exists
        if !config.pages.contains_key(&config.start) {
            return Err(anyhow!("Start page '{}' not found in pages", config.start));
        }

        // Collect all page IDs for reference validation
        let page_ids: HashSet<_> = config.pages.keys().cloned().collect();

        // Validate each page
        for (page_id, page) in &config.pages {
            Self::validate_page(page_id, page, &page_ids)
                .with_context(|| format!("Invalid page: {}", page_id))?;
        }

        Ok(())
    }

    fn validate_page(
        _page_id: &str,
        page: &super::schema::Page,
        page_ids: &HashSet<String>,
    ) -> Result<()> {
        // Validate title
        if page.title.trim().is_empty() {
            return Err(anyhow!("Page title cannot be empty"));
        }

        // Validate data source
        Self::validate_data_source(&page.data).context("Invalid data source")?;

        // Validate navigation references
        if let Some(nav) = &page.next {
            Self::validate_navigation(nav, page_ids).context("Invalid navigation")?;
        }

        // Validate actions
        if let Some(actions) = &page.actions {
            for (idx, action) in actions.iter().enumerate() {
                Self::validate_action(action, page_ids)
                    .with_context(|| format!("Invalid action at index {}", idx))?;
            }
        }

        Ok(())
    }

    fn validate_data_source(data_source: &DataSource) -> Result<()> {
        match data_source {
            DataSource::SingleOrStream(super::schema::SingleOrStream::Single(single)) => {
                Self::validate_single_data_source(single)
            }
            DataSource::SingleOrStream(super::schema::SingleOrStream::Stream(stream)) => {
                Self::validate_stream_data_source(stream)
            }
            DataSource::Multi(multi) => {
                if multi.sources.is_empty() {
                    return Err(anyhow!("Multi data source must have at least one source"));
                }
                for (idx, named_source) in multi.sources.iter().enumerate() {
                    Self::validate_single_data_source(&named_source.source)
                        .with_context(|| format!("Invalid source at index {}", idx))?;
                }
                Ok(())
            }
        }
    }

    fn validate_single_data_source(source: &SingleDataSource) -> Result<()> {
        // Get adapter name (either from adapter field or legacy source_type)
        let adapter_name = source
            .get_adapter_name()
            .ok_or_else(|| anyhow!("Data source must specify either 'adapter' or 'type' field"))?;

        // Validate adapter-specific requirements
        match adapter_name.as_str() {
            "cli" => {
                if !source.config.contains_key("command") {
                    return Err(anyhow!("CLI data source must have 'command' field"));
                }
            }
            "http" => {
                if !source.config.contains_key("url") {
                    return Err(anyhow!("HTTP data source must have 'url' field"));
                }
            }
            "script" => {
                if !source.config.contains_key("script") {
                    return Err(anyhow!("Script data source must have 'script' field"));
                }
            }
            "stream" => {
                return Err(anyhow!(
                    "SingleDataSource cannot have adapter 'stream'. Use StreamDataSource instead."
                ));
            }
            _ => {
                // Unknown adapter - will be caught by registry at runtime
            }
        }

        // Validate timeout format if present
        if let Some(timeout) = &source.timeout {
            humantime::parse_duration(timeout)
                .with_context(|| format!("Invalid timeout format: {}", timeout))?;
        }

        Ok(())
    }

    fn validate_navigation(
        nav: &super::schema::Navigation,
        page_ids: &HashSet<String>,
    ) -> Result<()> {
        match nav {
            super::schema::Navigation::Simple(simple) => {
                if !page_ids.contains(&simple.page) {
                    return Err(anyhow!("Navigation page '{}' not found", simple.page));
                }
            }
            super::schema::Navigation::Conditional(conditionals) => {
                let mut has_default = false;
                for cond in conditionals {
                    if !page_ids.contains(&cond.page) {
                        return Err(anyhow!("Navigation page '{}' not found", cond.page));
                    }
                    if cond.default {
                        if has_default {
                            return Err(anyhow!("Multiple default navigation routes defined"));
                        }
                        has_default = true;
                    }
                }
                if !has_default {
                    return Err(anyhow!("Conditional navigation must have a default route"));
                }
            }
        }
        Ok(())
    }

    fn validate_action(action: &super::schema::Action, page_ids: &HashSet<String>) -> Result<()> {
        // Validate key format
        if action.key.is_empty() {
            return Err(anyhow!("Action key cannot be empty"));
        }

        // Parse and validate key format
        let parsed_key = crate::input::ActionKey::parse(&action.key)
            .map_err(|e| anyhow!("Invalid action key '{}': {}", action.key, e))?;

        // Warn about legacy single-char keys
        if !parsed_key.is_ctrl() && action.key.len() == 1 {
            eprintln!(
                "Warning: Action '{}' uses legacy key format '{}'. \
                Consider migrating to 'ctrl+{}' for better discoverability.",
                action.name, action.key, action.key
            );
        }

        // Warn about problematic Ctrl combinations that may conflict with terminal
        if let crate::input::ActionKey::Ctrl(ch) = parsed_key {
            match ch {
                'c' | 'z' | 's' | 'q' | 'w' => {
                    eprintln!(
                        "Warning: Action '{}' uses Ctrl+{} which may be intercepted by the terminal. \
                        Consider using a different key combination.",
                        action.name, ch.to_ascii_uppercase()
                    );
                }
                _ => {}
            }
        }

        // Validate name
        if action.name.is_empty() {
            return Err(anyhow!("Action name cannot be empty"));
        }

        // Validate that at least one action type is defined
        let has_command = action.command.is_some();
        let has_http = action.http.is_some();
        let has_script = action.script.is_some();
        let has_page = action.page.is_some();
        let has_builtin = action.builtin.is_some();

        let action_count = [has_command, has_http, has_script, has_page, has_builtin]
            .iter()
            .filter(|&&x| x)
            .count();

        if action_count == 0 {
            return Err(anyhow!(
                "Action '{}' must define one of: command, http, script, page, or builtin",
                action.name
            ));
        }

        if action_count > 1 {
            return Err(anyhow!(
                "Action '{}' can only define one action type",
                action.name
            ));
        }

        // Validate page reference if present
        if let Some(page) = &action.page
            && !page_ids.contains(page) {
                return Err(anyhow!("Action page '{}' not found", page));
            }

        // Validate builtin actions
        if let Some(builtin) = &action.builtin {
            let valid_builtins = ["yaml_view", "help", "search", "refresh", "back", "quit"];
            if !valid_builtins.contains(&builtin.as_str()) {
                return Err(anyhow!(
                    "Unknown builtin action: {}. Valid builtins: {:?}",
                    builtin,
                    valid_builtins
                ));
            }
        }

        Ok(())
    }

    fn validate_stream_data_source(source: &super::schema::StreamDataSource) -> Result<()> {
        match source.source_type {
            DataSourceType::Stream => {
                // Validate that at least one source is specified
                if source.command.is_none() && source.websocket.is_none() && source.file.is_none() {
                    return Err(anyhow!(
                        "Stream data source must have 'command', 'websocket', or 'file' field"
                    ));
                }

                // Validate CLI streaming
                if source.command.is_some() && source.command.as_ref().unwrap().is_empty() {
                    return Err(anyhow!("Stream command cannot be empty"));
                }

                // Validate buffer_size
                if source.buffer_size == 0 {
                    return Err(anyhow!("Stream buffer_size must be greater than 0"));
                }

                // Validate buffer_time format if present
                if let Some(buffer_time) = &source.buffer_time {
                    humantime::parse_duration(buffer_time)
                        .with_context(|| format!("Invalid buffer_time format: {}", buffer_time))?;
                }

                // Validate timeout format if present
                if let Some(timeout) = &source.timeout {
                    humantime::parse_duration(timeout)
                        .with_context(|| format!("Invalid timeout format: {}", timeout))?;
                }
            }
            _ => {
                return Err(anyhow!("Stream data source must have type 'stream'"));
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::loader::ConfigLoader;

    #[test]
    fn test_validate_valid_config() {
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
      type: table
      columns:
        - path: "$.name"
          display: "Name"
"#;

        let config = ConfigLoader::load_from_string(yaml).unwrap();
        let result = ConfigValidator::validate(&config);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_invalid_version() {
        let yaml = r#"
version: v2
app:
  name: "Test App"
start: main
pages:
  main:
    title: "Main Page"
    data:
      type: cli
      command: "echo"
    view:
      type: table
      columns: []
"#;

        let config = ConfigLoader::load_from_string(yaml).unwrap();
        let result = ConfigValidator::validate(&config);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("version"));
    }

    #[test]
    fn test_validate_missing_start_page() {
        let yaml = r#"
version: v1
app:
  name: "Test App"
start: nonexistent
pages:
  main:
    title: "Main Page"
    data:
      type: cli
      command: "echo"
    view:
      type: table
      columns: []
"#;

        let config = ConfigLoader::load_from_string(yaml).unwrap();
        let result = ConfigValidator::validate(&config);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Start page"));
    }
}
