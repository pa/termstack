use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind};
use ratatui::{
    DefaultTerminal, Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table},
};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;

use crate::{
    action::executor::{ActionExecutor, ActionResult},
    config::{Config, View as ConfigView},
    data::{DataCache, JsonPathExtractor},
    error::Result,
    navigation::{NavigationContext, NavigationFrame, NavigationStack},
    template::{TemplateEngine, engine::TemplateContext},
};

pub struct App {
    config: Arc<Config>,
    running: bool,
    current_page: String,
    nav_stack: NavigationStack,
    nav_context: NavigationContext,
    template_engine: TemplateEngine,
    data_cache: DataCache,
    action_executor: ActionExecutor,

    // Current view state
    current_data: Vec<Value>,
    filtered_data: Vec<Value>,
    selected_index: usize,
    scroll_offset: usize,
    loading: bool,
    error_message: Option<String>,

    // Search/filter state
    search_mode: bool,
    search_query: String,

    // Confirmation dialogs
    show_quit_confirm: bool,
    action_confirm: Option<ActionConfirm>,

    // Action result message
    action_message: Option<ActionMessage>,

    // Auto-refresh timer
    last_refresh: std::time::Instant,
}

#[derive(Clone)]
struct ActionConfirm {
    action: crate::config::schema::Action,
    message: String,
}

#[derive(Clone)]
struct ActionMessage {
    message: String,
    is_error: bool,
}

impl App {
    pub fn new(config: Config) -> Result<Self> {
        let current_page = config.start.clone();
        let nav_context = NavigationContext::new().with_globals(config.globals.clone());
        let template_engine = TemplateEngine::new()?;
        let action_executor = ActionExecutor::new(Arc::new(template_engine.clone()));

        Ok(Self {
            config: Arc::new(config),
            running: false,
            current_page,
            nav_stack: NavigationStack::default(),
            nav_context,
            template_engine,
            data_cache: DataCache::new(),
            action_executor,
            current_data: Vec::new(),
            filtered_data: Vec::new(),
            selected_index: 0,
            scroll_offset: 0,
            loading: false,
            error_message: None,
            search_mode: false,
            search_query: String::new(),
            show_quit_confirm: false,
            action_confirm: None,
            action_message: None,
            last_refresh: std::time::Instant::now(),
        })
    }

    pub async fn run(mut self, mut terminal: DefaultTerminal) -> Result<()> {
        self.running = true;

        // Load initial page data
        self.load_current_page().await;

        while self.running {
            terminal.draw(|frame| self.render(frame))?;

            // Check for auto-refresh
            self.check_auto_refresh().await;

            if let Ok(true) = event::poll(std::time::Duration::from_millis(100)) {
                if let Event::Key(key) = event::read()? {
                    if key.kind == KeyEventKind::Press {
                        self.handle_key(key).await;
                    }
                }
            }
        }

        Ok(())
    }

    async fn load_current_page(&mut self) {
        self.loading = true;
        self.error_message = None;

        let page = match self.config.pages.get(&self.current_page) {
            Some(p) => p,
            None => {
                self.error_message = Some(format!("Page not found: {}", self.current_page));
                self.loading = false;
                return;
            }
        };

        // Fetch data
        match self.fetch_page_data(page).await {
            Ok(data) => {
                self.current_data = data;
                self.apply_sort_and_filter();
                self.selected_index = 0;
                self.scroll_offset = 0;
                self.loading = false;
                self.last_refresh = std::time::Instant::now();
            }
            Err(e) => {
                self.error_message = Some(format!("Failed to load data: {}", e));
                self.loading = false;
            }
        }
    }

    async fn check_auto_refresh(&mut self) {
        // Get refresh_interval for current page
        let page = match self.config.pages.get(&self.current_page) {
            Some(p) => p,
            None => return,
        };

        let refresh_interval = match &page.data {
            crate::config::DataSource::Single(single) => {
                if let Some(interval_str) = &single.refresh_interval {
                    humantime::parse_duration(interval_str).ok()
                } else {
                    None
                }
            }
            _ => None,
        };

        // If refresh_interval is set and expired, reload
        if let Some(interval) = refresh_interval {
            if self.last_refresh.elapsed() >= interval && !self.loading {
                self.load_current_page().await;
            }
        }
    }

