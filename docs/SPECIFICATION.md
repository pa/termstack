# TermStack - Specification Document

**Version**: 1.0.0  
**Date**: 2025-12-01  
**Status**: Draft

---

## Table of Contents

1. [Overview](#overview)
2. [Goals & Objectives](#goals--objectives)
3. [Architecture](#architecture)
4. [Technology Stack](#technology-stack)
5. [Configuration Schema](#configuration-schema)
6. [Core Components](#core-components)
7. [User Experience & Keybindings](#user-experience--keybindings)
8. [Data Flow](#data-flow)
9. [Error Handling](#error-handling)
10. [Phase 1 Features](#phase-1-features)
11. [Phase 2 Features](#phase-2-features)
12. [Implementation Plan](#implementation-plan)

---

## Overview

**TermStack** is a generic TUI (Terminal User Interface) framework for building rich, navigable dashboards using simple YAML configuration files. Inspired by k9s, it enables developers to create powerful terminal applications without writing UI code.

### Key Features

- **Config-driven**: Define pages, data sources, views, and actions in YAML
- **Dynamic rendering**: Automatically renders tables, details, logs, and YAML views
- **Multi-source data**: Fetch from CLI commands or HTTP endpoints
- **Template engine**: Use Tera templates for dynamic content and variable interpolation
- **Navigation stack**: Navigate between pages with context passing
- **Action system**: Execute CLI commands, HTTP requests, or Lua scripts
- **Adapter system**: Extend functionality with plugins
- **Hot reload**: Update config without restarting (Phase 2)

---

## Goals & Objectives

### Primary Goals

1. **Zero UI Code**: Users should define TUI apps purely through YAML configuration
2. **k9s-like UX**: Provide familiar navigation patterns (table → detail → logs → back)
3. **Extensibility**: Support plugins and custom actions
4. **Performance**: Async data fetching with responsive UI
5. **Developer Experience**: Clear error messages, validation, and testing tools

### Non-Goals

- Not a general-purpose UI framework (use ratatui directly for complex UIs)
- Not a replacement for dedicated tools (k9s for Kubernetes, lazydocker for Docker)
- Not a web framework or HTTP server

---

## Architecture

### High-Level Architecture

```
┌─────────────────────────────────────────────────────────┐
│                      User Input                         │
│                   (Keyboard Events)                     │
└────────────────────┬────────────────────────────────────┘
                     │
                     ▼
┌─────────────────────────────────────────────────────────┐
│                    App State Machine                    │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐  │
│  │   Router     │  │  Nav Stack   │  │   Context    │  │
│  └──────────────┘  └──────────────┘  └──────────────┘  │
└────────┬────────────────────────────────────┬───────────┘
         │                                    │
         ▼                                    ▼
┌─────────────────────┐            ┌─────────────────────┐
│   Data Providers    │            │   View Renderers    │
│  ┌───────────────┐  │            │  ┌───────────────┐  │
│  │ CLI Provider  │  │            │  │  Table View   │  │
│  │ HTTP Provider │  │            │  │  Detail View  │  │
│  │ Stream        │  │            │  │  Logs View    │  │
│  └───────────────┘  │            │  │  YAML View    │  │
│         │           │            │  └───────────────┘  │
│         ▼           │            └─────────────────────┘
│  ┌───────────────┐  │
│  │ JSONPath      │  │
│  │ Cache         │  │
│  └───────────────┘  │
└─────────────────────┘
         │
         ▼
┌─────────────────────────────────────────────────────────┐
│                  Template Engine (Tera)                 │
└─────────────────────────────────────────────────────────┘
```

### Directory Structure

```
termstack/
├── Cargo.toml
├── README.md
├── SPECIFICATION.md
├── LICENSE
├── examples/
│   ├── raptor.yaml
│   ├── kubernetes.yaml
│   └── docker.yaml
├── src/
│   ├── main.rs              # CLI entry point
│   ├── app.rs               # Main app state machine
│   │
│   ├── config/
│   │   ├── mod.rs
│   │   ├── schema.rs        # Serde structs for YAML
│   │   ├── loader.rs        # Load & validate config
│   │   ├── validator.rs     # Validation rules
│   │   └── defaults.rs      # Default keybindings, themes
│   │
│   ├── data/
│   │   ├── mod.rs
│   │   ├── provider.rs      # DataProvider trait
│   │   ├── cli.rs           # Execute shell commands
│   │   ├── http.rs          # HTTP requests (reqwest)
│   │   ├── stream.rs        # Streaming data (logs) [Phase 2]
│   │   ├── cache.rs         # TTL cache
│   │   └── jsonpath.rs      # JSONPath extraction
│   │
│   ├── navigation/
│   │   ├── mod.rs
│   │   ├── router.rs        # Page router
│   │   ├── stack.rs         # Navigation history
│   │   └── context.rs       # Context storage
│   │
│   ├── view/
│   │   ├── mod.rs
│   │   ├── renderer.rs      # ViewRenderer trait
│   │   ├── table.rs         # Table view (ratatui Table)
│   │   ├── detail.rs        # Detail/key-value view
│   │   ├── logs.rs          # Log streaming view [Phase 2]
│   │   ├── yaml.rs          # YAML/JSON viewer
│   │   └── help.rs          # Help overlay
│   │
│   ├── template/
│   │   ├── mod.rs
│   │   ├── engine.rs        # Tera template engine
│   │   └── filters.rs       # Custom filters (timeago, etc)
│   │
│   ├── action/
│   │   ├── mod.rs
│   │   ├── executor.rs      # Execute actions (CLI/HTTP/Lua)
│   │   ├── lua_runtime.rs   # Lua integration (mlua) [Phase 2]
│   │   └── builtins.rs      # Built-in actions
│   │
│   ├── adapter/             # [Phase 2]
│   │   ├── mod.rs
│   │   ├── loader.rs        # Load adapters
│   │   ├── manifest.rs      # Adapter manifest parsing
│   │   └── registry.rs      # Adapter registry
│   │
│   ├── ui/
│   │   ├── mod.rs
│   │   ├── layout.rs        # Layout manager
│   │   ├── theme.rs         # Color schemes
│   │   ├── statusbar.rs     # Status bar widget
│   │   ├── toast.rs         # Toast notifications
│   │   └── breadcrumb.rs    # Navigation breadcrumbs
│   │
│   ├── input/
│   │   ├── mod.rs
│   │   ├── handler.rs       # Key event handling
│   │   ├── search.rs        # Search mode
│   │   └── command.rs       # Command mode
│   │
│   └── util/
│       ├── mod.rs
│       ├── hotreload.rs     # File watching (notify) [Phase 2]
│       └── export.rs        # Data export [Phase 2]
│
└── tests/
    ├── integration/
    └── fixtures/
```

---

## Technology Stack

### Core Dependencies

```toml
[dependencies]
# TUI Framework
crossterm = "0.28"          # Terminal control
ratatui = "0.29"            # TUI framework
color-eyre = "0.6"          # Error handling

# Async Runtime
tokio = { version = "1", features = ["full"] }

# Serialization
serde = { version = "1", features = ["derive"] }
serde_json = "1"
serde_yaml = "0.9"

# Data & Templates
tera = "1.20"               # Template engine
serde_json_path = "0.6"     # JSONPath queries
reqwest = { version = "0.12", features = ["json"] }  # HTTP client

# CLI
clap = { version = "4", features = ["derive"] }

# Utilities
anyhow = "1"                # Error handling
thiserror = "1"             # Custom errors
chrono = "0.4"              # Date/time
humantime = "2"             # Duration parsing
humansize = "2"             # File size formatting
tui-input = "0.9"           # Input widget

# Phase 2
mlua = { version = "0.9", features = ["lua54", "async"] }  # Lua scripting
notify = "6"                # File watching
syntect = "5"               # Syntax highlighting
```

### Why These Choices?

- **Ratatui**: Industry-standard TUI framework, excellent docs, active community
- **Tera**: More powerful than Handlebars, better error messages
- **serde_json_path**: Most mature JSONPath implementation in Rust
- **Tokio**: De facto async runtime for Rust
- **mlua**: Safe Lua embedding for scripting (Phase 2)

---

## Configuration Schema

### Complete YAML Schema

```yaml
version: v1

# Application metadata
app:
  name: "Application Name"
  description: "Optional description"
  theme: "default"          # default | nord | dracula | custom
  refresh_interval: "30s"   # Optional auto-refresh
  history_size: 50          # Navigation stack size

# Global variables accessible via {{ globals.var }}
globals:
  api_url: "https://api.example.com"
  environment: "prod"
  custom_var: "value"

# Custom keybindings (optional, extends defaults)
keybindings:
  global:
    "Ctrl+q": quit
    "F1": help
  custom:
    "x": my_custom_action

# Entry page
start: page_id

# Page definitions
pages:
  page_id:
    # Page metadata
    title: "{{ page.title }}"
    description: "Optional description shown in help"
    
    # Data source configuration
    data:
      # === Single Source ===
      type: cli | http | stream
      
      # CLI Source
      command: "command_name"
      args: ["arg1", "{{ context.var }}"]
      shell: false              # Run in shell vs direct exec
      working_dir: "/path"      # Optional working directory
      env:                      # Optional environment variables
        VAR: "value"
      
      # HTTP Source
      url: "{{ globals.api_url }}/endpoint"
      method: GET               # GET | POST | PUT | DELETE | PATCH
      headers:
        Authorization: "Bearer {{ token }}"
        Content-Type: "application/json"
      body: '{"key": "{{ value }}"}'
      
      # Data Extraction
      items: "$.data[*]"        # JSONPath for array extraction
      timeout: "30s"
      cache: "5m"               # Cache TTL (optional)
      
      # === OR Multiple Sources ===
      sources:
        - id: main
          type: cli
          command: "..."
        - id: supplemental
          type: http
          url: "..."
          optional: true        # Don't fail if unavailable
      merge: true               # Merge sources into single dataset
    
    # View configuration
    view:
      layout: table | detail | logs | yaml
      
      # === TABLE LAYOUT ===
      columns:
        - path: "$.field"       # JSONPath to field
          display: "Column Name"
          width: 20             # Fixed width (optional)
          align: left           # left | center | right
          transform: "{{ value | upper }}"  # Tera filter
          style:
            - condition: "{{ value == 'active' }}"
              color: green
              bold: true
            - default:
              color: white
      
      # Table Options
      sort:
        column: "$.name"
        order: asc              # asc | desc
      group_by: "$.category"    # Group rows by field
      selectable: true          # Enable row selection
      multi_select: false       # Allow multi-row selection
      
      # Row-level Styling
      row_style:
        - condition: "{{ status == 'error' }}"
          color: red
        - condition: "{{ disabled }}"
          dim: true
      
      # === DETAIL LAYOUT ===
      sections:
        - title: "Section Title"
          fields:
            "Label": "$.path"
            "Another": "{{ value | transform }}"
      
      # OR simple flat fields
      fields:
        "Label": "$.path"
        "Another Label": "$.another.path"
      
      # === LOGS LAYOUT === [Phase 2]
      follow: true              # Auto-scroll to bottom
      wrap: false               # Line wrapping
      syntax: auto              # auto | json | yaml | none
      filters:
        - name: "Errors"
          key: "e"
          pattern: "ERROR|FATAL"
      
      # === YAML LAYOUT ===
      # (No additional config, shows raw data)
    
    # Navigation
    next:
      # === Simple Navigation ===
      page: next_page_id
      context:
        var_name: "$.field"     # Capture from selected row
      
      # === OR Conditional Routing ===
      - condition: "{{ type == 'deployment' }}"
        page: deployment_page
      - condition: "{{ type == 'service' }}"
        page: service_page
      - default: default_page
    
    # Actions (key bindings)
    actions:
      - key: "ctrl+d"
        name: "Delete"
        description: "Delete selected item"
        confirm: "Delete {{ name }}?"
        
        # === CLI Action ===
        command: "kubectl delete {{ kind }} {{ name }}"
        success_message: "Deleted {{ name }}"
        error_message: "Failed to delete"
        refresh: true           # Reload page after action
        
        # === HTTP Action === [Phase 2]
        http:
          method: DELETE
          url: "{{ globals.api }}/items/{{ id }}"
          headers: {}
          body: ""
        
        # === Lua Script === [Phase 2]
        script: |
          local input = prompt("Enter value:")
          if input then
            exec("command " .. input)
            return "Success!"
          end
        
        # === Navigation Action ===
        page: another_page
        context:
          var: "{{ value }}"
      
      # Built-in actions (always available)
      - key: "ctrl+y"
        name: "YAML View"
        builtin: yaml_view
```

### Schema Validation Rules

1. **Required Fields**:
   - `version`: Must be "v1"
   - `app.name`: Non-empty string
   - `start`: Must reference existing page
   - `pages`: At least one page defined
   - Each page must have: `title`, `data`, `view`

2. **Type Constraints**:
   - `data.type`: Must be "cli", "http", or "stream"
   - `view.layout`: Must be "table", "detail", "logs", or "yaml"
   - `timeout`: Must be valid duration (e.g., "30s", "5m")
   - `cache`: Must be valid duration

3. **Reference Validation**:
   - `start` must point to existing page
   - `next.page` must point to existing page
   - `actions[].page` must point to existing page

4. **JSONPath Validation**:
   - All `path` fields must be valid JSONPath expressions
   - `items` must be valid JSONPath (preferably array selector)

5. **Template Validation**:
   - All template strings must be valid Tera syntax
   - Variables must be in scope (from context, globals, or current data)

---

## Core Components

### 1. App State Machine

**File**: `src/app.rs`

```rust
pub struct App {
    config: Config,
    router: Router,
    navigation_stack: NavigationStack,
    context: Context,
    data_cache: DataCache,
    current_page: String,
    view_state: ViewState,
    input_mode: InputMode,
    toast_manager: ToastManager,
    template_engine: TemplateEngine,
    running: bool,
}

pub enum InputMode {
    Normal,
    Search(String),
    Command(String),
    Confirm(ConfirmDialog),
}

pub enum ViewState {
    Loading,
    Loaded(ViewData),
    Error(String),
}
```

### 2. Configuration System

**File**: `src/config/schema.rs`

```rust
#[derive(Debug, Deserialize, Serialize)]
pub struct Config {
    pub version: String,
    pub app: AppConfig,
    pub globals: HashMap<String, Value>,
    pub keybindings: Option<Keybindings>,
    pub start: String,
    pub pages: HashMap<String, Page>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Page {
    pub title: String,
    pub description: Option<String>,
    pub data: DataSource,
    pub view: View,
    pub next: Option<Navigation>,
    pub actions: Option<Vec<Action>>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum DataSource {
    Cli(CliSource),
    Http(HttpSource),
    Stream(StreamSource),  // Phase 2
    Multi(MultiSource),    // Multiple sources
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "layout", rename_all = "lowercase")]
pub enum View {
    Table(TableView),
    Detail(DetailView),
    Logs(LogsView),   // Phase 2
    Yaml(YamlView),
}
```

### 3. Data Provider System

**File**: `src/data/provider.rs`

```rust
#[async_trait]
pub trait DataProvider: Send + Sync {
    async fn fetch(&self, context: &Context) -> Result<Vec<Value>>;
}

pub struct CliProvider {
    command: String,
    args: Vec<String>,
    shell: bool,
    working_dir: Option<PathBuf>,
    env: HashMap<String, String>,
    timeout: Duration,
}

pub struct HttpProvider {
    url: String,
    method: Method,
    headers: HeaderMap,
    body: Option<String>,
    timeout: Duration,
}
```

**File**: `src/data/jsonpath.rs`

```rust
pub struct JsonPathExtractor {
    path: JsonPath,
}

impl JsonPathExtractor {
    pub fn new(path: &str) -> Result<Self> {
        let path = JsonPath::parse(path)?;
        Ok(Self { path })
    }
    
    pub fn extract(&self, data: &Value) -> Result<Vec<Value>> {
        // Extract array of items using JSONPath
    }
    
    pub fn extract_single(&self, data: &Value) -> Result<Value> {
        // Extract single value
    }
}
```

### 4. Navigation System

**File**: `src/navigation/router.rs`

```rust
pub struct Router {
    config: Arc<Config>,
    current_page: String,
}

impl Router {
    pub fn resolve_page(&self, page_id: &str) -> Result<&Page> {
        self.config.pages.get(page_id)
            .ok_or_else(|| anyhow!("Page not found: {}", page_id))
    }
    
    pub fn resolve_next(&self, page: &Page, selected_row: &Value) -> Result<NextPage> {
        // Resolve conditional navigation
    }
}
```

**File**: `src/navigation/stack.rs`

```rust
pub struct NavigationStack {
    frames: Vec<NavigationFrame>,
    max_size: usize,
}

#[derive(Debug, Clone)]
pub struct NavigationFrame {
    pub page_id: String,
    pub context: HashMap<String, Value>,
    pub scroll_state: usize,
    pub selected_index: usize,
}

impl NavigationStack {
    pub fn push(&mut self, frame: NavigationFrame) {
        if self.frames.len() >= self.max_size {
            self.frames.remove(0);
        }
        self.frames.push(frame);
    }
    
    pub fn pop(&mut self) -> Option<NavigationFrame> {
        self.frames.pop()
    }
    
    pub fn current(&self) -> Option<&NavigationFrame> {
        self.frames.last()
    }
}
```

**File**: `src/navigation/context.rs`

```rust
pub struct Context {
    // Navigation history contexts: page_name -> selected row data
    page_contexts: HashMap<String, Value>,
    // Global variables
    globals: HashMap<String, Value>,
}

impl Context {
    pub fn set_page_context(&mut self, page: &str, data: Value) {
        self.page_contexts.insert(page.to_string(), data);
    }
    
    pub fn get(&self, path: &str) -> Option<&Value> {
        // Resolve {{ page.field }} or {{ globals.var }}
        // Example: "projects.name" -> page_contexts["projects"]["name"]
    }
    
    pub fn to_tera_context(&self) -> tera::Context {
        // Convert to Tera context for template rendering
    }
}
```

### 5. View Rendering System

**File**: `src/view/renderer.rs`

```rust
pub trait ViewRenderer {
    fn render(&mut self, frame: &mut Frame, area: Rect, data: &[Value]);
    fn handle_input(&mut self, key: KeyEvent) -> ViewAction;
    fn get_selected(&self) -> Option<&Value>;
}

pub enum ViewAction {
    None,
    Navigate(String, Context),
    ExecuteAction(String),
    Back,
    Quit,
    Search,
    YamlView,
    Refresh,
}
```

**File**: `src/view/table.rs`

```rust
pub struct TableView {
    config: TableViewConfig,
    rows: Vec<Value>,
    selected_index: usize,
    scroll_offset: usize,
    search_filter: Option<String>,
}

impl TableView {
    pub fn new(config: TableViewConfig) -> Self { /* ... */ }
    
    fn render_table(&self, frame: &mut Frame, area: Rect) {
        // Use ratatui::widgets::Table
        // Apply column config, styling, sorting
    }
    
    fn apply_filter(&self, rows: &[Value]) -> Vec<Value> {
        // Apply search filter
    }
    
    fn apply_sort(&self, rows: &mut [Value]) {
        // Apply sorting
    }
}

impl ViewRenderer for TableView {
    fn handle_input(&mut self, key: KeyEvent) -> ViewAction {
        match key.code {
            KeyCode::Char('j') | KeyCode::Down => self.move_down(),
            KeyCode::Char('k') | KeyCode::Up => self.move_up(),
            KeyCode::Char('g') => self.move_top(),
            KeyCode::Char('G') => self.move_bottom(),
            KeyCode::Enter => ViewAction::Navigate(/* ... */),
            // ...
        }
    }
}
```

**File**: `src/view/detail.rs`

```rust
pub struct DetailView {
    config: DetailViewConfig,
    data: Value,
    scroll_offset: usize,
}

impl ViewRenderer for DetailView {
    fn render(&mut self, frame: &mut Frame, area: Rect, data: &[Value]) {
        // Render key-value pairs or sections
        // Use ratatui::widgets::Paragraph or List
    }
    
    fn handle_input(&mut self, key: KeyEvent) -> ViewAction {
        // Scrolling only
        match key.code {
            KeyCode::Char('j') | KeyCode::Down => self.scroll_down(),
            KeyCode::Char('k') | KeyCode::Up => self.scroll_up(),
            // ...
        }
    }
}
```

**File**: `src/view/yaml.rs`

```rust
pub struct YamlView {
    data: Value,
    scroll_offset: usize,
    formatted: String,
}

impl YamlView {
    pub fn new(data: Value) -> Self {
        let formatted = serde_yaml::to_string(&data).unwrap_or_default();
        Self {
            data,
            scroll_offset: 0,
            formatted,
        }
    }
}

impl ViewRenderer for YamlView {
    fn render(&mut self, frame: &mut Frame, area: Rect, _data: &[Value]) {
        // Render YAML/JSON with syntax highlighting (Phase 2)
        // Use ratatui::widgets::Paragraph with scrolling
    }
}
```

### 6. Template Engine

**File**: `src/template/engine.rs`

```rust
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
    
    pub fn render_string(&self, template: &str, context: &Context) -> Result<String> {
        let tera_context = context.to_tera_context();
        self.tera.render_str(template, &tera_context)
            .map_err(|e| anyhow!("Template error: {}", e))
    }
    
    pub fn render_value(&self, template: &str, context: &Context) -> Result<Value> {
        let rendered = self.render_string(template, context)?;
        serde_json::from_str(&rendered)
            .map_err(|e| anyhow!("JSON parse error: {}", e))
    }
}
```

**File**: `src/template/filters.rs`

```rust
pub fn timeago(value: &Value, _args: &HashMap<String, Value>) -> tera::Result<Value> {
    // Convert timestamp to "2 hours ago"
}

pub fn filesizeformat(value: &Value, _args: &HashMap<String, Value>) -> tera::Result<Value> {
    // Convert bytes to "1.5 GB"
}

pub fn status_color(value: &Value, _args: &HashMap<String, Value>) -> tera::Result<Value> {
    // Map status to color name
    match value.as_str() {
        Some("running") | Some("active") => Ok(Value::String("green".to_string())),
        Some("error") | Some("failed") => Ok(Value::String("red".to_string())),
        _ => Ok(Value::String("yellow".to_string())),
    }
}
```

### 7. Action System

**File**: `src/action/executor.rs`

```rust
pub struct ActionExecutor {
    template_engine: Arc<TemplateEngine>,
}

impl ActionExecutor {
    pub async fn execute(&self, action: &Action, context: &Context) -> Result<ActionResult> {
        match action {
            Action::Cli(cli) => self.execute_cli(cli, context).await,
            Action::Http(http) => self.execute_http(http, context).await,
            Action::Script(script) => self.execute_script(script, context).await,
            Action::Navigation(nav) => self.execute_navigation(nav, context),
            Action::Builtin(builtin) => self.execute_builtin(builtin),
        }
    }
    
    async fn execute_cli(&self, cli: &CliAction, context: &Context) -> Result<ActionResult> {
        // Render command and args with templates
        let command = self.template_engine.render_string(&cli.command, context)?;
        let args: Vec<String> = cli.args.iter()
            .map(|arg| self.template_engine.render_string(arg, context))
            .collect::<Result<Vec<_>>>()?;
        
        // Execute command
        let output = tokio::process::Command::new(command)
            .args(args)
            .output()
            .await?;
        
        if output.status.success() {
            Ok(ActionResult::Success(cli.success_message.clone()))
        } else {
            Ok(ActionResult::Error(String::from_utf8_lossy(&output.stderr).to_string()))
        }
    }
}

pub enum ActionResult {
    Success(Option<String>),
    Error(String),
    Navigate(String, Context),
}
```

### 8. UI Components

**File**: `src/ui/statusbar.rs`

```rust
pub struct StatusBar {
    current_page: String,
    current_mode: InputMode,
    shortcuts: Vec<(String, String)>,
}

impl StatusBar {
    pub fn render(&self, frame: &mut Frame, area: Rect) {
        // Render bottom status bar
        // Format: "[page_name] | Normal | q:quit ?:help /:search"
    }
}
```

**File**: `src/ui/toast.rs`

```rust
pub struct ToastManager {
    toasts: VecDeque<Toast>,
}

pub struct Toast {
    message: String,
    level: ToastLevel,
    created_at: Instant,
    duration: Duration,
}

pub enum ToastLevel {
    Info,
    Success,
    Warning,
    Error,
}

impl ToastManager {
    pub fn show(&mut self, message: String, level: ToastLevel) {
        self.toasts.push_back(Toast {
            message,
            level,
            created_at: Instant::now(),
            duration: Duration::from_secs(3),
        });
    }
    
    pub fn render(&mut self, frame: &mut Frame, area: Rect) {
        // Render toasts in top-right corner
        // Auto-dismiss after duration
        self.toasts.retain(|t| t.created_at.elapsed() < t.duration);
    }
}
```

**File**: `src/ui/breadcrumb.rs`

```rust
pub struct Breadcrumb {
    navigation_stack: Vec<String>,
}

impl Breadcrumb {
    pub fn render(&self, frame: &mut Frame, area: Rect) {
        // Render breadcrumb trail
        // Format: "projects > environments > resources"
    }
}
```

### 9. Input Handling

**File**: `src/input/handler.rs`

```rust
pub struct InputHandler {
    keybindings: Keybindings,
}

impl InputHandler {
    pub fn handle(&self, key: KeyEvent, mode: &InputMode) -> InputAction {
        match mode {
            InputMode::Normal => self.handle_normal(key),
            InputMode::Search(_) => self.handle_search(key),
            InputMode::Command(_) => self.handle_command(key),
            InputMode::Confirm(_) => self.handle_confirm(key),
        }
    }
    
    fn handle_normal(&self, key: KeyEvent) -> InputAction {
        // Check custom keybindings first
        // Then default keybindings
        match key.code {
            KeyCode::Char('q') => InputAction::Quit,
            KeyCode::Char('?') => InputAction::ShowHelp,
            KeyCode::Char('/') => InputAction::EnterSearch,
            KeyCode::Char(':') => InputAction::EnterCommand,
            KeyCode::Char('y') => InputAction::YamlView,
            KeyCode::Char('r') => InputAction::Refresh,
            KeyCode::Esc => InputAction::Back,
            _ => InputAction::PassToView(key),
        }
    }
}

pub enum InputAction {
    None,
    Quit,
    Back,
    ShowHelp,
    EnterSearch,
    EnterCommand,
    YamlView,
    Refresh,
    PassToView(KeyEvent),
    ExecuteAction(String),
}
```

**File**: `src/input/search.rs`

```rust
pub struct SearchMode {
    query: String,
    cursor: usize,
}

impl SearchMode {
    pub fn handle_key(&mut self, key: KeyEvent) -> SearchAction {
        match key.code {
            KeyCode::Char(c) => {
                self.query.insert(self.cursor, c);
                self.cursor += 1;
                SearchAction::Update(self.query.clone())
            }
            KeyCode::Backspace => {
                if self.cursor > 0 {
                    self.query.remove(self.cursor - 1);
                    self.cursor -= 1;
                }
                SearchAction::Update(self.query.clone())
            }
            KeyCode::Enter => SearchAction::Execute(self.query.clone()),
            KeyCode::Esc => SearchAction::Cancel,
            _ => SearchAction::None,
        }
    }
}

pub enum SearchAction {
    None,
    Update(String),
    Execute(String),
    Cancel,
}
```

---

## User Experience & Keybindings

### Default Keybindings

#### Global (All Modes)

| Key | Action | Description |
|-----|--------|-------------|
| `q` | Quit | Exit application |
| `?` | Help | Show help overlay |
| `Esc` | Back | Go back to previous page |
| `Ctrl+C` | Force Quit | Immediate exit |

#### Normal Mode (Navigation)

| Key | Action | Description |
|-----|--------|-------------|
| `j` / `↓` | Move Down | Select next item |
| `k` / `↑` | Move Up | Select previous item |
| `g` | Go to Top | Jump to first item |
| `G` | Go to Bottom | Jump to last item |
| `Enter` | Navigate | Go to next page / drill down |
| `r` | Refresh | Reload current page data |
| `/` | Search | Enter search mode |
| `:` | Command | Enter command mode |
| `y` | YAML View | Toggle YAML/raw view |
| `Ctrl+R` | Auto-Refresh | Toggle auto-refresh [Phase 2] |
| `h` / `←` | Back | Same as Esc |
| `l` / `→` | Forward | Navigate forward (if available) |

#### Table View Specific

| Key | Action | Description |
|-----|--------|-------------|
| `Space` | Select | Toggle row selection (multi-select) |
| `a` | Select All | Select all visible rows |
| `s` | Sort | Cycle sort column |
| `S` | Sort Desc | Reverse sort order |

#### Detail View Specific

| Key | Action | Description |
|-----|--------|-------------|
| `j` / `↓` | Scroll Down | Scroll content down |
| `k` / `↑` | Scroll Up | Scroll content up |
| `Ctrl+D` | Page Down | Scroll half page down |
| `Ctrl+U` | Page Up | Scroll half page up |

#### Search Mode

| Key | Action | Description |
|-----|--------|-------------|
| `Enter` | Apply | Apply search filter |
| `Esc` | Cancel | Exit search, clear filter |
| `Backspace` | Delete | Delete character |
| `Ctrl+I` | Toggle Case | Toggle case-sensitive search |

**Column-specific search:** Type `%Column Name% term` to search within a specific column. The column name is matched case-insensitively against table column display names. If the column is not found, falls back to global search.

Examples:
- `nginx` — searches all columns
- `%Name% nginx` — searches only the "Name" column
- `%Project Type% active` — handles multi-word column names
- `!error.*timeout` — regex search across all columns

#### Command Mode [Phase 2]

| Key | Action | Description |
|-----|--------|-------------|
| `Enter` | Execute | Execute command |
| `Esc` | Cancel | Exit command mode |
| `Tab` | Complete | Auto-complete command |

### UI Layout

```
┌────────────────────────────────────────────────────────┐
│ App Name                        [Toast Notification]   │ ← Toast (top-right)
├────────────────────────────────────────────────────────┤
│ projects > environments > resources                    │ ← Breadcrumb
├────────────────────────────────────────────────────────┤
│                                                        │
│  ID          Type         Status      Updated         │ ← Table View
│  ──          ────         ──────      ───────         │
│  > abc123    deployment   running     2h ago          │
│    def456    service      pending     5m ago          │
│    ghi789    job          completed   1d ago          │
│                                                        │
│                                                        │
│                                                        │
│                                                        │
├────────────────────────────────────────────────────────┤
│ [resources] Normal | q:quit ?:help /:search r:refresh │ ← Status Bar
└────────────────────────────────────────────────────────┘
```

### Help Overlay

```
┌──────────────────────────────────────┐
│            TermStack Help            │
├──────────────────────────────────────┤
│ Navigation                           │
│   j/↓        Move down               │
│   k/↑        Move up                 │
│   g          Go to top               │
│   G          Go to bottom            │
│   Enter      Drill down / Select     │
│   Esc        Go back                 │
│                                      │
│ Actions                              │
│   r          Refresh                 │
│   /          Search                  │
│   y          YAML view               │
│   ?          Toggle help             │
│   q          Quit                    │
│                                      │
│ Custom Actions                       │
│   d          Delete resource         │
│   l          View logs               │
│                                      │
│         Press ? or Esc to close      │
└──────────────────────────────────────┘
```

---

## Data Flow

### 1. Application Startup

```
┌─────────────┐
│   main()    │
└──────┬──────┘
       │
       ▼
┌─────────────────────────┐
│  Load Config YAML       │
│  - Parse with serde     │
│  - Validate schema      │
│  - Build Config struct  │
└──────┬──────────────────┘
       │
       ▼
┌─────────────────────────┐
│  Initialize Components  │
│  - Router               │
│  - TemplateEngine       │
│  - DataCache            │
│  - ToastManager         │
└──────┬──────────────────┘
       │
       ▼
┌─────────────────────────┐
│  Navigate to Start Page │
└──────┬──────────────────┘
       │
       ▼
┌─────────────────────────┐
│    Main Event Loop      │
└─────────────────────────┘
```

### 2. Page Navigation Flow

```
User presses Enter on selected row
           │
           ▼
┌─────────────────────────────────┐
│  Capture selected row data      │
│  Store in Context               │
└──────┬──────────────────────────┘
       │
       ▼
┌─────────────────────────────────┐
│  Resolve next page              │
│  - Check conditional routing    │
│  - Evaluate Tera conditions     │
└──────┬──────────────────────────┘
       │
       ▼
┌─────────────────────────────────┐
│  Push current frame to stack    │
│  - Page ID                      │
│  - Context                      │
│  - Scroll state                 │
└──────┬──────────────────────────┘
       │
       ▼
┌─────────────────────────────────┐
│  Fetch data for new page        │
│  (see Data Fetching Flow)       │
└──────┬──────────────────────────┘
       │
       ▼
┌─────────────────────────────────┐
│  Render new page                │
└─────────────────────────────────┘
```

### 3. Data Fetching Flow

```
Page requests data
       │
       ▼
┌─────────────────────────────────┐
│  Check cache                    │
│  - If cached & fresh, return    │
└──────┬──────────────────────────┘
       │ Cache miss
       ▼
┌─────────────────────────────────┐
│  Render command/URL template    │
│  - Substitute {{ variables }}   │
│  - Use current Context          │
└──────┬──────────────────────────┘
       │
       ▼
┌─────────────────────────────────┐
│  Execute data provider          │
│  - CLI: spawn process           │
│  - HTTP: make request           │
└──────┬──────────────────────────┘
       │
       ▼
┌─────────────────────────────────┐
│  Parse response (JSON/YAML)     │
└──────┬──────────────────────────┘
       │
       ▼
┌─────────────────────────────────┐
│  Apply JSONPath extraction      │
│  - Extract items array          │
└──────┬──────────────────────────┘
       │
       ▼
┌─────────────────────────────────┐
│  Transform & filter data        │
│  - Apply Tera transforms        │
│  - Apply view filters           │
└──────┬──────────────────────────┘
       │
       ▼
┌─────────────────────────────────┐
│  Cache result (if TTL set)      │
└──────┬──────────────────────────┘
       │
       ▼
┌─────────────────────────────────┐
│  Return to view renderer        │
└─────────────────────────────────┘
```

### 4. Action Execution Flow

```
User presses action key (e.g., 'd')
           │
           ▼
┌─────────────────────────────────┐
│  Lookup action by key           │
└──────┬──────────────────────────┘
       │
       ▼
┌─────────────────────────────────┐
│  Show confirmation (if set)     │
│  - Render confirm dialog        │
│  - Wait for y/n input           │
└──────┬──────────────────────────┘
       │ Confirmed
       ▼
┌─────────────────────────────────┐
│  Show loading toast             │
│  "Executing action..."          │
└──────┬──────────────────────────┘
       │
       ▼
┌─────────────────────────────────┐
│  Execute action                 │
│  - Render templates             │
│  - Execute CLI/HTTP/Lua         │
└──────┬──────────────────────────┘
       │
       ▼
┌─────────────────────────────────┐
│  Handle result                  │
│  - Success: show success toast  │
│  - Error: show error toast      │
└──────┬──────────────────────────┘
       │
       ▼
┌─────────────────────────────────┐
│  Refresh page (if configured)   │
└─────────────────────────────────┘
```

---

## Error Handling

### Error Types

```rust
#[derive(Debug, thiserror::Error)]
pub enum TermStackError {
    #[error("Config error: {0}")]
    Config(String),
    
    #[error("Validation error: {0}")]
    Validation(String),
    
    #[error("Data provider error: {0}")]
    DataProvider(String),
    
    #[error("Template error: {0}")]
    Template(String),
    
    #[error("Navigation error: {0}")]
    Navigation(String),
    
    #[error("Action execution error: {0}")]
    Action(String),
    
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    
    #[error("YAML error: {0}")]
    Yaml(#[from] serde_yaml::Error),
}
```

### Error Display Strategy

1. **Fatal Errors** (Config parse, validation):
   - Display error page with details
   - Show traceback in debug mode
   - Suggest fixes (e.g., "Invalid JSONPath at pages.projects.data.items")

2. **Runtime Errors** (Data fetch, action execution):
   - Show error toast (top-right, 5s)
   - Log to status bar
   - Allow retry

3. **Validation Warnings**:
   - Show warning toast
   - Continue execution

### Error Message Format

```
┌────────────────────────────────────────────┐
│ ❌ Configuration Error                      │
├────────────────────────────────────────────┤
│                                            │
│ Failed to parse YAML config:               │
│   File: raptor.yaml                        │
│   Line: 42                                 │
│                                            │
│ Error: Invalid JSONPath expression        │
│   Field: pages.resources.data.items       │
│   Value: "$.data[*"                       │
│   Expected: closing bracket ']'           │
│                                            │
│ Suggestion:                                │
│   Change "$.data[*" to "$.data[*]"        │
│                                            │
│ Press 'q' to quit, 'e' to edit config     │
└────────────────────────────────────────────┘
```

---

## Phase 1 Features

### Must Have (P0)

- [x] **Config System**
  - Load YAML config
  - Parse into typed structs (serde)
  - Validate schema
  - Default values

- [x] **Data Providers**
  - CLI provider (execute commands)
  - HTTP provider (GET requests)
  - JSON parsing
  - JSONPath extraction
  - Response caching (TTL)

- [x] **Template Engine**
  - Tera integration
  - Context management
  - Custom filters (timeago, filesizeformat, status_color)
  - Variable interpolation

- [x] **Navigation**
  - Page router
  - Navigation stack
  - Context passing (selected row data)
  - Back navigation

- [x] **Views**
  - Table view
    - Column configuration
    - Row selection
    - Scrolling
    - Sorting
  - Detail view
    - Key-value display
    - Sections
    - Scrolling
  - YAML view
    - Raw data display
    - Scrolling

- [x] **Input Handling**
  - Default keybindings
  - Custom keybindings
  - Search mode
  - Normal mode navigation

- [x] **UI Components**
  - Status bar
  - Toast notifications
  - Breadcrumb trail
  - Help overlay

- [x] **Actions**
  - CLI command execution
  - Confirmation dialogs
  - Success/error messages
  - Page refresh after action

- [x] **Error Handling**
  - Graceful error display
  - Error toast notifications
  - Config validation errors
  - Runtime error recovery

### Should Have (P1)

- [ ] **Advanced Data**
  - HTTP POST/PUT/DELETE
  - Custom headers
  - Request body templates
  - Multi-source data composition

- [ ] **Advanced Views**
  - Table grouping
  - Conditional row styling
  - Column width configuration
  - Detail view sections

- [ ] **Advanced Actions**
  - HTTP actions
  - Action conditions (show only if...)
  - Built-in actions (export, copy, etc.)

- [x] **Search & Filter**
  - [x] Column-specific search (`%col% term` syntax)
  - [x] Regex search (`!pattern` prefix)
  - [x] Case-sensitive toggle (Ctrl+I)
  - [ ] Fuzzy search
  - [ ] Search history

- [ ] **CLI Commands**
  - `termstack validate <config>` - Validate config
  - `termstack test <config> --page <page>` - Test data fetching
  - `termstack run <config>` - Run TUI
  - `termstack --help` - Show help

---

## Phase 2 Features

### Planned for Future

- [ ] **Log Streaming**
  - Stream view layout
  - Follow mode
  - Line wrapping
  - Syntax highlighting
  - Log filtering

- [ ] **Scripting**
  - Lua integration (mlua)
  - Script actions
  - Helper functions (exec, prompt, http)
  - Script debugging

- [ ] **Adapter System**
  - Plugin loader
  - Adapter manifest
  - Custom pages from adapters
  - Adapter actions
  - Adapter registry

- [ ] **Hot Reload**
  - Watch config file
  - Reload on change
  - Preserve navigation state
  - Notify user of reload

- [ ] **Multi-Pane Views**
  - Split layouts (horizontal/vertical)
  - Multiple views per page
  - Pane focus switching

- [ ] **Export & Scripting**
  - Export to JSON/CSV
  - Headless mode (no TUI)
  - Scriptable commands

- [ ] **Advanced UI**
  - Themes (Nord, Dracula, custom)
  - Custom colors per page
  - Icons & symbols
  - Progress bars

- [ ] **Performance**
  - Incremental rendering
  - Virtual scrolling for large tables
  - Background data refresh
  - Connection pooling

---

## Implementation Plan

### Week 1: Foundation

**Day 1-2: Project Setup & Config**
- [x] Initialize project structure
- [ ] Define Cargo.toml dependencies
- [ ] Create config schema structs
- [ ] Implement YAML loader
- [ ] Write config validator
- [ ] Add error types

**Day 3-4: Data Providers**
- [ ] Implement DataProvider trait
- [ ] CLI provider with process spawning
- [ ] HTTP provider with reqwest
- [ ] JSONPath extractor
- [ ] Basic caching layer

**Day 5-7: Template Engine & Navigation**
- [ ] Integrate Tera
- [ ] Implement custom filters
- [ ] Context management system
- [ ] Router implementation
- [ ] Navigation stack
- [ ] Template rendering in data providers

### Week 2: Views & Rendering

**Day 8-10: Table View**
- [ ] TableView struct
- [ ] Ratatui table rendering
- [ ] Row selection logic
- [ ] Scrolling (viewport)
- [ ] Sorting implementation
- [ ] Column styling

**Day 11-12: Detail & YAML Views**
- [ ] DetailView implementation
- [ ] Sections rendering
- [ ] YamlView implementation
- [ ] Syntax highlighting (basic)
- [ ] Scrolling for both views

**Day 13-14: UI Components**
- [ ] Status bar widget
- [ ] Toast manager
- [ ] Breadcrumb component
- [ ] Help overlay
- [ ] Layout management

### Week 3: Input & Actions

**Day 15-16: Input Handling**
- [ ] Default keybindings
- [ ] Custom keybinding loader
- [ ] Input handler with mode switching
- [ ] Search mode implementation
- [ ] Command mode (basic)

**Day 17-18: Action System**
- [ ] Action executor
- [ ] CLI action implementation
- [ ] Confirmation dialogs
- [ ] Success/error toasts
- [ ] Page refresh after action

**Day 19-21: Integration & Main Loop**
- [ ] App state machine
- [ ] Main event loop
- [ ] Async data fetching
- [ ] View state management
- [ ] Error recovery

### Week 4: Polish & Testing

**Day 22-23: CLI & Examples**
- [ ] CLI argument parsing (clap)
- [ ] `validate` command
- [ ] `test` command
- [ ] Example configs (raptor, k8s, docker)
- [ ] Documentation

**Day 24-26: Testing**
- [ ] Unit tests for core components
- [ ] Integration tests
- [ ] Config validation tests
- [ ] Error handling tests
- [ ] Manual testing with example configs

**Day 27-28: Bug Fixes & Documentation**
- [ ] Fix issues from testing
- [ ] Write README
- [ ] API documentation
- [ ] User guide
- [ ] Contributing guide

---

## Success Metrics

### Phase 1 Completion Criteria

1. **Config Loading**: Successfully parse and validate complex YAML configs
2. **Data Fetching**: Execute CLI commands and HTTP requests with template rendering
3. **Navigation**: Navigate through multi-level page hierarchies with context passing
4. **Views**: Render table, detail, and YAML views with proper scrolling
5. **Actions**: Execute CLI actions with confirmations and refresh
6. **UX**: Responsive input handling, search, and k9s-like navigation
7. **Examples**: Working example configs for at least 2 different use cases

### Performance Targets

- Config load time: < 100ms
- Page navigation: < 50ms (excluding data fetch)
- Data fetch + render: < 500ms (depends on external command)
- UI responsiveness: 60 FPS
- Memory usage: < 50MB for typical workloads

---

## Appendix

### A. Example Configs

See `examples/` directory:
- `raptor.yaml` - Multi-level resource navigation
- `kubernetes.yaml` - Kubernetes resource browser
- `docker.yaml` - Docker container manager

### B. JSONPath Reference

Common patterns:
- `$.field` - Top-level field
- `$.nested.field` - Nested field
- `$.array[*]` - All array items
- `$.array[0]` - First item
- `$.array[?(@.status == 'active')]` - Filtered items

### C. Tera Template Reference

Variables:
- `{{ variable }}` - Simple variable
- `{{ object.field }}` - Nested field
- `{{ array.0 }}` - Array access

Filters:
- `{{ value | upper }}` - Uppercase
- `{{ value | lower }}` - Lowercase
- `{{ value | truncate(length=20) }}` - Truncate
- `{{ value | timeago }}` - Time ago (custom)
- `{{ value | filesizeformat }}` - File size (custom)

Conditions:
- `{% if condition %}...{% endif %}`
- `{% if value == "active" %}...{% else %}...{% endif %}`

### D. Contribution Guidelines

See `CONTRIBUTING.md` (to be created)

---

**End of Specification Document**
