use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind};
use ratatui::{
    DefaultTerminal, Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table},
};
use serde_json::Value;
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use tokio::sync::mpsc;

use crate::{
    action::executor::{ActionExecutor, ActionResult},
    config::{Config, View as ConfigView},
    data::{JsonPathExtractor, StreamMessage},
    error::Result,
    globals,
    navigation::{NavigationContext, NavigationFrame, NavigationStack},
    template::engine::TemplateContext,
};
use regex::Regex;

/// Global search state that works across all views
#[derive(Debug, Clone)]
struct GlobalSearch {
    /// Whether search input is active
    active: bool,
    /// The search query string
    query: String,
    /// Whether the filter is applied (search was confirmed)
    filter_active: bool,
    /// Compiled regex pattern (cached)
    regex_pattern: Option<Regex>,
    /// Whether to use case-sensitive search
    case_sensitive: bool,
}

impl Default for GlobalSearch {
    fn default() -> Self {
        Self {
            active: false,
            query: String::new(),
            filter_active: false,
            regex_pattern: None,
            case_sensitive: false,
        }
    }
}

impl GlobalSearch {
    /// Compile the query into a regex pattern
    fn compile_pattern(&mut self) {
        if self.query.is_empty() {
            self.regex_pattern = None;
            return;
        }

        // Check if query starts with '!' for regex mode
        let pattern_str = if self.query.starts_with('!') {
            // Regex mode: use query after '!'
            self.query[1..].to_string()
        } else {
            // Literal mode: escape special regex characters
            regex::escape(&self.query)
        };

        // Build regex with case sensitivity
        let regex_result = if self.case_sensitive {
            Regex::new(&pattern_str)
        } else {
            Regex::new(&format!("(?i){}", pattern_str))
        };

        self.regex_pattern = regex_result.ok();
    }

    /// Test if a string matches the search pattern
    fn matches(&self, text: &str) -> bool {
        if !self.filter_active || self.query.is_empty() {
            return true; // No filter, everything matches
        }

        match &self.regex_pattern {
            Some(regex) => regex.is_match(text),
            None => true, // Invalid regex, show everything
        }
    }

    /// Activate search mode
    fn activate(&mut self) {
        self.active = true;
    }

    /// Deactivate and apply filter
    fn apply(&mut self) {
        self.active = false;
        self.filter_active = !self.query.is_empty();
        self.compile_pattern();
    }

    /// Cancel search without applying
    fn cancel(&mut self) {
        self.active = false;
        self.query.clear();
        self.filter_active = false;
        self.regex_pattern = None;
    }

    /// Clear the search filter
    fn clear(&mut self) {
        self.query.clear();
        self.filter_active = false;
        self.regex_pattern = None;
    }

    /// Add character to query
    fn push_char(&mut self, c: char) {
        self.query.push(c);
    }

    /// Remove last character from query
    fn pop_char(&mut self) {
        self.query.pop();
    }

    /// Toggle case sensitivity
    fn toggle_case_sensitive(&mut self) {
        self.case_sensitive = !self.case_sensitive;
        if self.filter_active {
            self.compile_pattern();
        }
    }
}

pub struct App {
    running: bool,
    current_page: String,
    nav_stack: NavigationStack,
    nav_context: NavigationContext,
    action_executor: ActionExecutor,

    // Current view state
    current_data: Vec<Value>,
    filtered_data: Vec<Value>,
    selected_index: usize,
    scroll_offset: usize,
    table_state: ratatui::widgets::TableState,
    loading: bool,
    error_message: Option<String>,

    // Global search (works across all views)
    global_search: GlobalSearch,

    // Confirmation dialogs
    show_quit_confirm: bool,
    action_confirm: Option<ActionConfirm>,

    // Action result message
    action_message: Option<ActionMessage>,

    // Auto-refresh timer
    last_refresh: std::time::Instant,

    // Stream state
    stream_active: bool,
    stream_paused: bool,
    stream_buffer: VecDeque<String>,
    stream_frozen_snapshot: VecDeque<String>, // Frozen snapshot when paused
    stream_receiver: Option<mpsc::Receiver<StreamMessage>>,
    stream_status: StreamStatus,

    // Logs view settings
    logs_follow: bool,
    logs_wrap: bool,
    logs_horizontal_scroll: usize,

    // Action mode (prefix key pattern)
    action_mode: bool,

    // UI state
    needs_clear: bool,
    needs_render: bool,

    // Data refresh watcher
    refresh_receiver: Option<mpsc::Receiver<RefreshMessage>>,
}

#[derive(Debug)]
struct RefreshMessage {
    page_name: String,
    data: Vec<Value>,
}

#[derive(Clone)]
struct ActionConfirm {
    action: crate::config::schema::Action,
    message: String,
}

#[derive(Clone)]
struct ActionMessage {
    message: String,
    message_type: MessageType,
    timestamp: std::time::Instant,
}

#[derive(Clone, Copy, PartialEq)]
#[allow(dead_code)]
enum MessageType {
    Success,
    Error,
    Info,
    Warning,
}

#[derive(Debug, Clone, PartialEq)]
enum StreamStatus {
    Idle,
    Connected,
    Streaming,
    Stopped,
    Error(String),
}

impl App {
    pub fn new(config: Config) -> Result<Self> {
        let current_page = config.start.clone();
        let nav_context = NavigationContext::new().with_globals(config.globals.clone());
        let action_executor = ActionExecutor::new(Arc::new(globals::template_engine().clone()));

        Ok(Self {
            running: false,
            current_page,
            nav_stack: NavigationStack::default(),
            nav_context,
            action_executor,
            current_data: Vec::new(),
            filtered_data: Vec::new(),
            selected_index: 0,
            scroll_offset: 0,
            table_state: ratatui::widgets::TableState::default(),
            loading: false,
            error_message: None,
            global_search: GlobalSearch::default(),
            show_quit_confirm: false,
            action_confirm: None,
            action_message: None,
            last_refresh: std::time::Instant::now(),
            stream_active: false,
            stream_paused: false,
            stream_buffer: VecDeque::new(),
            stream_frozen_snapshot: VecDeque::new(),
            stream_receiver: None,
            stream_status: StreamStatus::Idle,
            logs_follow: true,
            logs_wrap: true,
            logs_horizontal_scroll: 0,
            action_mode: false,
            needs_clear: false,
            needs_render: true, // Initial render needed
            refresh_receiver: None,
        })
    }

    pub async fn run(mut self, mut terminal: DefaultTerminal) -> Result<()> {
        self.running = true;

        // Load initial page data
        self.load_current_page().await;

        while self.running {
            if self.needs_clear {
                terminal.clear()?;
                self.needs_clear = false;
            }

            // Check for refresh updates from background watcher
            self.check_refresh_updates();

            // Check for stream updates
            self.check_stream_updates();

            // Auto-dismiss notifications after 3 seconds
            if let Some(msg) = &self.action_message {
                if msg.timestamp.elapsed() > std::time::Duration::from_secs(3) {
                    self.action_message = None;
                    self.needs_render = true;
                }
            }

            // Only render if needed (data changed, user input, etc.)
            if self.needs_render {
                // Update table state to match selected_index
                self.table_state.select(Some(self.selected_index));

                terminal.draw(|frame| self.render(frame))?;
                self.needs_render = false;
            }

            // Poll for user input with timeout
            if let Ok(true) = event::poll(std::time::Duration::from_millis(100)) {
                if let Event::Key(key) = event::read()? {
                    if key.kind == KeyEventKind::Press {
                        self.handle_key(key).await;
                        // Don't auto-render on every key press - let handlers decide
                        // This allows pause mode to truly freeze the display
                    }
                }
            }
        }

        Ok(())
    }