    async fn fetch_page_data(&self, page: &crate::config::Page) -> Result<Vec<Value>> {
        use crate::config::DataSource;
        use crate::data::{CliProvider, DataProvider};

        let data_source = &page.data;

        match data_source {
            DataSource::Single(single) => {
                // Generate cache key
                let cache_key = self.generate_cache_key(&self.current_page, single)?;

                // Check cache first if TTL is set
                if single.cache.is_some() {
                    if let Some(cached_data) = self.data_cache.get(&cache_key).await {
                        if let Value::Array(arr) = cached_data {
                            return Ok(arr);
                        }
                    }
                }

                match single.source_type {
                    crate::config::DataSourceType::Cli => {
                        let command = single.command.as_ref().ok_or_else(|| {
                            crate::error::TermStackError::DataProvider(
                                "Missing command".to_string(),
                            )
                        })?;

                        // Render command and args with templates
                        let ctx = self.create_template_context(None);
                        let rendered_command = self.template_engine.render_string(command, &ctx)?;
                        let rendered_args: Result<Vec<String>> = single
                            .args
                            .iter()
                            .map(|arg| self.template_engine.render_string(arg, &ctx))
                            .collect();
                        let rendered_args = rendered_args?;

                        let mut provider = CliProvider::new(rendered_command)
                            .with_args(rendered_args)
                            .with_shell(single.shell);

                        if let Some(timeout_str) = &single.timeout {
                            if let Ok(duration) = humantime::parse_duration(timeout_str) {
                                provider = provider.with_timeout(duration);
                            }
                        }

                        let data_context = crate::data::provider::DataContext {
                            globals: self.nav_context.globals.clone(),
                            page_contexts: self.nav_context.page_contexts.clone(),
                        };

                        let result = provider.fetch(&data_context).await?;

                        // Extract items using JSONPath
                        let items = if let Some(items_path) = &single.items {
                            let extractor = JsonPathExtractor::new(items_path)?;
                            extractor.extract(&result)?
                        } else {
                            vec![result]
                        };

                        // Cache the result if TTL is set
                        if let Some(cache_str) = &single.cache {
                            if let Ok(duration) = humantime::parse_duration(cache_str) {
                                self.data_cache
                                    .set(cache_key, Value::Array(items.clone()), duration)
                                    .await;
                            }
                        }

                        Ok(items)
                    }
                    _ => Err(crate::error::TermStackError::DataProvider(
                        "HTTP and Stream sources not yet implemented".to_string(),
                    )),
                }
            }
            DataSource::Multi(_) => Err(crate::error::TermStackError::DataProvider(
                "Multi-source not yet implemented".to_string(),
            )),
        }
    }

    fn generate_cache_key(
        &self,
        page_id: &str,
        source: &crate::config::SingleDataSource,
    ) -> Result<String> {
        // Create a cache key from page ID, command, args, and context
        let ctx = self.create_template_context(None);

        let empty_string = String::new();
        let command = source.command.as_ref().unwrap_or(&empty_string);
        let rendered_command = self.template_engine.render_string(command, &ctx)?;

        let rendered_args: Result<Vec<String>> = source
            .args
            .iter()
            .map(|arg| self.template_engine.render_string(arg, &ctx))
            .collect();
        let rendered_args = rendered_args?;

        // Create a deterministic key
        Ok(format!(
            "{}:{}:{}",
            page_id,
            rendered_command,
            rendered_args.join(":")
        ))
    }

    fn create_template_context(&self, current_row: Option<&Value>) -> TemplateContext {
        let mut ctx = TemplateContext::new().with_globals(self.nav_context.globals.clone());

        for (page, data) in &self.nav_context.page_contexts {
            ctx = ctx.with_page_context(page.clone(), data.clone());
        }

        if let Some(row) = current_row {
            ctx = ctx.with_current(row.clone());
        }

        ctx
    }

    async fn handle_key(&mut self, key: KeyEvent) {
        // Handle action confirmation dialog
        if let Some(confirm) = &self.action_confirm {
            match key.code {
                KeyCode::Char('y') | KeyCode::Char('Y') => {
                    let action = confirm.action.clone();
                    self.action_confirm = None;
                    self.execute_action(&action).await;
                }
                KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                    self.action_confirm = None;
                }
                _ => {}
            }
            return;
        }

