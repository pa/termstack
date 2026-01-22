use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    pub version: String,
    pub app: AppConfig,
    #[serde(default)]
    pub globals: HashMap<String, serde_json::Value>,
    #[serde(default)]
    pub keybindings: Option<Keybindings>,
    pub start: String,
    pub pages: HashMap<String, Page>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AppConfig {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default = "default_theme")]
    pub theme: String,
    #[serde(default)]
    pub refresh_interval: Option<String>,
    #[serde(default = "default_history_size")]
    pub history_size: usize,
}

fn default_theme() -> String {
    "default".to_string()
}

fn default_history_size() -> usize {
    50
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Keybindings {
    #[serde(default)]
    pub global: HashMap<String, String>,
    #[serde(default)]
    pub custom: HashMap<String, String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Page {
    pub title: String,
    #[serde(default)]
    pub description: Option<String>,
    pub data: DataSource,
    pub view: View,
    #[serde(default)]
    pub next: Option<Navigation>,
    #[serde(default)]
    pub actions: Option<Vec<Action>>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum DataSource {
    Multi(MultiDataSource),
    #[serde(with = "single_or_stream")]
    SingleOrStream(SingleOrStream),
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum SingleOrStream {
    Stream(StreamDataSource),
    Single(SingleDataSource),
}

mod single_or_stream {
    use super::*;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn deserialize<'de, D>(deserializer: D) -> Result<SingleOrStream, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct Helper {
            #[serde(rename = "type", default)]
            source_type: Option<DataSourceType>,
            #[serde(default)]
            adapter: Option<String>,
        }

        let value = serde_json::Value::deserialize(deserializer)?;
        let helper: Helper =
            serde_json::from_value(value.clone()).map_err(serde::de::Error::custom)?;

        // Check if it's a stream type (either old "type: stream" or new "adapter: stream")
        let is_stream = match (&helper.source_type, &helper.adapter) {
            (Some(DataSourceType::Stream), _) => true,
            (_, Some(adapter)) if adapter == "stream" => true,
            _ => false,
        };

        if is_stream {
            let stream: StreamDataSource =
                serde_json::from_value(value).map_err(serde::de::Error::custom)?;
            Ok(SingleOrStream::Stream(stream))
        } else {
            let single: SingleDataSource =
                serde_json::from_value(value).map_err(serde::de::Error::custom)?;
            Ok(SingleOrStream::Single(single))
        }
    }

    pub fn serialize<S>(value: &SingleOrStream, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match value {
            SingleOrStream::Stream(s) => s.serialize(serializer),
            SingleOrStream::Single(s) => s.serialize(serializer),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SingleDataSource {
    // New adapter-based approach
    #[serde(default)]
    pub adapter: Option<String>,

    // Old approach (for backwards compatibility during transition)
    #[serde(rename = "type", default)]
    pub source_type: Option<DataSourceType>,

    // Generic config fields (used by all adapters)
    #[serde(flatten)]
    pub config: HashMap<String, serde_json::Value>,

    // Common fields (kept for convenience and backwards compat)
    #[serde(default)]
    pub items: Option<String>,
    #[serde(default)]
    pub timeout: Option<String>,
    #[serde(default)]
    pub refresh_interval: Option<String>,
}

impl SingleDataSource {
    /// Get the adapter name, falling back to source_type for backwards compatibility
    pub fn get_adapter_name(&self) -> Option<String> {
        if let Some(adapter) = &self.adapter {
            Some(adapter.clone())
        } else { self.source_type.as_ref().map(|source_type| match source_type {
                DataSourceType::Cli => "cli".to_string(),
                DataSourceType::Http => "http".to_string(),
                DataSourceType::Stream => "stream".to_string(),
            }) }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MultiDataSource {
    pub sources: Vec<NamedDataSource>,
    #[serde(default)]
    pub merge: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NamedDataSource {
    pub id: String,
    #[serde(flatten)]
    pub source: SingleDataSource,
    #[serde(default)]
    pub optional: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StreamDataSource {
    #[serde(rename = "type")]
    pub source_type: DataSourceType,

    // CLI streaming fields
    #[serde(default)]
    pub command: Option<String>,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub shell: bool,
    #[serde(default)]
    pub working_dir: Option<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,

    // WebSocket streaming fields (future)
    #[serde(default)]
    pub websocket: Option<String>,

    // File tailing fields (future)
    #[serde(default)]
    pub file: Option<String>,

    // Stream buffer configuration
    #[serde(default = "default_buffer_size")]
    pub buffer_size: usize,
    #[serde(default)]
    pub buffer_time: Option<String>,
    #[serde(default = "default_true")]
    pub follow: bool,

    // Common fields
    #[serde(default)]
    pub timeout: Option<String>,
}

fn default_buffer_size() -> usize {
    100
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum DataSourceType {
    Cli,
    Http,
    Stream,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "UPPERCASE")]
#[derive(Default)]
pub enum HttpMethod {
    #[default]
    GET,
    POST,
    PUT,
    DELETE,
    PATCH,
}


#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum View {
    Table(TableView),
    Logs(LogsView),
    Text(TextView),
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TableView {
    pub columns: Vec<TableColumn>,
    #[serde(default)]
    pub sort: Option<TableSort>,
    #[serde(default)]
    pub group_by: Option<String>,
    #[serde(default = "default_true")]
    pub selectable: bool,
    #[serde(default)]
    pub multi_select: bool,
    #[serde(default)]
    pub row_style: Vec<ConditionalStyle>,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TableColumn {
    pub path: String,
    pub display: String,
    #[serde(default)]
    pub width: Option<u16>,
    #[serde(default)]
    pub align: Option<Alignment>,
    #[serde(default)]
    pub transform: Option<String>,
    #[serde(default)]
    pub style: Vec<ConditionalStyle>,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Alignment {
    Left,
    Center,
    Right,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ConditionalStyle {
    #[serde(default)]
    pub condition: Option<String>,
    #[serde(default)]
    pub color: Option<String>,
    #[serde(default)]
    pub bold: bool,
    #[serde(default)]
    pub dim: bool,
    #[serde(default)]
    pub bg: Option<String>,
    #[serde(default)]
    pub default: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TableSort {
    pub column: String,
    #[serde(default)]
    pub order: SortOrder,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum SortOrder {
    #[default]
    Asc,
    Desc,
}


#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LogsView {
    #[serde(default = "default_true")]
    pub follow: bool,
    #[serde(default)]
    pub wrap: bool,
    #[serde(default)]
    pub show_timestamps: bool,
    #[serde(default)]
    pub show_line_numbers: bool,
    #[serde(default)]
    pub syntax: Option<String>,
    #[serde(default)]
    pub filters: Vec<LogFilter>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LogFilter {
    pub name: String,
    pub key: String,
    pub pattern: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TextView {
    /// Optional: Explicitly specify the content type (yaml, json, xml, toml, etc.)
    /// If not specified, will auto-detect based on content
    #[serde(default)]
    pub syntax: Option<String>,

    /// Enable line numbers
    #[serde(default)]
    pub line_numbers: bool,

    /// Enable word wrap for long lines
    #[serde(default = "default_true")]
    pub wrap: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum Navigation {
    Simple(SimpleNavigation),
    Conditional(Vec<ConditionalNavigation>),
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SimpleNavigation {
    pub page: String,
    #[serde(default)]
    pub context: HashMap<String, String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ConditionalNavigation {
    #[serde(default)]
    pub condition: Option<String>,
    pub page: String,
    #[serde(default)]
    pub context: HashMap<String, String>,
    #[serde(default)]
    pub default: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Action {
    pub key: String,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub confirm: Option<String>,

    // Action type (one of these should be set)
    #[serde(default)]
    pub command: Option<String>,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub http: Option<HttpAction>,
    #[serde(default)]
    pub script: Option<String>,
    #[serde(default)]
    pub page: Option<String>,
    #[serde(default)]
    pub builtin: Option<String>,

    // Action result handling
    #[serde(default)]
    pub success_message: Option<String>,
    #[serde(default)]
    pub error_message: Option<String>,
    #[serde(default)]
    pub notification: Option<NotificationConfig>,
    #[serde(default)]
    pub refresh: bool,
    #[serde(default)]
    pub context: HashMap<String, String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NotificationConfig {
    #[serde(default)]
    pub on_success: Option<String>,
    #[serde(default)]
    pub on_failure: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct HttpAction {
    pub method: HttpMethod,
    pub url: String,
    #[serde(default)]
    pub headers: HashMap<String, String>,
    #[serde(default)]
    pub body: Option<String>,
}