    async fn load_current_page(&mut self) {
        self.loading = true;
        self.error_message = None;

        // Stop any active stream from previous page
        self.stop_stream();

        let page = match globals::config().pages.get(&self.current_page).cloned() {
            Some(p) => p,
            None => {
                self.error_message = Some(format!("Page not found: {}", self.current_page));
                self.loading = false;
                return;
            }
        };

        // Check if this is a stream data source
        if let crate::config::DataSource::SingleOrStream(crate::config::SingleOrStream::Stream(_)) =
            &page.data
        {
            // Start streaming
            if let Err(e) = self.start_stream(&page).await {
                self.error_message = Some(format!("Failed to start stream: {}", e));
                self.loading = false;
            } else {
                self.loading = false;
            }
            return;
        }

        // Fetch data for non-stream sources
        match self.fetch_page_data(&page).await {
            Ok(data) => {
                self.current_data = data;
                self.apply_sort_and_filter();
                self.selected_index = 0;
                self.scroll_offset = 0;
                self.loading = false;
                self.last_refresh = std::time::Instant::now();
                self.needs_render = true;

                // Spawn background refresh watcher if refresh_interval is set
                self.spawn_refresh_watcher(self.current_page.clone(), page);
            }
            Err(e) => {
                self.error_message = Some(format!("Failed to load data: {}", e));
                self.loading = false;
                self.needs_render = true;
            }
        }
    }

    fn spawn_refresh_watcher(&mut self, page_name: String, page: crate::config::Page) {
        use crate::config::DataSource;

        // Get refresh interval
        let refresh_interval = match &page.data {
            DataSource::SingleOrStream(crate::config::SingleOrStream::Single(single)) => {
                if let Some(interval_str) = &single.refresh_interval {
                    humantime::parse_duration(interval_str).ok()
                } else {
                    None
                }
            }
            _ => None,
        };

        // Only spawn watcher if refresh_interval is set
        let interval = match refresh_interval {
            Some(i) => i,
            None => return,
        };

        // Create channel for sending refresh updates
        let (tx, rx) = mpsc::channel(10);
        self.refresh_receiver = Some(rx);

        // Clone necessary data for the background task
        let nav_context = self.nav_context.clone();

        // Spawn background task
        tokio::spawn(async move {
            let mut interval_timer = tokio::time::interval(interval);
            interval_timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

            loop {
                interval_timer.tick().await;

                // Fetch data in background
                let data = Self::fetch_data_static(&page, &nav_context).await;

                if let Ok(data) = data {
                    // Send update through channel
                    if tx
                        .send(RefreshMessage {
                            page_name: page_name.clone(),
                            data,
                        })
                        .await
                        .is_err()
                    {
                        // Channel closed, exit background task
                        break;
                    }
                }
            }
        });
    }

    fn check_refresh_updates(&mut self) {
        // Collect all pending messages first
        let mut messages = Vec::new();
        if let Some(receiver) = &mut self.refresh_receiver {
            while let Ok(msg) = receiver.try_recv() {
                messages.push(msg);
            }
        }

        // Process messages without holding the receiver borrow
        for msg in messages {
            // Only update if the message is for the current page
            if msg.page_name == self.current_page {
                self.current_data = msg.data;
                self.apply_sort_and_filter();
                self.needs_render = true;
            }
        }
    }

    async fn start_stream(&mut self, page: &crate::config::Page) -> Result<()> {
        use crate::config::{DataSource, SingleOrStream};
        use crate::data::StreamProvider;

        let stream_source = match &page.data {
            DataSource::SingleOrStream(SingleOrStream::Stream(stream)) => stream,
            _ => return Ok(()),
        };

        // Only support CLI streaming for now
        let command = stream_source.command.as_ref().ok_or_else(|| {
            crate::error::TermStackError::DataProvider("Stream must have command".to_string())
        })?;

        // Render command and args with templates
        let ctx = self.create_template_context(None);
        let rendered_command = globals::template_engine().render_string(command, &ctx)?;
        let rendered_args: Result<Vec<String>> = stream_source
            .args
            .iter()
            .map(|arg| globals::template_engine().render_string(arg, &ctx))
            .collect();
        let rendered_args = rendered_args?;

        // Create stream provider
        let mut provider = StreamProvider::new(rendered_command)
            .with_args(rendered_args)
            .with_shell(stream_source.shell);

        if let Some(working_dir) = &stream_source.working_dir {
            provider = provider.with_working_dir(working_dir.clone());
        }

        if !stream_source.env.is_empty() {
            provider = provider.with_env(stream_source.env.clone());
        }

        // Start streaming
        let receiver = provider.start_stream()?;

        // Update state
        self.stream_receiver = Some(receiver);
        self.stream_active = true;
        self.stream_paused = false;
        self.stream_buffer.clear();
        self.stream_status = StreamStatus::Connected;
        self.selected_index = 0;
        self.scroll_offset = 0;
        self.needs_clear = true; // Force full terminal clear on stream start

        Ok(())
    }

    fn stop_stream(&mut self) {
        if self.stream_active {
            self.needs_clear = true;
        }
        self.stream_receiver = None;
        self.stream_active = false;
        self.stream_paused = false;
        self.stream_status = StreamStatus::Stopped;
    }

    fn check_stream_updates(&mut self) {
        if !self.stream_active {
            return;
        }

        // Get buffer size limit from config
        let page = match globals::config().pages.get(&self.current_page) {
            Some(p) => p,
            None => return,
        };

        let buffer_size = match &page.data {
            crate::config::DataSource::SingleOrStream(crate::config::SingleOrStream::Stream(
                stream,
            )) => stream.buffer_size,
            _ => 100,
        };

        // Check for new messages
        if let Some(receiver) = &mut self.stream_receiver {
            while let Ok(msg) = receiver.try_recv() {
                match msg {
                    StreamMessage::Connected => {
                        self.stream_status = StreamStatus::Streaming;
                        self.needs_render = true;
                    }
                    StreamMessage::Data(line) => {
                        self.stream_status = StreamStatus::Streaming;

                        // Add to buffer
                        self.stream_buffer.push_back(line);

                        // Remove oldest if buffer is full
                        while self.stream_buffer.len() > buffer_size {
                            self.stream_buffer.pop_front();
                        }

                        // Only trigger render and update position when NOT paused
                        if !self.stream_paused {
                            // Auto-scroll to bottom if follow is enabled
                            if self.logs_follow {
                                self.selected_index = self.stream_buffer.len().saturating_sub(1);
                            }
                            self.needs_render = true;
                        }
                        // When paused: buffer is updated but NO render triggered
                        // View stays frozen on the same content
                    }
                    StreamMessage::End => {
                        self.stream_status = StreamStatus::Stopped;
                        self.stream_active = false;
                        self.needs_render = true;
                    }
                    StreamMessage::Error(err) => {
                        self.stream_status = StreamStatus::Error(err.clone());
                        self.stream_active = false;
                        self.error_message = Some(format!("Stream error: {}", err));
                        self.needs_render = true;
                    }
                }
            }
        }
    }