        // Handle quit confirmation dialog
        if self.show_quit_confirm {
            match key.code {
                KeyCode::Char('y') | KeyCode::Char('Y') => {
                    self.running = false;
                }
                KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                    self.show_quit_confirm = false;
                }
                _ => {}
            }
            return;
        }

        // Handle search mode
        if self.search_mode {
            match key.code {
                KeyCode::Char(c) => {
                    self.update_search_query(c);
                    return;
                }
                KeyCode::Backspace => {
                    self.backspace_search_query();
                    return;
                }
                KeyCode::Esc | KeyCode::Enter => {
                    self.toggle_search_mode();
                    return;
                }
                _ => return,
            }
        }

        // Clear action message on any key
        if self.action_message.is_some() {
            self.action_message = None;
        }

        // Normal key handling
        match key.code {
            KeyCode::Char('q') => {
                if self.nav_stack.is_empty() {
                    // On start page, show confirmation
                    self.show_quit_confirm = true;
                } else {
                    self.go_back().await;
                }
            }
            KeyCode::Esc => {
                if !self.nav_stack.is_empty() {
                    self.go_back().await;
                }
            }
            KeyCode::Char('j') | KeyCode::Down => self.move_down(),
            KeyCode::Char('k') | KeyCode::Up => self.move_up(),
            KeyCode::Char('g') => self.move_top(),
            KeyCode::Char('G') => self.move_bottom(),
            KeyCode::Char('r') => self.load_current_page().await,
            KeyCode::Char('/') => self.toggle_search_mode(),
            KeyCode::Enter => self.navigate_next().await,
            KeyCode::Char(c) => {
                // Check for action keybindings
                self.handle_action_key(c).await;
            }
            _ => {}
        }
    }

    async fn handle_action_key(&mut self, key: char) {
        // Find matching action and clone it to avoid borrow issues
        let action_to_execute = {
            let page = match self.config.pages.get(&self.current_page) {
                Some(p) => p,
                None => return,
            };

            let actions = match &page.actions {
                Some(a) => a,
                None => return,
            };

            // Find action with matching key
            actions
                .iter()
                .find(|action| action.key == key.to_string())
                .cloned()
        };

        if let Some(action) = action_to_execute {
            // Check if confirmation is needed
            if let Some(confirm_msg) = &action.confirm {
                // Render confirmation message with context
                let rendered_msg = self
                    .template_engine
                    .render_string(
                        confirm_msg,
                        &self.create_template_context(self.get_selected_row()),
                    )
                    .unwrap_or_else(|_| confirm_msg.clone());

                self.action_confirm = Some(ActionConfirm {
                    action: action.clone(),
                    message: rendered_msg,
                });
            } else {
                // Execute immediately
                self.execute_action(&action).await;
            }
        }
    }

    fn get_selected_row(&self) -> Option<&Value> {
        self.filtered_data.get(self.selected_index)
    }

    fn create_template_context_map(&self) -> std::collections::HashMap<String, Value> {
        let mut context = std::collections::HashMap::new();

        // Add globals
        for (key, value) in &self.nav_context.globals {
            context.insert(key.clone(), value.clone());
        }

        // Add page contexts
        for (page, data) in &self.nav_context.page_contexts {
            context.insert(page.clone(), data.clone());
        }

        // Add current row data
        if let Some(row) = self.get_selected_row() {
            context.insert("row".to_string(), row.clone());
            context.insert("value".to_string(), row.clone());

            // Flatten current object fields
            if let Value::Object(map) = row {
                for (key, value) in map {
                    context.insert(key.clone(), value.clone());
                }
            }
        }

        context
    }

    async fn execute_action(&mut self, action: &crate::config::schema::Action) {
        // Create template context
        let context = self.create_template_context_map();

        // Execute action
        match self.action_executor.execute(action, &context).await {
            Ok(ActionResult::Success(msg)) => {
                self.action_message = Some(ActionMessage {
                    message: msg.unwrap_or_else(|| "Action completed successfully".to_string()),
                    is_error: false,
                });
            }
            Ok(ActionResult::Error(msg)) => {
                self.action_message = Some(ActionMessage {
                    message: msg,
                    is_error: true,
                });
            }
            Ok(ActionResult::Refresh) => {
                // Reload the page
                self.load_current_page().await;
            }
            Ok(ActionResult::Navigate(page, context_map)) => {
                // Navigate to the specified page with context
                self.navigate_to_page(&page, context_map).await;
            }
            Err(e) => {
                self.action_message = Some(ActionMessage {
                    message: format!("Action failed: {}", e),
                    is_error: true,
                });
            }
        }
    }

    async fn navigate_to_page(
        &mut self,
        target_page: &str,
        context_map: std::collections::HashMap<String, String>,
    ) {
        // Get the current selected row
        let selected_row = self.get_selected_row().cloned();

        // Render context values with template engine
        let mut rendered_context = std::collections::HashMap::new();
        if let Some(row) = &selected_row {
            let template_ctx = self.create_template_context(Some(row));

            for (key, template) in context_map {
                match self.template_engine.render_string(&template, &template_ctx) {
                    Ok(rendered) => {
                        rendered_context.insert(key, serde_json::json!(rendered));
                    }
                    Err(e) => {
                        self.error_message = Some(format!("Failed to render context: {}", e));
                        return;
                    }
                }
            }
        }

        // Save current state to navigation stack
        let frame = NavigationFrame {
            page_id: self.current_page.clone(),
            context: HashMap::new(),
            scroll_offset: self.scroll_offset,
            selected_index: self.selected_index,
        };
        self.nav_stack.push(frame);

        // Update navigation context with new data
        for (key, value) in rendered_context {
            self.nav_context.page_contexts.insert(key, value);
        }

        // Navigate to new page
        self.current_page = target_page.to_string();
        self.selected_index = 0;
        self.scroll_offset = 0;

        // Load new page data
        self.load_current_page().await;
    }

    fn move_down(&mut self) {
        if self.filtered_data.is_empty() {
            return;
        }
        if self.selected_index < self.filtered_data.len() - 1 {
            self.selected_index += 1;
        }
    }

    fn move_up(&mut self) {
        if self.selected_index > 0 {
            self.selected_index -= 1;
        }
    }

    fn move_top(&mut self) {
        self.selected_index = 0;
    }

    fn move_bottom(&mut self) {
        if !self.filtered_data.is_empty() {
            self.selected_index = self.filtered_data.len() - 1;
        }
    }

    async fn go_back(&mut self) {
        if let Some(frame) = self.nav_stack.pop() {
            self.current_page = frame.page_id;
            self.selected_index = frame.selected_index;
            self.scroll_offset = frame.scroll_offset;
            self.load_current_page().await;
        }
    }

    async fn navigate_next(&mut self) {
        let page = match self.config.pages.get(&self.current_page) {
            Some(p) => p,
            None => return,
        };

        let next_nav = match &page.next {
            Some(nav) => nav,
            None => return,
        };

        use crate::config::Navigation;
        let (next_page, context_map) = match next_nav {
            Navigation::Simple(simple) => (&simple.page, &simple.context),
            Navigation::Conditional(conditionals) => {
                // Find first matching condition or default
                let mut found = None;
                for cond in conditionals {
                    if cond.default {
                        found = Some((&cond.page, &cond.context));
                        break;
                    }
                    // TODO: Evaluate conditions
                }
                match found {
                    Some(f) => f,
                    None => return,
                }
            }
        };

        // Save current frame to navigation stack
        let mut frame = NavigationFrame::new(self.current_page.clone());
        frame.selected_index = self.selected_index;
        frame.scroll_offset = self.scroll_offset;
        self.nav_stack.push(frame);

        // Capture context from selected row
        if let Some(selected_row) = self.filtered_data.get(self.selected_index) {
            for (key, json_path) in context_map {
                if let Ok(extractor) = JsonPathExtractor::new(json_path) {
                    if let Ok(Some(value)) = extractor.extract_single(selected_row) {
                        self.nav_context.set_page_context(key.clone(), value);
                    }
                }
            }

            // Also store the entire selected row under the current page name
            self.nav_context
                .set_page_context(self.current_page.clone(), selected_row.clone());
        }

        // Navigate to next page
        self.current_page = next_page.clone();
        self.load_current_page().await;
    }

    fn render(&mut self, frame: &mut Frame) {
        let area = frame.area();

        let chunks = Layout::vertical([
            Constraint::Length(3), // Header
            Constraint::Min(0),    // Content
            Constraint::Length(4), // Status bar (now 2 lines + border)
        ])
        .split(area);

        self.render_header(frame, chunks[0]);
        self.render_content(frame, chunks[1]);
        self.render_statusbar(frame, chunks[2]);

        // Render action message if present
        if let Some(msg) = &self.action_message {
            self.render_action_message(frame, area, msg);
        }

        // Render action confirmation dialog on top if active
        if let Some(confirm) = &self.action_confirm {
            self.render_action_confirm(frame, area, confirm);
        }

        // Render quit confirmation dialog on top if active
        if self.show_quit_confirm {
            self.render_quit_confirm(frame, area);
        }
    }

    fn render_header(&self, frame: &mut Frame, area: Rect) {
        let page = self.config.pages.get(&self.current_page);
        let title = if let Some(page) = page {
            // Try to render title template
            let ctx = self.create_template_context(None);
            self.template_engine
                .render_string(&page.title, &ctx)
                .unwrap_or_else(|_| page.title.clone())
        } else {
            self.current_page.clone()
        };

        let breadcrumb = if self.nav_stack.is_empty() {
            title.clone()
        } else {
            format!("{} levels deep > {}", self.nav_stack.len(), title)
        };

        let header = Paragraph::new(Line::from(vec![
            Span::styled(
                &self.config.app.name,
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" | "),
            Span::styled(breadcrumb, Style::default().fg(Color::Yellow)),
        ]))
        .block(Block::default().borders(Borders::ALL));

        frame.render_widget(header, area);
    }

    fn render_content(&mut self, frame: &mut Frame, area: Rect) {
        if self.loading {
            let loading = Paragraph::new("Loading...")
                .style(Style::default().fg(Color::Yellow))
                .block(Block::default().borders(Borders::ALL).title("Content"));
            frame.render_widget(loading, area);
            return;
        }

        if let Some(error) = &self.error_message {
            let error_widget = Paragraph::new(error.as_str())
                .style(Style::default().fg(Color::Red))
                .block(Block::default().borders(Borders::ALL).title("Error"));
            frame.render_widget(error_widget, area);
            return;
        }

        let page = match self.config.pages.get(&self.current_page) {
            Some(p) => p,
            None => return,
        };

        match &page.view {
            ConfigView::Table(table_view) => {
                let table_view = table_view.clone();
                self.render_table(frame, area, &table_view);
            }
            _ => {
                let msg = Paragraph::new("View type not yet implemented")
                    .block(Block::default().borders(Borders::ALL).title("Content"));
                frame.render_widget(msg, area);
            }
        }
    }

    fn render_table(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        table_config: &crate::config::TableView,
    ) {
        if self.filtered_data.is_empty() {
            let empty = Paragraph::new("No data")
                .block(Block::default().borders(Borders::ALL).title("Table"));
            frame.render_widget(empty, area);
            return;
        }

        // Build header
        let header_cells: Vec<Cell> = table_config
            .columns
            .iter()
            .map(|col| {
                Cell::from(col.display.clone()).style(
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                )
            })
            .collect();
        let header = Row::new(header_cells).height(1);

        // Build rows
        let _ctx = self.create_template_context(None);
        let rows: Vec<Row> = self
            .filtered_data
            .iter()
            .enumerate()
            .map(|(idx, item)| {
                let cells: Vec<Cell> = table_config
                    .columns
                    .iter()
                    .map(|col| {
                        // Extract value using JSONPath
                        let value_str = if let Ok(extractor) = JsonPathExtractor::new(&col.path) {
                            if let Ok(Some(value)) = extractor.extract_single(item) {
                                // Apply transform if present
                                if let Some(transform) = &col.transform {
                                    let row_ctx = self.create_template_context(Some(&value));
                                    self.template_engine
                                        .render_string(transform, &row_ctx)
                                        .unwrap_or_else(|_| value_to_string(&value))
                                } else {
                                    value_to_string(&value)
                                }
                            } else {
                                "".to_string()
                            }
                        } else {
                            "".to_string()
                        };

                        Cell::from(value_str)
                    })
                    .collect();

                let mut row = Row::new(cells);
                if idx == self.selected_index {
                    row = row.style(
                        Style::default()
                            .bg(Color::DarkGray)
                            .add_modifier(Modifier::BOLD),
                    );
                }
                row
            })
            .collect();

        // Calculate column widths
        let widths: Vec<Constraint> = table_config
            .columns
            .iter()
            .map(|col| {
                if let Some(width) = col.width {
                    Constraint::Length(width)
                } else {
                    Constraint::Percentage((100 / table_config.columns.len()) as u16)
                }
            })
            .collect();

        let table = Table::new(rows, widths)
            .header(header)
            .block(Block::default().borders(Borders::ALL).title("Table"))
            .row_highlight_style(Style::default().bg(Color::DarkGray));

        frame.render_widget(table, area);
    }

    fn render_statusbar(&self, frame: &mut Frame, area: Rect) {
        // Build navigation shortcuts (always shown)
        let nav_shortcuts = if self.current_data.is_empty() {
            "q/ESC: Quit  |  r: Refresh"
        } else {
            "j/k: Move  |  g/G: Top/Bottom  |  Enter: Select  |  ESC: Back  |  r: Refresh  |  q: Quit"
        };

        let row_info = if self.search_mode {
            format!(
                "Search: {} | {}/{}",
                self.search_query,
                self.selected_index + 1,
                self.filtered_data.len()
            )
        } else if !self.search_query.is_empty() {
            format!(
                "Filtered: {}/{} | Row {}/{}",
                self.filtered_data.len(),
                self.current_data.len(),
                self.selected_index + 1,
                self.filtered_data.len()
            )
        } else {
            format!(
                "Row {}/{}",
                self.selected_index + 1,
                self.filtered_data.len()
            )
        };

        let nav_line = Line::from(vec![
            Span::styled(
                row_info,
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" | "),
            Span::styled(nav_shortcuts, Style::default().fg(Color::White)),
        ]);

        // Build action shortcuts (if available)
        let action_line = if let Some(page) = self.config.pages.get(&self.current_page) {
            if let Some(actions) = &page.actions {
                if !actions.is_empty() {
                    let action_shortcuts: Vec<Span> = actions
                        .iter()
                        .flat_map(|a| {
                            vec![
                                Span::styled(
                                    format!("{}", a.key),
                                    Style::default()
                                        .fg(Color::Yellow)
                                        .add_modifier(Modifier::BOLD),
                                ),
                                Span::raw(format!(": {}  ", a.name)),
                            ]
                        })
                        .collect();

                    let mut spans = vec![Span::styled(
                        "Actions: ",
                        Style::default()
                            .fg(Color::Green)
                            .add_modifier(Modifier::BOLD),
                    )];
                    spans.extend(action_shortcuts);
                    Line::from(spans)
                } else {
                    Line::from("")
                }
            } else {
                Line::from("")
            }
        } else {
            Line::from("")
        };

        let status = Paragraph::new(vec![nav_line, action_line])
            .style(Style::default().fg(Color::White))
            .block(Block::default().borders(Borders::ALL).title("Status"));

        frame.render_widget(status, area);
    }

    fn render_action_message(&self, frame: &mut Frame, area: Rect, msg: &ActionMessage) {
        use ratatui::layout::Alignment;

        let color = if msg.is_error {
            Color::Red
        } else {
            Color::Green
        };

        // Calculate dynamic width (between 40 and 80% of screen, min 30, max 100)
        let max_width = ((area.width as f32 * 0.8) as u16)
            .min(area.width.saturating_sub(4))
            .max(30);

        // Word wrap the message to fit within the width
        // Account for borders (2 chars) and padding (2 chars)
        let content_width = max_width.saturating_sub(4) as usize;
        let wrapped_lines = Self::wrap_text(&msg.message, content_width);

        // Calculate height based on wrapped lines (add padding + borders)
        let content_height = wrapped_lines.len() as u16;
        let msg_height = (content_height + 4).min(area.height.saturating_sub(4)); // +4 for padding and borders

        // Center the message box
        let msg_width = max_width;
        let msg_x = (area.width.saturating_sub(msg_width)) / 2;
        let msg_y = 2;

        let msg_area = Rect {
            x: msg_x,
            y: msg_y,
            width: msg_width,
            height: msg_height,
        };

        // Build the message text with wrapped lines
        let mut message_lines = vec![Line::from("")]; // Top padding
        for line in wrapped_lines {
            message_lines.push(Line::from(Span::styled(
                line,
                Style::default().fg(color).add_modifier(Modifier::BOLD),
            )));
        }
        message_lines.push(Line::from("")); // Bottom padding

        let message_box = Paragraph::new(message_lines)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(color))
                    .title(if msg.is_error { "Error" } else { "Success" }),
            )
            .alignment(Alignment::Center)
            .wrap(ratatui::widgets::Wrap { trim: true });

        frame.render_widget(message_box, msg_area);
    }

    fn wrap_text(text: &str, max_width: usize) -> Vec<String> {
        let mut lines = Vec::new();
        let words: Vec<&str> = text.split_whitespace().collect();
        let mut current_line = String::new();

        for word in words {
            // If the word itself is longer than max_width, split it
            if word.len() > max_width {
                if !current_line.is_empty() {
                    lines.push(current_line.clone());
                    current_line.clear();
                }
                // Split long word into chunks
                for chunk in word.chars().collect::<Vec<_>>().chunks(max_width) {
                    lines.push(chunk.iter().collect());
                }
                continue;
            }

            let potential_line = if current_line.is_empty() {
                word.to_string()
            } else {
                format!("{} {}", current_line, word)
            };

            if potential_line.len() <= max_width {
                current_line = potential_line;
            } else {
                if !current_line.is_empty() {
                    lines.push(current_line);
                }
                current_line = word.to_string();
            }
        }

        if !current_line.is_empty() {
            lines.push(current_line);
        }

        // Return at least one line even if empty
        if lines.is_empty() {
            lines.push(String::new());
        }

        lines
    }

    fn render_action_confirm(&self, frame: &mut Frame, area: Rect, confirm: &ActionConfirm) {
        use ratatui::layout::Alignment;

        // Create a centered popup
        let popup_width = 60.min(area.width.saturating_sub(4));
        let popup_height = 9;
        let popup_x = (area.width.saturating_sub(popup_width)) / 2;
        let popup_y = (area.height.saturating_sub(popup_height)) / 2;

        let popup_area = Rect {
            x: popup_x,
            y: popup_y,
            width: popup_width,
            height: popup_height,
        };

        // Clear the background
        let clear_block = Block::default().style(Style::default().bg(Color::Black));
        frame.render_widget(clear_block, popup_area);

        // Render the confirmation dialog
        let dialog_text = vec![
            Line::from(""),
            Line::from(Span::styled(
                &confirm.message,
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(Span::styled(
                format!("Action: {}", confirm.action.name),
                Style::default().fg(Color::Cyan),
            )),
            Line::from(""),
            Line::from(Span::raw("Press 'y' to confirm, 'n' or ESC to cancel")),
            Line::from(""),
        ];

        let dialog = Paragraph::new(dialog_text)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Yellow))
                    .title("Confirm Action"),
            )
            .alignment(Alignment::Center);

        frame.render_widget(dialog, popup_area);
    }

    fn render_quit_confirm(&self, frame: &mut Frame, area: Rect) {
        use ratatui::layout::Alignment;

        // Create a centered popup
        let popup_width = 50;
        let popup_height = 7;
        let popup_x = (area.width.saturating_sub(popup_width)) / 2;
        let popup_y = (area.height.saturating_sub(popup_height)) / 2;

        let popup_area = Rect {
            x: popup_x,
            y: popup_y,
            width: popup_width,
            height: popup_height,
        };

        // Clear the background
        let clear_block = Block::default().style(Style::default().bg(Color::Black));
        frame.render_widget(clear_block, popup_area);

        // Render the confirmation dialog
        let dialog_text = vec![
            Line::from(""),
            Line::from(Span::styled(
                "Quit TermStack?",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(Span::raw("Press 'y' to quit, 'n' or ESC to cancel")),
            Line::from(""),
        ];

        let dialog = Paragraph::new(dialog_text)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Yellow))
                    .title("Confirm"),
            )
            .alignment(Alignment::Center);

        frame.render_widget(dialog, popup_area);
    }

    fn apply_sort_and_filter(&mut self) {
        // Start with all data
        let mut data = self.current_data.clone();

        // Apply search filter if active
        if !self.search_query.is_empty() {
            data = self.filter_data(&data);
        }

        // Apply sorting if configured
        if let Some(page) = self.config.pages.get(&self.current_page) {
            if let ConfigView::Table(table_view) = &page.view {
                if let Some(sort_config) = &table_view.sort {
                    data = self.sort_data(&data, sort_config);
                }
            }
        }

        self.filtered_data = data;
    }

    fn filter_data(&self, data: &[Value]) -> Vec<Value> {
        let query = self.search_query.to_lowercase();

        data.iter()
            .filter(|item| {
                // Search through all string values in the item
                self.item_matches_query(item, &query)
            })
            .cloned()
            .collect()
    }

    fn item_matches_query(&self, item: &Value, query: &str) -> bool {
        match item {
            Value::String(s) => s.to_lowercase().contains(query),
            Value::Number(n) => n.to_string().contains(query),
            Value::Bool(b) => b.to_string().contains(query),
            Value::Array(arr) => arr.iter().any(|v| self.item_matches_query(v, query)),
            Value::Object(map) => map.values().any(|v| self.item_matches_query(v, query)),
            Value::Null => false,
        }
    }

    fn sort_data(
        &self,
        data: &[Value],
        sort_config: &crate::config::schema::TableSort,
    ) -> Vec<Value> {
        use crate::config::schema::SortOrder;
        use crate::data::JsonPathExtractor;

        let mut sorted = data.to_vec();

        // Create extractor once for efficiency
        let extractor = match JsonPathExtractor::new(&sort_config.column) {
            Ok(ext) => ext,
            Err(_) => return sorted, // Return unsorted if path is invalid
        };

        sorted.sort_by(|a, b| {
            let a_val = extractor.extract_single(a);
            let b_val = extractor.extract_single(b);

            let cmp = match (&a_val, &b_val) {
                (Ok(Some(av)), Ok(Some(bv))) => Self::compare_values(av, bv),
                (Ok(Some(_)), Ok(None)) => std::cmp::Ordering::Less,
                (Ok(None), Ok(Some(_))) => std::cmp::Ordering::Greater,
                _ => std::cmp::Ordering::Equal,
            };

            match sort_config.order {
                SortOrder::Asc => cmp,
                SortOrder::Desc => cmp.reverse(),
            }
        });

        sorted
    }

    fn compare_values(a: &Value, b: &Value) -> std::cmp::Ordering {
        use std::cmp::Ordering;

        match (a, b) {
            (Value::String(a), Value::String(b)) => a.cmp(b),
            (Value::Number(a), Value::Number(b)) => {
                if let (Some(a_f), Some(b_f)) = (a.as_f64(), b.as_f64()) {
                    a_f.partial_cmp(&b_f).unwrap_or(Ordering::Equal)
                } else {
                    Ordering::Equal
                }
            }
            (Value::Bool(a), Value::Bool(b)) => a.cmp(b),
            (Value::Null, Value::Null) => Ordering::Equal,
            (Value::Null, _) => Ordering::Less,
            (_, Value::Null) => Ordering::Greater,
            _ => value_to_string(a).cmp(&value_to_string(b)),
        }
    }

    fn toggle_search_mode(&mut self) {
        self.search_mode = !self.search_mode;
    }

    fn clear_search(&mut self) {
        self.search_query.clear();
        self.apply_sort_and_filter();
        self.selected_index = 0;
    }

    fn update_search_query(&mut self, c: char) {
        self.search_query.push(c);
        self.apply_sort_and_filter();
        self.selected_index = 0;
    }

    fn backspace_search_query(&mut self) {
        self.search_query.pop();
        self.apply_sort_and_filter();
        self.selected_index = 0;
    }
}

fn value_to_string(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Null => "null".to_string(),
        Value::Array(arr) => format!("[{} items]", arr.len()),
        Value::Object(_) => "{...}".to_string(),
    }
}
