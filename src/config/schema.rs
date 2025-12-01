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
    Single(SingleDataSource),
    Multi(MultiDataSource),
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SingleDataSource {
    #[serde(rename = "type")]
    pub source_type: DataSourceType,

    // CLI fields
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

    // HTTP fields
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub method: Option<HttpMethod>,
    #[serde(default)]
    pub headers: HashMap<String, String>,
    #[serde(default)]
    pub body: Option<String>,

    // Common fields
    #[serde(default)]
    pub items: Option<String>,
    #[serde(default)]
    pub timeout: Option<String>,
    #[serde(default)]
    pub cache: Option<String>,
    #[serde(default)]
    pub refresh_interval: Option<String>,
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

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum DataSourceType {
    Cli,
    Http,
    Stream, // Phase 2
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "UPPERCASE")]
pub enum HttpMethod {
    GET,
    POST,
    PUT,
    DELETE,
    PATCH,
}

impl Default for HttpMethod {
    fn default() -> Self {
        HttpMethod::GET
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "layout", rename_all = "lowercase")]
pub enum View {
    Table(TableView),
    Detail(DetailView),
    Logs(LogsView), // Phase 2
    Yaml(YamlView),
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
pub enum SortOrder {
    Asc,
    Desc,
}

impl Default for SortOrder {
    fn default() -> Self {
        SortOrder::Asc
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DetailView {
    #[serde(default)]
    pub sections: Vec<DetailSection>,
    #[serde(default)]
    pub fields: HashMap<String, String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DetailSection {
    pub title: String,
    pub fields: HashMap<String, String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LogsView {
    #[serde(default = "default_true")]
    pub follow: bool,
    #[serde(default)]
    pub wrap: bool,
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
pub struct YamlView {
    // No additional config for now
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
    pub refresh: bool,
    #[serde(default)]
    pub context: HashMap<String, String>,
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