    async fn fetch_page_data(&self, page: &crate::config::Page) -> Result<Vec<Value>> {
        use crate::config::DataSource;
        use crate::data::{CliProvider, DataProvider};

        let data_source = &page.data;

        match data_source {
            DataSource::SingleOrStream(crate::config::SingleOrStream::Single(single)) => {
                match single.source_type {
                    crate::config::DataSourceType::Cli => {
                        let command = single.command.as_ref().ok_or_else(|| {
                            crate::error::TermStackError::DataProvider(
                                "Missing command".to_string(),
                            )
                        })?;

                        // Render command and args with templates
                        let ctx = self.create_template_context(None);
                        let rendered_command =
                            globals::template_engine().render_string(command, &ctx)?;
                        let rendered_args: Result<Vec<String>> = single
                            .args
                            .iter()
                            .map(|arg| globals::template_engine().render_string(arg, &ctx))
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
            DataSource::SingleOrStream(crate::config::SingleOrStream::Stream(_)) => {
                // Stream sources don't use fetch_page_data
                // They will be handled separately with streaming infrastructure
                Ok(Vec::new())
            }
        }
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

    // Static version of fetch_page_data for background tasks
    async fn fetch_data_static(
        page: &crate::config::Page,
        nav_context: &NavigationContext,
    ) -> Result<Vec<Value>> {
        use crate::config::DataSource;
        use crate::data::{CliProvider, DataProvider};

        let data_source = &page.data;

        match data_source {
            DataSource::SingleOrStream(crate::config::SingleOrStream::Single(single)) => {
                match single.source_type {
                    crate::config::DataSourceType::Cli => {
                        let command = single.command.as_ref().ok_or_else(|| {
                            crate::error::TermStackError::DataProvider(
                                "Missing command".to_string(),
                            )
                        })?;

                        // Create template context
                        let mut ctx =
                            TemplateContext::new().with_globals(nav_context.globals.clone());
                        for (page_name, data) in &nav_context.page_contexts {
                            ctx = ctx.with_page_context(page_name.clone(), data.clone());
                        }

                        // Render command and args with templates
                        let rendered_command =
                            globals::template_engine().render_string(command, &ctx)?;
                        let rendered_args: Result<Vec<String>> = single
                            .args
                            .iter()
                            .map(|arg| globals::template_engine().render_string(arg, &ctx))
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
                            globals: nav_context.globals.clone(),
                            page_contexts: nav_context.page_contexts.clone(),
                        };

                        let result = provider.fetch(&data_context).await?;

                        // Extract items using JSONPath
                        let items = if let Some(items_path) = &single.items {
                            let extractor = JsonPathExtractor::new(items_path)?;
                            extractor.extract(&result)?
                        } else {
                            vec![result]
                        };

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
            DataSource::SingleOrStream(crate::config::SingleOrStream::Stream(_)) => Ok(Vec::new()),
        }
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

        // Handle global search mode
        if self.global_search.active {
            match key.code {
                KeyCode::Char(c)
                    if c == 'C'
                        && key
                            .modifiers
                            .contains(crossterm::event::KeyModifiers::CONTROL) =>
                {
                    // Ctrl+C: Toggle case sensitivity
                    self.global_search.toggle_case_sensitive();
                    return;
                }
                KeyCode::Char(c) => {
                    self.global_search.push_char(c);
                    self.needs_render = true;
                    return;
                }
                KeyCode::Backspace => {
                    self.global_search.pop_char();
                    self.needs_render = true;
                    return;
                }
                KeyCode::Enter => {
                    // Apply the search filter
                    self.global_search.apply();
                    // Re-filter the data for table views
                    if !self.stream_active {
                        self.apply_sort_and_filter();
                        self.selected_index = 0;
                    } else {
                        // For stream views, trigger render to apply filter
                        self.selected_index = 0;
                        self.needs_render = true;
                    }
                    return;
                }
                KeyCode::Esc => {
                    // Cancel search and clear filter
                    self.global_search.cancel();
                    // Re-filter the data for table views
                    if !self.stream_active {
                        self.apply_sort_and_filter();
                        self.selected_index = 0;
                    } else {
                        // For stream views, trigger render to clear filter
                        self.selected_index = 0;
                        self.needs_render = true;
                    }
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
                // Always show quit confirmation
                self.show_quit_confirm = true;
                self.needs_render = true;
            }
            KeyCode::Esc => {
                // If in action mode, exit action mode first
                if self.action_mode {
                    self.action_mode = false;
                    self.needs_render = true;
                }
                // If search filter is active, clear it first
                else if self.global_search.filter_active {
                    self.global_search.clear();
                    // Re-filter the data for table views
                    if !self.stream_active {
                        self.apply_sort_and_filter();
                        self.selected_index = 0;
                    }
                    self.needs_render = true;
                } else if !self.nav_stack.is_empty() {
                    self.go_back().await;
                }
            }
            KeyCode::Char('j') | KeyCode::Down => self.move_down(),
            KeyCode::Char('k') | KeyCode::Up => self.move_up(),
            KeyCode::Char('g') => self.move_top(),
            KeyCode::Char('G') => self.move_bottom(),
            KeyCode::Char('r') => self.load_current_page().await,
            KeyCode::Char('/') => {
                // Activate global search
                self.global_search.activate();
                self.needs_render = true;
            }
            KeyCode::Char('f') => {
                // Toggle follow in logs view (when paused, 'f' resumes LIVE mode)
                if self.stream_active {
                    if self.stream_paused {
                        // Currently paused, resume to LIVE
                        self.stream_paused = false;
                        self.logs_follow = true;
                        // Clear the frozen snapshot
                        self.stream_frozen_snapshot.clear();
                        if !self.stream_buffer.is_empty() {
                            self.selected_index = self.stream_buffer.len() - 1;
                        }
                        self.needs_render = true; // Force render when resuming
                    } else {
                        // Currently live, pause at current position
                        self.stream_paused = true;
                        self.logs_follow = false;
                        // Take a snapshot of the current buffer
                        self.stream_frozen_snapshot = self.stream_buffer.clone();
                        self.needs_render = true; // Force render to update status indicator
                    }
                }
            }
            KeyCode::Char('w') => {
                // Toggle wrap in logs view
                if self.stream_active {
                    self.logs_wrap = !self.logs_wrap;
                    // Reset horizontal scroll when enabling wrap
                    if self.logs_wrap {
                        self.logs_horizontal_scroll = 0;
                    }
                    // Always render user actions, even when paused
                    self.needs_render = true;
                }
            }
            KeyCode::Left => {
                // Scroll left in logs view (when wrap is off)
                if self.stream_active && !self.logs_wrap {
                    self.logs_horizontal_scroll = self.logs_horizontal_scroll.saturating_sub(5);
                    // Always render user actions, even when paused
                    self.needs_render = true;
                }
            }
            KeyCode::Right => {
                // Scroll right in logs view (when wrap is off)
                if self.stream_active && !self.logs_wrap {
                    self.logs_horizontal_scroll = self.logs_horizontal_scroll.saturating_add(5);
                    // Always render user actions, even when paused
                    self.needs_render = true;
                }
            }
            KeyCode::Char('h') => {
                // In action mode: treat as action key
                if self.action_mode {
                    self.action_mode = false;
                    self.handle_action_key('h').await;
                } else if self.stream_active && !self.logs_wrap {
                    // Normal mode: horizontal scroll left in logs view
                    self.logs_horizontal_scroll = self.logs_horizontal_scroll.saturating_sub(5);
                    // Always render user actions, even when paused
                    self.needs_render = true;
                }
            }
            KeyCode::Char('l') => {
                // In action mode: treat as action key
                if self.action_mode {
                    self.action_mode = false;
                    self.handle_action_key('l').await;
                } else if self.stream_active && !self.logs_wrap {
                    // Normal mode: horizontal scroll right in logs view
                    self.logs_horizontal_scroll = self.logs_horizontal_scroll.saturating_add(5);
                    // Always render user actions, even when paused
                    self.needs_render = true;
                }
            }
            KeyCode::Enter => {
                // In action mode: treat as action key
                if self.action_mode {
                    self.action_mode = false;
                    self.handle_action_key('\n').await;
                } else {
                    // Normal mode: navigate to next page
                    self.navigate_next().await;
                }
            }
            KeyCode::Char('a') => {
                // Enter action mode (never conflicts because 'a' is the action mode trigger)
                if !self.action_mode {
                    self.action_mode = true;
                    self.needs_render = true;
                }
            }
            KeyCode::Char(c) => {
                // In action mode: ANY character is an action key
                if self.action_mode {
                    self.action_mode = false;
                    self.handle_action_key(c).await;
                }
                // Normal mode: ignore unmapped keys (no conflict)
            }
            _ => {}
        }
    }

    async fn handle_action_key(&mut self, key: char) {
        // Find matching action and clone it to avoid borrow issues
        let action_to_execute = {
            let page = match globals::config().pages.get(&self.current_page) {
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
                let rendered_msg = globals::template_engine()
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
        // Create template context for rendering custom messages
        let selected_row = self.get_selected_row();
        let template_ctx = self.create_template_context(selected_row);

        // Create context map for action executor
        let context = self.create_template_context_map();

        // Execute action
        match self.action_executor.execute(action, &context).await {
            Ok(ActionResult::Success(_msg)) => {
                // Only show notification if explicitly configured
                if let Some(notification) = &action.notification {
                    if let Some(custom_msg) = &notification.on_success {
                        let message = globals::template_engine()
                            .render_string(custom_msg, &template_ctx)
                            .unwrap_or_else(|_| custom_msg.clone());

                        self.action_message = Some(ActionMessage {
                            message,
                            message_type: MessageType::Success,
                            timestamp: std::time::Instant::now(),
                        });
                        self.needs_render = true;
                    }
                } else if let Some(success_msg) = &action.success_message {
                    // Legacy support for success_message
                    let message = globals::template_engine()
                        .render_string(success_msg, &template_ctx)
                        .unwrap_or_else(|_| success_msg.clone());

                    self.action_message = Some(ActionMessage {
                        message,
                        message_type: MessageType::Success,
                        timestamp: std::time::Instant::now(),
                    });
                    self.needs_render = true;
                }
                // If neither notification nor success_message is set, execute silently
            }
            Ok(ActionResult::Error(msg)) => {
                // Always show errors, but use custom message if configured
                let message = if let Some(notification) = &action.notification {
                    if let Some(custom_msg) = &notification.on_failure {
                        globals::template_engine()
                            .render_string(custom_msg, &template_ctx)
                            .unwrap_or_else(|_| custom_msg.clone())
                    } else {
                        msg
                    }
                } else if let Some(error_msg) = &action.error_message {
                    globals::template_engine()
                        .render_string(error_msg, &template_ctx)
                        .unwrap_or_else(|_| error_msg.clone())
                } else {
                    msg
                };

                self.action_message = Some(ActionMessage {
                    message,
                    message_type: MessageType::Error,
                    timestamp: std::time::Instant::now(),
                });
                self.needs_render = true;
            }
            Ok(ActionResult::Refresh) => {
                // Show success notification if configured, then reload
                if let Some(notification) = &action.notification {
                    if let Some(custom_msg) = &notification.on_success {
                        let message = globals::template_engine()
                            .render_string(custom_msg, &template_ctx)
                            .unwrap_or_else(|_| custom_msg.clone());

                        self.action_message = Some(ActionMessage {
                            message,
                            message_type: MessageType::Success,
                            timestamp: std::time::Instant::now(),
                        });
                        self.needs_render = true;
                    }
                } else if let Some(success_msg) = &action.success_message {
                    // Legacy support for success_message
                    let message = globals::template_engine()
                        .render_string(success_msg, &template_ctx)
                        .unwrap_or_else(|_| success_msg.clone());

                    self.action_message = Some(ActionMessage {
                        message,
                        message_type: MessageType::Success,
                        timestamp: std::time::Instant::now(),
                    });
                    self.needs_render = true;
                }

                // Reload the page
                self.load_current_page().await;
            }
            Ok(ActionResult::Navigate(page, context_map)) => {
                // Navigate to the specified page with context
                self.navigate_to_page(&page, context_map).await;
            }
            Err(e) => {
                // Use custom error notification message if available for executor errors
                let message = if let Some(notification) = &action.notification {
                    if let Some(custom_msg) = &notification.on_failure {
                        globals::template_engine()
                            .render_string(custom_msg, &template_ctx)
                            .unwrap_or_else(|_| format!("Action failed: {}", e))
                    } else {
                        format!("Action failed: {}", e)
                    }
                } else if let Some(error_msg) = &action.error_message {
                    globals::template_engine()
                        .render_string(error_msg, &template_ctx)
                        .unwrap_or_else(|_| format!("Action failed: {}", e))
                } else {
                    format!("Action failed: {}", e)
                };

                self.action_message = Some(ActionMessage {
                    message,
                    message_type: MessageType::Error,
                    timestamp: std::time::Instant::now(),
                });
                self.needs_render = true;
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
                match globals::template_engine().render_string(&template, &template_ctx) {
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

        // Save current page ID before navigation
        let source_page_id = self.current_page.clone();

        // Save current state to navigation stack
        let frame = NavigationFrame {
            page_id: source_page_id.clone(),
            context: HashMap::new(),
            scroll_offset: self.scroll_offset,
            selected_index: self.selected_index,
        };
        self.nav_stack.push(frame);

        // Update navigation context with new data
        for (key, value) in rendered_context {
            self.nav_context.page_contexts.insert(key, value);
        }

        // Also store the entire selected row under the current page name
        // This allows templates like "Pods - {{ namespaces.metadata.name }}" to work
        if let Some(row) = selected_row {
            self.nav_context.set_page_context(source_page_id, row);
        }

        // Navigate to new page
        self.current_page = target_page.to_string();
        self.selected_index = 0;
        self.scroll_offset = 0;

        // Load new page data
        self.load_current_page().await;
    }

    fn move_down(&mut self) {
        // Check if we're in a text view
        if let Some(page) = globals::config().pages.get(&self.current_page) {
            if matches!(page.view, ConfigView::Text(_)) {
                // Text view: scroll down by one line
                self.scroll_offset += 1;
                self.needs_render = true;
                return;
            }
        }

        let max_index = if self.stream_active || !self.stream_buffer.is_empty() {
            // Stream mode: use display buffer (frozen snapshot if paused)
            let display_buffer_len =
                if self.stream_paused && !self.stream_frozen_snapshot.is_empty() {
                    self.stream_frozen_snapshot.len()
                } else {
                    self.stream_buffer.len()
                };
            if display_buffer_len == 0 {
                return;
            }
            display_buffer_len - 1
        } else {
            // Table mode: use filtered data
            if self.filtered_data.is_empty() {
                return;
            }
            self.filtered_data.len() - 1
        };

        if self.selected_index < max_index {
            self.selected_index += 1;
            // Always render cursor movement, even when paused
            self.needs_render = true;
        }
    }

    fn move_up(&mut self) {
        // Check if we're in a text view
        if let Some(page) = globals::config().pages.get(&self.current_page) {
            if matches!(page.view, ConfigView::Text(_)) {
                // Text view: scroll up by one line
                if self.scroll_offset > 0 {
                    self.scroll_offset -= 1;
                    self.needs_render = true;
                }
                return;
            }
        }

        if self.selected_index > 0 {
            self.selected_index -= 1;
            // Always render cursor movement, even when paused
            self.needs_render = true;
        }
    }

    fn move_top(&mut self) {
        // Check if we're in a text view
        if let Some(page) = globals::config().pages.get(&self.current_page) {
            if matches!(page.view, ConfigView::Text(_)) {
                // Text view: scroll to top
                self.scroll_offset = 0;
                self.needs_render = true;
                return;
            }
        }

        self.selected_index = 0;
        // Always render cursor movement, even when paused
        self.needs_render = true;
    }

    fn move_bottom(&mut self) {
        // Check if we're in a text view
        if let Some(page) = globals::config().pages.get(&self.current_page) {
            if matches!(page.view, ConfigView::Text(_)) {
                // Text view: scroll to bottom (will be clamped in render_text)
                self.scroll_offset = usize::MAX;
                self.needs_render = true;
                return;
            }
        }

        if self.stream_active || !self.stream_buffer.is_empty() {
            // Stream mode - jumping to bottom does NOT change pause state
            // Use display buffer (frozen snapshot if paused)
            let display_buffer_len =
                if self.stream_paused && !self.stream_frozen_snapshot.is_empty() {
                    self.stream_frozen_snapshot.len()
                } else {
                    self.stream_buffer.len()
                };
            if display_buffer_len > 0 {
                self.selected_index = display_buffer_len - 1;
                // Always render cursor movement, even when paused
                self.needs_render = true;
            }
        } else {
            // Table mode
            if !self.filtered_data.is_empty() {
                self.selected_index = self.filtered_data.len() - 1;
                self.needs_render = true;
            }
        }
    }

    async fn go_back(&mut self) {
        if let Some(frame) = self.nav_stack.pop() {
            // Stop any active stream before navigating back
            self.stop_stream();

            // Clear search when navigating back
            self.global_search.clear();

            self.current_page = frame.page_id;
            self.selected_index = frame.selected_index;
            self.scroll_offset = frame.scroll_offset;
            self.load_current_page().await;
        }
    }

    async fn navigate_next(&mut self) {
        let page = match globals::config().pages.get(&self.current_page) {
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

        // Clear search when navigating to next page
        self.global_search.clear();

        // Navigate to next page
        self.current_page = next_page.clone();
        self.load_current_page().await;
    }

    fn render(&mut self, frame: &mut Frame) {
        let area = frame.area();

        // Dynamically adjust header size based on search state
        let header_height = if self.global_search.active {
            6 // Breadcrumb + search input
        } else {
            3 // Just breadcrumb (with inline filter tag if active)
        };

        let chunks = Layout::vertical([
            Constraint::Length(header_height), // Header
            Constraint::Min(0),                // Content
            Constraint::Length(4),             // Status bar
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
        // Only show search input if actively typing
        if self.global_search.active {
            let header_chunks = Layout::default()
                .direction(ratatui::layout::Direction::Vertical)
                .constraints([
                    Constraint::Length(3), // Breadcrumb with filter tag
                    Constraint::Length(3), // Search input
                ])
                .split(area);

            // Render breadcrumb
            self.render_breadcrumb(frame, header_chunks[0]);

            // Render search input
            self.render_search_input(frame, header_chunks[1]);
        } else {
            // Just show breadcrumb (with filter tag if active)
            self.render_breadcrumb(frame, area);
        }
    }

    fn render_breadcrumb(&self, frame: &mut Frame, area: Rect) {
        let mut spans = vec![
            Span::styled(
                &globals::config().app.name,
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" | "),
        ];

        // Add pages from navigation stack (if any)
        for (idx, nav_frame) in self.nav_stack.frames().iter().enumerate() {
            if idx > 0 {
                spans.push(Span::raw(" > "));
            }
            spans.push(Span::styled(
                &nav_frame.page_id,
                Style::default().fg(Color::White),
            ));
        }

        // Add separator before current page if there are previous pages
        if !self.nav_stack.frames().is_empty() {
            spans.push(Span::raw(" > "));
        }

        // Add current page with distinct color
        spans.push(Span::styled(
            &self.current_page,
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ));

        let header =
            Paragraph::new(Line::from(spans)).block(Block::default().borders(Borders::ALL));
        frame.render_widget(header, area);
    }

    fn render_search_input(&self, frame: &mut Frame, area: Rect) {
        // Only renders during active input
        let search_text = format!("{}_", self.global_search.query);

        let case_indicator = if self.global_search.case_sensitive {
            " [Case-sensitive]"
        } else {
            ""
        };

        let mode_indicator = if self.global_search.query.starts_with('!') {
            " (Regex)"
        } else {
            " (Literal)"
        };

        let title = format!(
            "Search{}{} - Enter to apply, Esc to cancel",
            mode_indicator, case_indicator
        );

        let search_input = Paragraph::new(search_text)
            .style(
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(title)
                    .border_style(Style::default().fg(Color::Yellow)),
            );

        frame.render_widget(search_input, area);
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

        let page = match globals::config().pages.get(&self.current_page) {
            Some(p) => p,
            None => return,
        };

        match &page.view {
            ConfigView::Table(table_view) => {
                let table_view = table_view.clone();
                self.render_table(frame, area, &table_view);
            }
            ConfigView::Logs(logs_view) => {
                let logs_view = logs_view.clone();
                self.render_logs(frame, area, &logs_view);
            }
            ConfigView::Text(text_view) => {
                self.render_text(frame, area, text_view);
            }
        }
    }

    fn render_table(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        table_config: &crate::config::TableView,
    ) {
        // Get the rendered page title
        let page_title = self.get_rendered_page_title();

        if self.filtered_data.is_empty() {
            let empty = Paragraph::new("No data")
                .block(Block::default().borders(Borders::ALL).title(page_title));
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
            .map(|(_idx, item)| {
                let cells: Vec<Cell> = table_config
                    .columns
                    .iter()
                    .map(|col| {
                        // Extract value using JSONPath
                        let value_str = if let Ok(extractor) = JsonPathExtractor::new(&col.path) {
                            if let Ok(Some(value)) = extractor.extract_single(item) {
                                // Apply transform if present
                                if let Some(transform) = &col.transform {
                                    // Create context with full row for transform
                                    let mut row_ctx = self.create_template_context(Some(item));
                                    // Add the extracted value as "value" page context for easy access in transforms
                                    row_ctx = row_ctx
                                        .with_page_context("value".to_string(), value.clone());

                                    globals::template_engine()
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

                // TableState handles highlighting, no manual styling needed
                Row::new(cells)
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
            .block(Block::default().borders(Borders::ALL).title(page_title))
            .row_highlight_style(
                Style::default()
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol(">> ");

        // Use stateful rendering for efficient highlight updates
        frame.render_stateful_widget(table, area, &mut self.table_state);
    }

    fn render_text(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        text_config: &crate::config::schema::TextView,
    ) {
        let page_title = self.get_rendered_page_title();

        if self.current_data.is_empty() {
            let msg = Paragraph::new("No data")
                .block(Block::default().borders(Borders::ALL).title(page_title));
            frame.render_widget(msg, area);
            return;
        }

        // Get the first item (text views typically show single document)
        let item = &self.current_data[0];

        // Convert to string representation
        let content_str = if item.is_string() {
            // Already a string - check if it's JSON and re-format for proper indentation
            let raw = item.as_str().unwrap_or("");
            if let Ok(json_val) = serde_json::from_str::<Value>(raw) {
                // Re-parse and pretty-print JSON
                serde_json::to_string_pretty(&json_val).unwrap_or_else(|_| raw.to_string())
            } else {
                raw.to_string()
            }
        } else {
            // Convert JSON object to formatted string
            serde_json::to_string_pretty(item).unwrap_or_else(|_| "Failed to serialize".to_string())
        };

        // Auto-detect content type if not specified
        let detected_syntax: String = text_config
            .syntax
            .as_ref()
            .map(|s| s.to_string())
            .unwrap_or_else(|| self.detect_content_type(&content_str).to_string());

        // Apply syntax highlighting
        let mut lines =
            self.highlight_text(&content_str, &detected_syntax, text_config.line_numbers);

        // Apply search filter if active
        if self.global_search.filter_active && !self.global_search.query.is_empty() {
            let content_lines: Vec<&str> = content_str.lines().collect();
            lines = lines
                .into_iter()
                .zip(content_lines.iter())
                .filter(|(_, line_text)| self.global_search.matches(line_text))
                .map(|(line, _)| line)
                .collect();
        }

        let total_lines = lines.len();

        // Calculate visible area
        let visible_height = area.height.saturating_sub(2) as usize; // Account for borders

        // Adjust scroll offset to stay within bounds
        if self.scroll_offset >= total_lines.saturating_sub(visible_height) {
            self.scroll_offset = total_lines.saturating_sub(visible_height);
        }

        let scroll_offset = self.scroll_offset;

        // Get visible lines based on scroll offset
        let visible_lines: Vec<Line> = lines
            .into_iter()
            .skip(scroll_offset)
            .take(visible_height)
            .collect();

        let mut paragraph = Paragraph::new(visible_lines).block(
            Block::default().borders(Borders::ALL).title(format!(
                "{} [{}] ({}/{})",
                page_title,
                detected_syntax,
                scroll_offset + 1,
                total_lines
            )),
        );

        if text_config.wrap {
            paragraph = paragraph.wrap(ratatui::widgets::Wrap { trim: false });
        }

        frame.render_widget(paragraph, area);
    }

    /// Detect content type based on content
    fn detect_content_type(&self, content: &str) -> &str {
        let trimmed = content.trim_start();

        // YAML detection
        if trimmed.starts_with("---")
            || trimmed.contains("apiVersion:")
            || trimmed.contains("kind:")
        {
            return "yaml";
        }

        // JSON detection
        if trimmed.starts_with('{') || trimmed.starts_with('[') {
            return "json";
        }

        // XML detection
        if trimmed.starts_with("<?xml") || trimmed.starts_with('<') {
            return "xml";
        }

        // TOML detection
        if trimmed.contains('[') && trimmed.contains(']') && trimmed.contains('=') {
            return "toml";
        }

        // Default to plain text
        "text"
    }

    /// Apply basic syntax highlighting to text
    fn highlight_text(
        &self,
        content: &str,
        syntax: &str,
        line_numbers: bool,
    ) -> Vec<Line<'static>> {
        let lines: Vec<&str> = content.lines().collect();
        let line_count = lines.len();
        let line_num_width = line_count.to_string().len();

        lines
            .iter()
            .enumerate()
            .map(|(idx, line)| {
                let mut spans = Vec::new();

                // Add line numbers if enabled
                if line_numbers {
                    spans.push(Span::styled(
                        format!("{:>width$} │ ", idx + 1, width = line_num_width),
                        Style::default().fg(Color::DarkGray),
                    ));
                }

                // Apply syntax-specific highlighting
                match syntax {
                    "yaml" => spans.extend(self.highlight_yaml_line(line)),
                    "json" => spans.extend(self.highlight_json_line(line)),
                    "xml" => spans.extend(self.highlight_xml_line(line)),
                    _ => spans.push(Span::raw(line.to_string())),
                }

                Line::from(spans)
            })
            .collect()
    }

    /// Simple YAML syntax highlighting
    fn highlight_yaml_line(&self, line: &str) -> Vec<Span<'static>> {
        let trimmed = line.trim_start();

        // Comments
        if trimmed.starts_with('#') {
            return vec![Span::styled(
                line.to_string(),
                Style::default().fg(Color::Green),
            )];
        }

        // Document separator
        if trimmed.starts_with("---") || trimmed.starts_with("...") {
            return vec![Span::styled(
                line.to_string(),
                Style::default().fg(Color::Magenta),
            )];
        }

        // Key-value pairs
        if let Some(colon_pos) = line.find(':') {
            let key = &line[..colon_pos];
            let rest = &line[colon_pos..];

            vec![
                Span::styled(
                    key.to_string(),
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(rest.to_string(), Style::default().fg(Color::White)),
            ]
        } else {
            vec![Span::raw(line.to_string())]
        }
    }

    /// Simple JSON syntax highlighting
    fn highlight_json_line(&self, line: &str) -> Vec<Span<'static>> {
        let trimmed = line.trim();

        // Keys (quoted strings followed by colon)
        if trimmed.contains("\":") {
            let mut spans = Vec::new();
            let mut current_pos = 0;

            for (idx, ch) in line.char_indices() {
                if ch == '"' && idx + 1 < line.len() {
                    // Find closing quote
                    if let Some(close_idx) = line[idx + 1..].find('"') {
                        let close_pos = idx + 1 + close_idx;
                        if close_pos + 1 < line.len()
                            && line.chars().nth(close_pos + 1) == Some(':')
                        {
                            // This is a key
                            if current_pos < idx {
                                spans.push(Span::raw(line[current_pos..idx].to_string()));
                            }
                            spans.push(Span::styled(
                                line[idx..=close_pos].to_string(),
                                Style::default()
                                    .fg(Color::Cyan)
                                    .add_modifier(Modifier::BOLD),
                            ));
                            current_pos = close_pos + 1;
                        }
                    }
                }
            }

            if current_pos < line.len() {
                spans.push(Span::raw(line[current_pos..].to_string()));
            }

            spans
        } else {
            vec![Span::raw(line.to_string())]
        }
    }

    /// Simple XML syntax highlighting
    fn highlight_xml_line(&self, line: &str) -> Vec<Span<'static>> {
        if line.trim().starts_with('<') {
            vec![Span::styled(
                line.to_string(),
                Style::default().fg(Color::Magenta),
            )]
        } else {
            vec![Span::raw(line.to_string())]
        }
    }

    /// Parse ANSI escape codes in text and convert to ratatui Line with styles
    fn parse_ansi_line(&self, text: &str) -> Line<'static> {
        use ansi_to_tui::IntoText;
        match text.into_text() {
            Ok(parsed_text) => {
                // Convert Text to Line - take first line or empty
                if parsed_text.lines.is_empty() {
                    Line::from(text.to_string())
                } else {
                    parsed_text.lines[0].clone()
                }
            }
            Err(_) => {
                // Fallback: return plain text if parsing fails
                Line::from(text.to_string())
            }
        }
    }

    fn render_logs(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        _logs_config: &crate::config::schema::LogsView,
    ) {
        // Get the rendered page title
        let page_title = self.get_rendered_page_title();

        // For streaming logs, render from stream buffer
        if self.stream_active || !self.stream_buffer.is_empty() {
            // Use frozen snapshot when paused, otherwise use live buffer
            let display_buffer = if self.stream_paused && !self.stream_frozen_snapshot.is_empty() {
                &self.stream_frozen_snapshot
            } else {
                &self.stream_buffer
            };

            if display_buffer.is_empty() {
                let empty = Paragraph::new("Waiting for data...")
                    .style(Style::default().fg(Color::Yellow))
                    .block(Block::default().borders(Borders::ALL).title(page_title));
                frame.render_widget(empty, area);
                return;
            }

            // Filter logs using global search if active
            let filtered_indices: Vec<usize> = if self.global_search.filter_active {
                display_buffer
                    .iter()
                    .enumerate()
                    .filter(|(_, line)| self.global_search.matches(line))
                    .map(|(idx, _)| idx)
                    .collect()
            } else {
                // No filter, use all indices
                (0..display_buffer.len()).collect()
            };

            // Calculate visible area
            let visible_height = area.height.saturating_sub(2) as usize; // Account for borders

            // When follow is enabled, ensure we stay at the bottom of the buffer
            if self.logs_follow && !self.stream_paused {
                if !display_buffer.is_empty() {
                    self.selected_index = display_buffer.len() - 1;
                }
            }

            // Ensure selected_index is within bounds of the display buffer
            if !display_buffer.is_empty() {
                self.selected_index = self.selected_index.min(display_buffer.len() - 1);
            }

            // Find the position of selected_index in the filtered list
            let selected_filter_pos = filtered_indices
                .iter()
                .position(|&idx| idx == self.selected_index)
                .unwrap_or(filtered_indices.len().saturating_sub(1));

            // Calculate scroll position based on filtered results
            let total_lines = filtered_indices.len();
            let mut start_line = selected_filter_pos.saturating_sub(visible_height / 2);

            // Adjust if at the end
            if selected_filter_pos + visible_height / 2 >= total_lines {
                start_line = total_lines.saturating_sub(visible_height);
            }

            let _end_line = (start_line + visible_height).min(total_lines);

            // Build visible lines with optional timestamps and wrapping
            let content_width = area.width.saturating_sub(4) as usize; // Account for borders and padding
            let mut lines: Vec<Line> = Vec::new();

            for i in start_line..total_lines.min(start_line + visible_height) {
                // When wrapping is disabled, limit the number of lines to visible height
                // When wrapping is enabled, don't limit since lines may wrap to multiple rows
                if !self.logs_wrap && lines.len() >= visible_height {
                    break;
                }

                let actual_idx = filtered_indices[i];
                let line = &display_buffer[actual_idx];
                let display_line = line.clone();

                // Parse ANSI codes to preserve colors
                let mut parsed_line = self.parse_ansi_line(&display_line);

                // Apply selection highlighting if this is the selected line
                if actual_idx == self.selected_index {
                    // Add background to all spans in the line
                    for span in &mut parsed_line.spans {
                        span.style = span.style.bg(Color::DarkGray).add_modifier(Modifier::BOLD);
                    }
                }

                // Handle wrapping if enabled
                if self.logs_wrap {
                    // For wrapping, we need to handle line width
                    let line_width: usize = parsed_line.spans.iter().map(|s| s.content.len()).sum();

                    if line_width > content_width {
                        // TODO: Proper wrapping with ANSI styles is complex
                        // For now, just push the line and let Paragraph wrap it
                        lines.push(parsed_line);
                    } else {
                        lines.push(parsed_line);
                    }
                } else {
                    // Single line with horizontal scroll support
                    if !self.logs_wrap && display_line.len() > content_width {
                        // Apply horizontal scroll offset
                        let start = self.logs_horizontal_scroll.min(display_line.len());
                        let end = (start + content_width).min(display_line.len());
                        let slice = &display_line[start..end];

                        // Add indicators for horizontal scroll
                        let left_indicator = if self.logs_horizontal_scroll > 0 {
                            "< "
                        } else {
                            ""
                        };
                        let right_indicator = if end < display_line.len() { " >" } else { "" };

                        let visible_line =
                            format!("{}{}{}", left_indicator, slice, right_indicator);
                        let mut scrolled_line = self.parse_ansi_line(&visible_line);

                        if actual_idx == self.selected_index {
                            for span in &mut scrolled_line.spans {
                                span.style =
                                    span.style.bg(Color::DarkGray).add_modifier(Modifier::BOLD);
                            }
                        }
                        lines.push(scrolled_line);
                    } else {
                        lines.push(parsed_line);
                    }
                }
            }

            // Add stream status indicator to title
            let mut title_parts = vec![];

            // Add base title
            title_parts.push(page_title);

            // Add stream status
            let status_str = match &self.stream_status {
                StreamStatus::Streaming if !self.stream_paused => " ● LIVE",
                StreamStatus::Streaming if self.stream_paused => " ⏸ PAUSED",
                StreamStatus::Stopped => " ⏹ STOPPED",
                StreamStatus::Error(err) => {
                    title_parts.push(format!(" ✗ ERROR: {}", err));
                    ""
                }
                _ => "",
            };
            if !status_str.is_empty() {
                title_parts.push(status_str.to_string());
            }

            // Add settings indicators
            let mut settings = vec![];
            if self.logs_follow {
                settings.push("F");
            }
            if self.logs_wrap {
                settings.push("W");
            }
            if !settings.is_empty() {
                title_parts.push(format!(" [{}]", settings.join("")));
            }

            // Add filter count if search is active
            if self.global_search.filter_active {
                title_parts.push(format!(
                    " ({}/{})",
                    filtered_indices.len(),
                    display_buffer.len()
                ));
            }

            let title_with_status = title_parts.join("");

            let mut logs = Paragraph::new(lines).block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(title_with_status),
            );

            // Enable wrapping if configured
            if self.logs_wrap {
                logs = logs.wrap(ratatui::widgets::Wrap { trim: false });
            }

            frame.render_widget(logs, area);
        } else {
            // Non-streaming logs view (not implemented yet)
            let msg = Paragraph::new("Non-streaming logs not yet implemented")
                .style(Style::default().fg(Color::Yellow))
                .block(Block::default().borders(Borders::ALL).title(page_title));
            frame.render_widget(msg, area);
        }
    }

    fn get_rendered_page_title(&self) -> String {
        // Get current page config
        let page = match globals::config().pages.get(&self.current_page) {
            Some(p) => p,
            None => return self.current_page.clone(), // Fallback to page ID
        };

        // Render the page title with template context
        let ctx = self.create_template_context(None);
        let mut title = globals::template_engine()
            .render_string(&page.title, &ctx)
            .unwrap_or_else(|_| page.title.clone());

        // Add search filter tag if active (but not during input)
        if self.global_search.filter_active && !self.global_search.active {
            let filter_display = if self.global_search.query.len() > 25 {
                format!("{}...", &self.global_search.query[..22])
            } else {
                self.global_search.query.clone()
            };

            let mode_indicator = if self.global_search.query.starts_with('!') {
                "~/" // regex
            } else {
                "" // literal
            };

            title = format!("{} | 🔍 {}{}", title, mode_indicator, filter_display);
        }

        title
    }

    fn render_statusbar(&self, frame: &mut Frame, area: Rect) {
        // Build navigation shortcuts (always shown)
        let nav_shortcuts = if self.stream_active && !self.logs_wrap {
            "j/k: Scroll  |  h/l: Side-scroll  |  g/G: Top/Bottom  |  /: Search  |  f: LIVE/Pause  |  w: Wrap  |  ESC: Back  |  q: Quit"
        } else if self.stream_active {
            "j/k: Scroll  |  g/G: Top/Bottom  |  /: Search  |  f: LIVE/Pause  |  w: Wrap  |  ESC: Back  |  q: Quit"
        } else if self.current_data.is_empty() {
            "q/ESC: Quit  |  r: Refresh"
        } else {
            "j/k: Move  |  g/G: Top/Bottom  |  Enter: Select  |  ESC: Back  |  r: Refresh  |  q: Quit"
        };

        let row_info = if self.stream_active {
            // Use frozen snapshot size when paused, otherwise use live buffer
            let buffer_len = if self.stream_paused && !self.stream_frozen_snapshot.is_empty() {
                self.stream_frozen_snapshot.len()
            } else {
                self.stream_buffer.len()
            };
            format!(
                "Lines: {} | Line {}/{}",
                buffer_len,
                self.selected_index + 1,
                buffer_len
            )
        } else if self.global_search.filter_active {
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
        let action_line = if self.action_mode {
            // In action mode: show available actions
            if let Some(page) = globals::config().pages.get(&self.current_page) {
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
                            "ACTION MODE - Select: ",
                            Style::default()
                                .fg(Color::Black)
                                .bg(Color::Yellow)
                                .add_modifier(Modifier::BOLD),
                        )];
                        spans.extend(action_shortcuts);
                        spans.push(Span::styled(
                            " [ESC to cancel]",
                            Style::default().fg(Color::DarkGray),
                        ));
                        Line::from(spans)
                    } else {
                        Line::from("")
                    }
                } else {
                    Line::from("")
                }
            } else {
                Line::from("")
            }
        } else if let Some(page) = globals::config().pages.get(&self.current_page) {
            // Normal mode: show hint to press 'a'
            if let Some(actions) = &page.actions {
                if !actions.is_empty() {
                    Line::from(vec![
                        Span::styled("Press ", Style::default().fg(Color::DarkGray)),
                        Span::styled(
                            "a",
                            Style::default()
                                .fg(Color::Yellow)
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(" for actions", Style::default().fg(Color::DarkGray)),
                    ])
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
        use ratatui::widgets::Clear;

        let (color, icon, title) = match msg.message_type {
            MessageType::Success => (Color::Green, "✓", "Success"),
            MessageType::Error => (Color::Red, "✗", "Error"),
            MessageType::Info => (Color::Blue, "ℹ", "Info"),
            MessageType::Warning => (Color::Yellow, "⚠", "Warning"),
        };

        // Calculate dynamic width based on message length
        let icon_title_len = icon.chars().count() + 1 + title.len(); // icon + space + title
        let message_len = msg.message.chars().count();
        let max_line_len = icon_title_len.max(message_len);

        // Dynamic width: fit content + borders (2) + padding (4)
        let content_width = max_line_len.min(60); // Max 60 chars for readability
        let msg_width = (content_width + 6) as u16; // +6 for borders and padding

        // Word wrap the message to fit within the width
        let wrapped_lines = Self::wrap_text(&msg.message, content_width);

        // Calculate height based on wrapped lines
        // icon line + spacing + message lines + padding + borders
        let content_height = wrapped_lines.len() as u16;
        let msg_height = (content_height + 6).min(area.height.saturating_sub(2)); // +6 for icon, spacing, padding, borders

        // Position at top-right corner
        let msg_x = area.width.saturating_sub(msg_width + 1); // 1 char padding from right edge
        let msg_y = 1; // 1 char from top

        let msg_area = Rect {
            x: msg_x,
            y: msg_y,
            width: msg_width,
            height: msg_height,
        };

        // Clear the background area to hide content behind
        frame.render_widget(Clear, msg_area);

        // Build the message text with wrapped lines and icon
        let mut message_lines = vec![Line::from("")]; // Top padding

        // Add icon line
        message_lines.push(Line::from(Span::styled(
            format!("{} {}", icon, title),
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        )));
        message_lines.push(Line::from("")); // Spacing

        // Add message lines
        for line in wrapped_lines {
            message_lines.push(Line::from(Span::styled(
                line,
                Style::default().fg(Color::White),
            )));
        }
        message_lines.push(Line::from("")); // Bottom padding

        let message_box = Paragraph::new(message_lines)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(color).add_modifier(Modifier::BOLD))
                    .style(Style::default().bg(Color::Black)),
            )
            .alignment(Alignment::Left)
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
        use ratatui::widgets::Clear;

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

        // Clear the background area to hide content behind
        frame.render_widget(Clear, popup_area);

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
                    .style(Style::default().bg(Color::Black))
                    .title("Confirm Action"),
            )
            .alignment(Alignment::Center);

        frame.render_widget(dialog, popup_area);
    }

    fn render_quit_confirm(&self, frame: &mut Frame, area: Rect) {
        use ratatui::layout::Alignment;
        use ratatui::widgets::Clear;

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

        // Clear the background area to hide content behind
        frame.render_widget(Clear, popup_area);

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
                    .style(Style::default().bg(Color::Black))
                    .title("Confirm"),
            )
            .alignment(Alignment::Center);

        frame.render_widget(dialog, popup_area);
    }

    fn apply_sort_and_filter(&mut self) {
        // Start with all data
        let mut data = self.current_data.clone();

        // Apply global search filter if active
        if self.global_search.filter_active {
            data = self.filter_data(&data);
        }

        // Apply sorting if configured
        if let Some(page) = globals::config().pages.get(&self.current_page) {
            if let ConfigView::Table(table_view) = &page.view {
                if let Some(sort_config) = &table_view.sort {
                    data = self.sort_data(&data, sort_config);
                }
            }
        }

        self.filtered_data = data;
    }

    fn filter_data(&self, data: &[Value]) -> Vec<Value> {
        data.iter()
            .filter(|item| {
                // Convert item to searchable string
                let item_text = self.item_to_searchable_text(item);
                // Use global search to match
                self.global_search.matches(&item_text)
            })
            .cloned()
            .collect()
    }

    fn item_to_searchable_text(&self, item: &Value) -> String {
        match item {
            Value::String(s) => s.clone(),
            Value::Number(n) => n.to_string(),
            Value::Bool(b) => b.to_string(),
            Value::Array(arr) => arr
                .iter()
                .map(|v| self.item_to_searchable_text(v))
                .collect::<Vec<_>>()
                .join(" "),
            Value::Object(map) => map
                .values()
                .map(|v| self.item_to_searchable_text(v))
                .collect::<Vec<_>>()
                .join(" "),
            Value::Null => String::new(),
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
