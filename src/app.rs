use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::{
    DefaultTerminal, Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table},
};
use serde_json::Value;
use std::collections::{HashMap, VecDeque};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};
use std::sync::Arc;
use tokio::sync::mpsc;

use crate::{
    action::executor::{ActionExecutor, ActionResult},
    config::{Config, View as ConfigView, TableColumn},
    data::{JsonPathExtractor, StreamMessage},
    error::Result,
    globals,
    navigation::{NavigationContext, NavigationFrame, NavigationStack},
    template::engine::TemplateContext,
};
use regex::Regex;

/// Global search state that works across all views
/// Search mode for global search
#[derive(Debug, Clone, PartialEq)]
enum SearchMode {
    /// Search across all columns
    Global,
    /// Search within a specific column
    ColumnSpecific {
        column_display_name: String,  // User-friendly name from "display" field
        column_path: String,          // JSONPath from "path" field
        search_term: String,
    },
}

impl Default for SearchMode {
    fn default() -> Self {
        SearchMode::Global
    }
}

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
    /// Current search mode (global or column-specific)
    mode: SearchMode,
}

impl Default for GlobalSearch {
    fn default() -> Self {
        GlobalSearch {
            active: false,
            query: String::new(),
            filter_active: false,
            regex_pattern: None,
            case_sensitive: false,
            mode: SearchMode::Global,
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

        // Fast path: for literal search (no regex), use simple string contains
        if !self.query.starts_with('!') {
            // Literal search - much faster than regex
            if self.case_sensitive {
                return text.contains(&self.query);
            } else {
                return text.to_lowercase().contains(&self.query.to_lowercase());
            }
        }

        // Regex path
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
        self.mode = SearchMode::Global;
    }

    /// Clear the search filter
    fn clear(&mut self) {
        self.query.clear();
        self.filter_active = false;
        self.regex_pattern = None;
        self.active = false; // Close search input when clearing
        self.mode = SearchMode::Global; // Reset to global search
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

    /// Parse query to determine if it's column-specific or global.
    /// Uses `%column_name%` delimiter syntax for unambiguous multi-word column names.
    /// E.g. `%Project Type% active` matches column "Project Type" with term "active".
    fn parse_mode(&self, table_columns: &[TableColumn]) -> SearchMode {
        // Check for %column_name% pattern
        if self.query.starts_with('%') {
            if let Some(end_pct) = self.query[1..].find('%') {
                let column_name = self.query[1..1 + end_pct].trim();
                let after_delim = &self.query[2 + end_pct..];

                // Must have a space then search term after closing %
                if after_delim.starts_with(' ') {
                    let search_term = after_delim[1..].trim();
                    if !search_term.is_empty() {
                        if let Some(col) = table_columns.iter()
                            .find(|c| c.display.eq_ignore_ascii_case(column_name))
                        {
                            return SearchMode::ColumnSpecific {
                                column_display_name: col.display.clone(),
                                column_path: col.path.clone(),
                                search_term: search_term.to_string(),
                            };
                        }
                    }
                }
            }
        }

        // Default to global search
        SearchMode::Global
    }

    /// Highlight search matches within spans by splitting them at match boundaries.
    /// Match regions get yellow background with black foreground overlay.
    fn highlight_search_in_spans<'a>(&self, spans: Vec<Span<'a>>) -> Vec<Span<'a>> {
        if !self.filter_active || self.query.is_empty() {
            return spans;
        }

        // For ColumnSpecific mode, use the search_term for highlighting
        let effective_query = match &self.mode {
            SearchMode::ColumnSpecific { search_term, .. } => search_term.as_str(),
            SearchMode::Global => &self.query,
        };

        if effective_query.is_empty() {
            return spans;
        }

        // Build a regex for finding matches in text
        let pattern = if effective_query.starts_with('!') {
            let pat = &effective_query[1..];
            if pat.is_empty() {
                return spans;
            }
            if self.case_sensitive {
                Regex::new(pat)
            } else {
                Regex::new(&format!("(?i){}", pat))
            }
        } else {
            let escaped = regex::escape(effective_query);
            if self.case_sensitive {
                Regex::new(&escaped)
            } else {
                Regex::new(&format!("(?i){}", escaped))
            }
        };

        let regex = match pattern {
            Ok(r) => r,
            Err(_) => return spans,
        };

        let highlight_style_modifier = |base: Style| -> Style {
            base.bg(Color::Yellow).fg(Color::Black)
        };

        let mut result = Vec::new();
        for span in spans {
            let text = span.content.as_ref();
            let style = span.style;

            let mut last_end = 0;
            for m in regex.find_iter(text) {
                // Add text before match with original style
                if m.start() > last_end {
                    result.push(Span::styled(
                        text[last_end..m.start()].to_string(),
                        style,
                    ));
                }
                // Add matched text with highlight style
                result.push(Span::styled(
                    text[m.start()..m.end()].to_string(),
                    highlight_style_modifier(style),
                ));
                last_end = m.end();
            }
            // Add remaining text after last match
            if last_end < text.len() {
                result.push(Span::styled(text[last_end..].to_string(), style));
            } else if last_end == 0 {
                // No matches found in this span, keep as-is
                result.push(span);
            }
        }
        result
    }
}

#[derive(Clone)]
struct LogLine {
    raw: String,            // ANSI-stripped plain text (for search matching)
    parsed: Line<'static>,  // Pre-parsed styled spans (for rendering)
}

pub struct App {
    running: bool,
    current_page: String,
    nav_stack: NavigationStack,
    nav_context: NavigationContext,
    action_executor: ActionExecutor,
    adapter_registry: Arc<crate::adapters::registry::AdapterRegistry>,

    // Current view state
    current_data: Vec<Value>,
    filtered_indices: Vec<usize>, // Indices into current_data (optimized - no cloning)
    selected_index: usize,
    scroll_offset: usize,
    table_state: ratatui::widgets::TableState,
    activity: ActivityState,
    spinner_frame: usize, // Current spinner animation frame (0-9)
    error_message: Option<String>,

    // Global search (works across all views)
    global_search: GlobalSearch,

    // Confirmation dialogs
    show_quit_confirm: bool,
    action_confirm: Option<ActionConfirm>,


    // Auto-refresh timer
    last_refresh: std::time::Instant,

    // Stream state
    stream_active: bool,
    stream_paused: bool,
    stream_buffer: VecDeque<LogLine>,
    stream_frozen_snapshot: Option<Arc<VecDeque<LogLine>>>, // Frozen snapshot when paused (Arc for efficient cloning)
    stream_receiver: Option<mpsc::Receiver<StreamMessage>>,
    stream_status: StreamStatus,

    // Logs view settings
    logs_follow: bool,
    logs_wrap: bool,
    logs_horizontal_scroll: usize,

    // Background action execution
    pending_action_info: Option<PendingActionInfo>,
    action_result_receiver: Option<mpsc::Receiver<ActionResultMsg>>,

    // Action menu (Shift+A to open, navigate with j/k, execute with Enter)
    show_action_menu: bool,
    action_menu_selected: usize,

    // UI state
    needs_clear: bool,
    needs_render: bool,

    // Data refresh watcher
    refresh_receiver: Option<mpsc::Receiver<RefreshMessage>>,

    // Page data cache for instant back navigation
    page_cache: HashMap<String, Vec<Value>>,
}

#[derive(Debug)]
enum RefreshMessage {
    Started { page_name: String },
    Completed { page_name: String, data: Vec<Value>, reset_selection: bool },
    Error { page_name: String, error: String },
}

#[derive(Clone)]
struct ActionConfirm {
    action: crate::config::schema::Action,
    message: String,
    executing: bool,
}

#[derive(Clone, Copy, PartialEq)]
#[allow(dead_code)]
enum MessageType {
    Success,
    Error,
    Info,
    Warning,
}

#[derive(Clone)]
enum ActivityState {
    Idle,
    Loading { message: String },
    Result { message: String, kind: MessageType, timestamp: std::time::Instant },
}

impl ActivityState {
    fn is_loading(&self) -> bool {
        matches!(self, ActivityState::Loading { .. })
    }
}

#[derive(Debug, Clone, PartialEq)]
enum StreamStatus {
    Idle,
    Connected,
    Streaming,
    Stopped,
    Error(String),
}

/// Info captured at action trigger time for processing results later
struct PendingActionInfo {
    action: crate::config::schema::Action,
    template_ctx: TemplateContext,
}

/// Message sent from background action task to main event loop
enum ActionResultMsg {
    Completed(std::result::Result<ActionResult, String>),
}

impl App {
    pub fn new(
        config: Config,
        adapter_registry: crate::adapters::registry::AdapterRegistry,
    ) -> Result<Self> {
        let current_page = config.start.clone();
        let nav_context = NavigationContext::new().with_globals(config.globals.clone());
        let action_executor = ActionExecutor::new(Arc::new(globals::template_engine().clone()));

        Ok(Self {
            running: false,
            current_page,
            nav_stack: NavigationStack::default(),
            nav_context,
            action_executor,
            adapter_registry: Arc::new(adapter_registry),
            current_data: Vec::new(),
            filtered_indices: Vec::new(),
            selected_index: 0,
            scroll_offset: 0,
            table_state: ratatui::widgets::TableState::default(),
            activity: ActivityState::Idle,
            spinner_frame: 0,
            error_message: None,
            global_search: GlobalSearch::default(),
            show_quit_confirm: false,
            action_confirm: None,
            last_refresh: std::time::Instant::now(),
            stream_active: false,
            stream_paused: false,
            stream_buffer: VecDeque::new(),
            stream_frozen_snapshot: None,
            stream_receiver: None,
            stream_status: StreamStatus::Idle,
            logs_follow: true,
            logs_wrap: true,
            logs_horizontal_scroll: 0,
            pending_action_info: None,
            action_result_receiver: None,
            show_action_menu: false,
            action_menu_selected: 0,
            needs_clear: false,
            needs_render: true, // Initial render needed
            refresh_receiver: None,
            page_cache: HashMap::new(),
        })
    }

    /// Parse a raw ANSI string into a LogLine with pre-parsed styled spans.
    /// Called once per line at insertion time. Sanitizes span content to remove
    /// any residual control characters (ESC, CR, BS, etc.) that ansi_to_tui
    /// didn't convert — these corrupt terminal state during ratatui rendering.
    fn parse_and_store_line(raw_ansi: &str) -> LogLine {
        use ansi_to_tui::IntoText;
        let parsed = match raw_ansi.into_text() {
            Ok(text) => {
                if text.lines.is_empty() {
                    Line::from(raw_ansi.to_string())
                } else {
                    text.lines.into_iter().next().unwrap()
                }
            }
            Err(_) => {
                // Fallback: strip all control chars from raw input
                let clean: String = raw_ansi.chars()
                    .map(|c| if c == '\t' { ' ' } else { c })
                    .filter(|c| !c.is_control())
                    .collect();
                Line::from(clean)
            }
        };
        // Sanitize each span: replace tabs with spaces, strip remaining control chars
        let sanitized_spans: Vec<Span<'static>> = parsed.spans.into_iter().map(|span| {
            let clean: String = span.content.chars()
                .map(|c| if c == '\t' { ' ' } else { c })
                .filter(|c| !c.is_control())
                .collect();
            Span::styled(clean, span.style)
        }).collect();
        let parsed = Line::from(sanitized_spans);
        // Build ANSI-stripped plain text by concatenating span contents
        let raw: String = parsed.spans.iter().map(|s| s.content.as_ref()).collect();
        LogLine { raw, parsed }
    }

    /// Truncate a pre-parsed Line at character boundaries using unicode widths.
    /// Skips `char_offset` display columns, then takes up to `width` columns.
    /// Preserves span styles through truncation.
    fn format_log_line(line: &Line<'static>, char_offset: usize, width: usize) -> Line<'static> {
        let mut result_spans: Vec<Span<'static>> = Vec::new();
        let mut cols_skipped: usize = 0;
        let mut cols_taken: usize = 0;

        for span in &line.spans {
            if cols_taken >= width {
                break;
            }
            let mut sliced = String::new();
            for ch in span.content.chars() {
                let cw = UnicodeWidthChar::width(ch).unwrap_or(0);
                if cols_skipped < char_offset {
                    cols_skipped += cw;
                    continue;
                }
                if cols_taken + cw > width {
                    break;
                }
                sliced.push(ch);
                cols_taken += cw;
            }
            if !sliced.is_empty() {
                result_spans.push(Span::styled(sliced, span.style));
            }
        }

        Line::from(result_spans)
    }

    pub async fn run(mut self, mut terminal: DefaultTerminal) -> Result<()> {
        self.running = true;

        // Load initial page (non-blocking for non-stream pages)
        self.load_current_page().await;

        while self.running {
            if self.needs_clear {
                terminal.clear()?;
                self.needs_clear = false;
            }

            // Check for background load / refresh updates
            self.check_refresh_updates();

            // Check for stream updates
            self.check_stream_updates();

            // Check for background action completion
            if let Some(action_result) = self.check_action_result() {
                match action_result {
                    ActionResult::Navigate(page, context_map) => {
                        self.navigate_to_page(&page, context_map).await;
                    }
                    ActionResult::Refresh => {
                        self.load_current_page_background();
                    }
                    _ => {}
                }
            }

            // Auto-dismiss notifications after 3 seconds
            if let ActivityState::Result { timestamp, .. } = &self.activity {
                if timestamp.elapsed() > std::time::Duration::from_secs(3) {
                    self.activity = ActivityState::Idle;
                    self.needs_render = true;
                }
            }

            // Advance spinner animation if loading
            if self.activity.is_loading() {
                self.advance_spinner();
                self.needs_render = true;
            }

            // Only render if needed (data changed, user input, etc.)
            if self.needs_render {
                // Update table state to match selected_index
                self.table_state.select(Some(self.selected_index));

                terminal.draw(|frame| self.render(frame))?;
                self.needs_render = false;
            }

            // Poll for user input with timeout
            if let Ok(true) = event::poll(std::time::Duration::from_millis(100))
                && let Event::Key(key) = event::read()?
                && key.kind == KeyEventKind::Press
            {
                self.handle_key(key).await;
                // Don't auto-render on every key press - let handlers decide
                // This allows pause mode to truly freeze the display
            }
        }

        Ok(())
    }

    fn load_current_page_background(&mut self) {
        // Show spinner while loading fresh data in background
        self.activity = ActivityState::Loading { message: "Refreshing...".into() };
        self.spinner_frame = 0;
        self.needs_render = true;

        // Get the page config
        let page = match globals::config().pages.get(&self.current_page).cloned() {
            Some(p) => p,
            None => return,
        };

        // Create a one-time channel for this background load
        let (tx, rx) = mpsc::channel(10);
        
        // Replace the existing refresh receiver (if any) with the new one
        self.refresh_receiver = Some(rx);

        let current_page = self.current_page.clone();
        let nav_context = self.nav_context.clone();
        let adapter_registry = self.adapter_registry.clone();

        // Spawn background task for one-time refresh
        tokio::spawn(async move {
            // Send started notification
            let _ = tx
                .send(RefreshMessage::Started {
                    page_name: current_page.clone(),
                })
                .await;

            match Self::fetch_data_static(&page, &nav_context, &adapter_registry).await {
                Ok(data) => {
                    let _ = tx
                        .send(RefreshMessage::Completed {
                            page_name: current_page,
                            data,
                            reset_selection: false,
                        })
                        .await;
                }
                Err(e) => {
                    let _ = tx
                        .send(RefreshMessage::Error {
                            page_name: current_page,
                            error: e.to_string(),
                        })
                        .await;
                }
            }
        });
    }

    async fn load_current_page(&mut self) {
        self.activity = ActivityState::Loading { message: format!("Loading {}...", self.current_page) };
        self.spinner_frame = 0; // Reset spinner animation
        self.error_message = None;
        self.current_data.clear();
        self.filtered_indices.clear();
        self.needs_render = true; // Force render to show spinner

        // Stop any active stream from previous page
        self.stop_stream();

        let page = match globals::config().pages.get(&self.current_page).cloned() {
            Some(p) => p,
            None => {
                self.error_message = Some(format!("Page not found: {}", self.current_page));
                self.activity = ActivityState::Idle;
                return;
            }
        };

        // Check if this is a stream data source
        if let crate::config::DataSource::SingleOrStream(crate::config::SingleOrStream::Stream(_)) =
            &page.data
        {
            // Start streaming (needs &mut self, must be synchronous)
            if let Err(e) = self.start_stream(&page).await {
                self.error_message = Some(format!("Failed to start stream: {}", e));
                self.activity = ActivityState::Idle;
            } else {
                self.activity = ActivityState::Idle;
            }
            return;
        }

        // Non-stream: spawn background task so the event loop keeps rendering the spinner
        let (tx, rx) = mpsc::channel(10);
        self.refresh_receiver = Some(rx);

        let current_page = self.current_page.clone();
        let nav_context = self.nav_context.clone();
        let adapter_registry = self.adapter_registry.clone();

        tokio::spawn(async move {
            match Self::fetch_data_static(&page, &nav_context, &adapter_registry).await {
                Ok(data) => {
                    let _ = tx.send(RefreshMessage::Completed {
                        page_name: current_page,
                        data,
                        reset_selection: true,
                    }).await;
                }
                Err(e) => {
                    let _ = tx.send(RefreshMessage::Error {
                        page_name: current_page,
                        error: e.to_string(),
                    }).await;
                }
            }
        });
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
        let adapter_registry = self.adapter_registry.clone();

        // Spawn background task
        tokio::spawn(async move {
            let mut interval_timer = tokio::time::interval(interval);
            interval_timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

            loop {
                interval_timer.tick().await;

                // Notify that refresh is starting
                if tx
                    .send(RefreshMessage::Started {
                        page_name: page_name.clone(),
                    })
                    .await
                    .is_err()
                {
                    break;
                }

                // Fetch data in background
                let data = Self::fetch_data_static(&page, &nav_context, &adapter_registry).await;

                if let Ok(data) = data {
                    // Send completion update through channel
                    if tx
                        .send(RefreshMessage::Completed {
                            page_name: page_name.clone(),
                            data,
                            reset_selection: false,
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
            match msg {
                RefreshMessage::Started { page_name } => {
                    // Mark as refreshing if it's for the current page
                    if page_name == self.current_page {
                        self.activity = ActivityState::Loading { message: "Refreshing...".into() };
                        self.spinner_frame = 0; // Reset spinner
                        self.needs_render = true;
                    }
                }
                RefreshMessage::Completed { page_name, data, reset_selection } => {
                    // Cache the refreshed data
                    self.page_cache.insert(page_name.clone(), data.clone());

                    // Update data and stop loading indicator
                    if page_name == self.current_page {
                        self.current_data = data;
                        self.apply_sort_and_filter();
                        if reset_selection {
                            self.selected_index = 0;
                            self.scroll_offset = 0;
                        }
                        self.activity = ActivityState::Idle;
                        self.last_refresh = std::time::Instant::now();
                        self.needs_render = true;

                        // Spawn/restart refresh watcher if page has refresh_interval
                        if let Some(page_config) = globals::config().pages.get(&self.current_page).cloned() {
                            self.spawn_refresh_watcher(self.current_page.clone(), page_config);
                        }
                    }
                }
                RefreshMessage::Error { page_name, error } => {
                    if page_name == self.current_page {
                        self.error_message = Some(format!("Failed to load data: {}", error));
                        self.activity = ActivityState::Idle;
                        self.needs_render = true;
                    }
                }
            }
        }
    }

    /// Advance the spinner animation to the next frame
    fn advance_spinner(&mut self) {
        self.spinner_frame = crate::ui::loading::Spinner::next_frame(self.spinner_frame);
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

                        // Add to buffer (parse ANSI once at insertion time)
                        self.stream_buffer.push_back(Self::parse_and_store_line(&line));

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

    fn create_template_context(&self, current_row: Option<&Value>) -> TemplateContext {
        // Use with_capacity for pre-allocation (optimization)
        let mut ctx =
            TemplateContext::with_capacity().with_globals(self.nav_context.globals.clone());

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
        adapter_registry: &crate::adapters::registry::AdapterRegistry,
    ) -> Result<Vec<Value>> {
        use crate::config::DataSource;

        let data_source = &page.data;

        match data_source {
            DataSource::SingleOrStream(crate::config::SingleOrStream::Single(single)) => {
                // Create data context for template rendering
                let data_context = crate::data::provider::DataContext {
                    globals: nav_context.globals.clone(),
                    page_contexts: nav_context.page_contexts.clone(),
                };

                // Fetch data using adapter registry
                let result = adapter_registry
                    .fetch(single, &data_context)
                    .await
                    .map_err(|e| crate::error::TermStackError::DataProvider(e.to_string()))?;

                // Extract items using JSONPath
                let items = if let Some(items_path) = &single.items {
                    let extractor = JsonPathExtractor::new(items_path)?;
                    extractor.extract(&result)?
                } else {
                    vec![result]
                };

                Ok(items)
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
            if confirm.executing {
                // Block all input while action is executing
                return;
            }
            match key.code {
                KeyCode::Char('y') | KeyCode::Char('Y') => {
                    let action = confirm.action.clone();
                    // Keep dialog open but mark as executing
                    if let Some(confirm) = &mut self.action_confirm {
                        confirm.executing = true;
                    }
                    self.execute_action(&action).await;
                    self.needs_render = true;
                }
                KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                    self.action_confirm = None;
                    self.needs_render = true;
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
                    self.needs_render = true;
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
                    self.update_search_mode();
                    self.needs_render = true;
                    return;
                }
                KeyCode::Backspace => {
                    self.global_search.pop_char();
                    self.update_search_mode();
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
                        self.needs_render = true;
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
                        self.needs_render = true;
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

        // Clear result notification on any key
        if matches!(self.activity, ActivityState::Result { .. }) {
            self.activity = ActivityState::Idle;
        }

        // Block action-triggering input while loading
        if self.activity.is_loading() {
            // Allow: q/Esc (quit), j/k/arrows (scroll), / (search), Backspace (back)
            // Block: Ctrl+key actions, Shift+A menu, Enter (drill-down)
            match key.code {
                KeyCode::Char('q') | KeyCode::Esc | KeyCode::Char('j') | KeyCode::Char('k')
                | KeyCode::Up | KeyCode::Down | KeyCode::Char('/') | KeyCode::Backspace => {
                    // Allow these through
                }
                _ => return,
            }
        }

        // Handle Ctrl+key combinations for direct action execution
        if key.modifiers.contains(KeyModifiers::CONTROL) {
            if let KeyCode::Char(c) = key.code {
                self.handle_ctrl_action(c).await;
                return;
            }
        }

        // Normal key handling
        match key.code {
            KeyCode::Char('q') => {
                // Always show quit confirmation
                self.show_quit_confirm = true;
                self.needs_render = true;
            }
            KeyCode::Esc => {
                // If action menu is open, close it first
                if self.show_action_menu {
                    self.show_action_menu = false;
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
            KeyCode::Char('j') | KeyCode::Down => {
                if self.show_action_menu {
                    // Navigate action menu down
                    let page = match globals::config().pages.get(&self.current_page) {
                        Some(p) => p,
                        None => return,
                    };
                    if let Some(actions) = &page.actions {
                        if !actions.is_empty() {
                            self.action_menu_selected = (self.action_menu_selected + 1) % actions.len();
                            self.needs_render = true;
                        }
                    }
                } else {
                    self.move_down();
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                if self.show_action_menu {
                    // Navigate action menu up
                    let page = match globals::config().pages.get(&self.current_page) {
                        Some(p) => p,
                        None => return,
                    };
                    if let Some(actions) = &page.actions {
                        if !actions.is_empty() {
                            if self.action_menu_selected == 0 {
                                self.action_menu_selected = actions.len() - 1;
                            } else {
                                self.action_menu_selected -= 1;
                            }
                            self.needs_render = true;
                        }
                    }
                } else {
                    self.move_up();
                }
            }
            KeyCode::Char('g') => {
                self.move_top();
            }
            KeyCode::Char('G') => self.move_bottom(),
            KeyCode::Char('r') => {
                if self.stream_active {
                    // Restart the stream
                    self.stop_stream();
                    self.load_current_page().await;
                } else {
                    // Manual refresh - use background loading for animated spinner
                    self.load_current_page_background();
                }
            }
            KeyCode::Char('/') => {
                // Activate global search
                self.global_search.activate();
                self.needs_render = true;
            }
            KeyCode::Char('f') => {
                // Toggle follow in logs view (when paused, 'f' resumes LIVE mode)
                if self.stream_active || !self.stream_buffer.is_empty() {
                    if self.stream_paused {
                        // Currently paused, resume to LIVE
                        self.stream_paused = false;
                        self.logs_follow = true;
                        // Clear the frozen snapshot
                        self.stream_frozen_snapshot = None;
                        if !self.stream_buffer.is_empty() {
                            self.selected_index = self.stream_buffer.len() - 1;
                        }
                        self.needs_render = true; // Force render when resuming
                    } else {
                        // Currently live, pause at current position
                        self.stream_paused = true;
                        self.logs_follow = false;
                        // Take a snapshot of the current buffer
                        self.stream_frozen_snapshot = Some(Arc::new(self.stream_buffer.clone()));
                        self.needs_render = true; // Force render to update status indicator
                    }
                }
            }
            KeyCode::Char('w') => {
                // Toggle wrap in logs view
                if self.stream_active || !self.stream_buffer.is_empty() {
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
                if (self.stream_active || !self.stream_buffer.is_empty()) && !self.logs_wrap {
                    self.logs_horizontal_scroll = self.logs_horizontal_scroll.saturating_sub(5);
                    // Always render user actions, even when paused
                    self.needs_render = true;
                }
            }
            KeyCode::Right => {
                // Scroll right in logs view (when wrap is off)
                if (self.stream_active || !self.stream_buffer.is_empty()) && !self.logs_wrap {
                    self.logs_horizontal_scroll = self.logs_horizontal_scroll.saturating_add(5);
                    // Always render user actions, even when paused
                    self.needs_render = true;
                }
            }
            KeyCode::Char('h') => {
                if (self.stream_active || !self.stream_buffer.is_empty()) && !self.logs_wrap {
                    // Horizontal scroll left in logs view
                    self.logs_horizontal_scroll = self.logs_horizontal_scroll.saturating_sub(5);
                    // Always render user actions, even when paused
                    self.needs_render = true;
                }
            }
            KeyCode::Char('l') => {
                if (self.stream_active || !self.stream_buffer.is_empty()) && !self.logs_wrap {
                    // Horizontal scroll right in logs view
                    self.logs_horizontal_scroll = self.logs_horizontal_scroll.saturating_add(5);
                    // Always render user actions, even when paused
                    self.needs_render = true;
                }
            }
            KeyCode::Enter => {
                if self.show_action_menu {
                    // Execute selected action from menu
                    let action_to_execute = {
                        let page = match globals::config().pages.get(&self.current_page) {
                            Some(p) => p,
                            None => return,
                        };
                        page.actions.as_ref().and_then(|actions| {
                            if self.action_menu_selected < actions.len() {
                                Some(actions[self.action_menu_selected].clone())
                            } else {
                                None
                            }
                        })
                    };

                    if let Some(action) = action_to_execute {
                        self.show_action_menu = false;
                        self.needs_render = true;
                        // Check if confirmation is needed
                        if let Some(confirm_msg) = &action.confirm {
                            let rendered_msg = globals::template_engine()
                                .render_string(
                                    confirm_msg,
                                    &self.create_template_context(self.get_selected_row()),
                                )
                                .unwrap_or_else(|_| confirm_msg.clone());
                            self.action_confirm = Some(ActionConfirm {
                                action: action.clone(),
                                message: rendered_msg,
                                executing: false,
                            });
                        } else {
                            self.execute_action(&action).await;
                        }
                    }
                } else {
                    // Normal mode: navigate to next page
                    self.navigate_next().await;
                }
            }
            KeyCode::Char('A') => {
                // Shift+A: Toggle action menu (lazygit-style)
                let page = globals::config().pages.get(&self.current_page);
                let has_actions = page
                    .and_then(|p| p.actions.as_ref())
                    .map(|a| !a.is_empty())
                    .unwrap_or(false);
                if has_actions {
                    self.show_action_menu = !self.show_action_menu;
                    if self.show_action_menu {
                        self.action_menu_selected = 0; // Reset selection when opening
                    }
                    self.needs_render = true;
                }
            }
            KeyCode::Char(_) => {
                // Ignore unmapped keys
            }
            _ => {}
        }
    }

    async fn handle_ctrl_action(&mut self, key_char: char) {

        // Find matching action by Ctrl+key or fallback to simple key for backward compatibility
        let action_to_execute = {
            let page = match globals::config().pages.get(&self.current_page) {
                Some(p) => p,
                None => return,
            };

            // Look for action with matching Ctrl+key first, then try simple key
            page.actions
                .as_ref()
                .and_then(|actions| {
                    actions
                        .iter()
                        .find(|action| {
                            if let Ok(parsed_key) = action.parse_key() {
                                // Try to match with a Ctrl key event
                                let ctrl_event = KeyEvent::new(
                                    KeyCode::Char(key_char),
                                    KeyModifiers::CONTROL
                                );
                                parsed_key.matches(&ctrl_event)
                            } else {
                                false
                            }
                        })
                        .cloned()
                })
        };

        if let Some(action) = action_to_execute {
            // Close action menu if it's open
            if self.show_action_menu {
                self.show_action_menu = false;
                self.needs_render = true;
            }

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
                    executing: false,
                });
            } else {
                // Execute immediately
                self.execute_action(&action).await;
            }
        }
    }

    /// Returns filtered line indices for the logs buffer when search filter is active.
    /// Returns None if not in logs/stream mode or no filter is active.
    fn get_logs_filtered_indices(&self) -> Option<Vec<usize>> {
        if !self.global_search.filter_active {
            return None;
        }
        if !self.stream_active && self.stream_buffer.is_empty() {
            return None;
        }
        let display_buffer: &VecDeque<LogLine> = if self.stream_paused {
            if let Some(ref snapshot) = self.stream_frozen_snapshot {
                snapshot.as_ref()
            } else {
                &self.stream_buffer
            }
        } else {
            &self.stream_buffer
        };
        let indices: Vec<usize> = display_buffer
            .iter()
            .enumerate()
            .filter(|(_, log_line)| self.global_search.matches(&log_line.raw))
            .map(|(idx, _)| idx)
            .collect();
        Some(indices)
    }

    fn get_selected_row(&self) -> Option<&Value> {
        self.filtered_indices
            .get(self.selected_index)
            .and_then(|&idx| self.current_data.get(idx))
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

    /// Update protected pages in NavigationContext based on current navigation stack
    /// Protected pages won't be evicted from the LRU cache
    fn update_protected_pages(&mut self) {
        // Clear existing protections
        self.nav_context.clear_protected();

        // Protect all pages in the navigation stack (active navigation path)
        for frame in self.nav_stack.frames() {
            self.nav_context.protect_page(&frame.page_id);
        }

        // Also protect the current page
        self.nav_context.protect_page(&self.current_page);
    }

    async fn execute_action(&mut self, action: &crate::config::schema::Action) {
        // Block concurrent actions
        if self.activity.is_loading() {
            return;
        }

        // Page navigation is instant — handle inline (no I/O)
        if let Some(page) = &action.page
            && !page.is_empty()
        {
            let page = page.clone();
            let context_map = action.context.clone();
            self.activity = ActivityState::Loading { message: format!("{}...", action.name) };
            self.navigate_to_page(&page, context_map).await;
            return;
        }

        // Capture template context and context map NOW (before user scrolls away)
        let selected_row = self.get_selected_row();
        let template_ctx = self.create_template_context(selected_row);
        let context = self.create_template_context_map();

        // Set up background execution state
        self.activity = ActivityState::Loading { message: format!("Executing: {}...", action.name) };
        self.spinner_frame = 0;
        self.needs_render = true;

        // Store pending info for result handling
        self.pending_action_info = Some(PendingActionInfo {
            action: action.clone(),
            template_ctx,
        });

        // Create channel for result
        let (tx, rx) = mpsc::channel(1);
        self.action_result_receiver = Some(rx);

        // Clone what we need for the spawned task
        let executor = self.action_executor.clone();
        let action_owned = action.clone();

        // Spawn background task
        tokio::spawn(async move {
            let result = executor.execute(&action_owned, &context).await;
            let msg = match result {
                Ok(action_result) => ActionResultMsg::Completed(Ok(action_result)),
                Err(e) => ActionResultMsg::Completed(Err(e.to_string())),
            };
            let _ = tx.send(msg).await;
        });
    }

    /// Process results from background action execution (called every event loop iteration)
    fn check_action_result(&mut self) -> Option<ActionResult> {
        let msg = {
            let receiver = self.action_result_receiver.as_mut()?;
            match receiver.try_recv() {
                Ok(msg) => msg,
                Err(_) => return None,
            }
        };

        // Clear execution state
        self.action_result_receiver = None;
        self.action_confirm = None; // Dismiss confirm dialog if it was showing executing state

        let pending = self.pending_action_info.take();

        match msg {
            ActionResultMsg::Completed(Ok(action_result)) => {
                if let Some(info) = &pending {
                    self.process_action_result(&action_result, &info.action, &info.template_ctx);
                }
                // Return Navigate/Refresh for async handling in event loop
                match action_result {
                    ActionResult::Navigate(..) | ActionResult::Refresh => Some(action_result),
                    _ => None,
                }
            }
            ActionResultMsg::Completed(Err(e)) => {
                let message = if let Some(info) = &pending {
                    if let Some(notification) = &info.action.notification {
                        if let Some(custom_msg) = &notification.on_failure {
                            globals::template_engine()
                                .render_string(custom_msg, &info.template_ctx)
                                .unwrap_or_else(|_| format!("Action failed: {}", e))
                        } else {
                            format!("Action failed: {}", e)
                        }
                    } else if let Some(error_msg) = &info.action.error_message {
                        globals::template_engine()
                            .render_string(error_msg, &info.template_ctx)
                            .unwrap_or_else(|_| format!("Action failed: {}", e))
                    } else {
                        format!("Action failed: {}", e)
                    }
                } else {
                    format!("Action failed: {}", e)
                };

                self.activity = ActivityState::Result {
                    message,
                    kind: MessageType::Error,
                    timestamp: std::time::Instant::now(),
                };
                self.needs_render = true;
                None
            }
        }
    }

    /// Process a successful action result, setting notifications as appropriate
    fn process_action_result(
        &mut self,
        result: &ActionResult,
        action: &crate::config::schema::Action,
        template_ctx: &TemplateContext,
    ) {
        match result {
            ActionResult::Success(_) => {
                // Only show notification if explicitly configured
                if let Some(notification) = &action.notification {
                    if let Some(custom_msg) = &notification.on_success {
                        let message = globals::template_engine()
                            .render_string(custom_msg, template_ctx)
                            .unwrap_or_else(|_| custom_msg.clone());

                        self.activity = ActivityState::Result {
                            message,
                            kind: MessageType::Success,
                            timestamp: std::time::Instant::now(),
                        };
                        self.needs_render = true;
                    } else {
                        self.activity = ActivityState::Idle;
                    }
                } else if let Some(success_msg) = &action.success_message {
                    let message = globals::template_engine()
                        .render_string(success_msg, template_ctx)
                        .unwrap_or_else(|_| success_msg.clone());

                    self.activity = ActivityState::Result {
                        message,
                        kind: MessageType::Success,
                        timestamp: std::time::Instant::now(),
                    };
                    self.needs_render = true;
                } else {
                    self.activity = ActivityState::Idle;
                }
            }
            ActionResult::Error(msg) => {
                let message = if let Some(notification) = &action.notification {
                    if let Some(custom_msg) = &notification.on_failure {
                        globals::template_engine()
                            .render_string(custom_msg, template_ctx)
                            .unwrap_or_else(|_| custom_msg.clone())
                    } else {
                        msg.clone()
                    }
                } else if let Some(error_msg) = &action.error_message {
                    globals::template_engine()
                        .render_string(error_msg, template_ctx)
                        .unwrap_or_else(|_| error_msg.clone())
                } else {
                    msg.clone()
                };

                self.activity = ActivityState::Result {
                    message,
                    kind: MessageType::Error,
                    timestamp: std::time::Instant::now(),
                };
                self.needs_render = true;
            }
            ActionResult::Refresh => {
                // Show success notification if configured (reload handled by caller)
                if let Some(notification) = &action.notification {
                    if let Some(custom_msg) = &notification.on_success {
                        let message = globals::template_engine()
                            .render_string(custom_msg, template_ctx)
                            .unwrap_or_else(|_| custom_msg.clone());

                        self.activity = ActivityState::Result {
                            message,
                            kind: MessageType::Success,
                            timestamp: std::time::Instant::now(),
                        };
                        self.needs_render = true;
                    } else {
                        self.activity = ActivityState::Idle;
                    }
                } else if let Some(success_msg) = &action.success_message {
                    let message = globals::template_engine()
                        .render_string(success_msg, template_ctx)
                        .unwrap_or_else(|_| success_msg.clone());

                    self.activity = ActivityState::Result {
                        message,
                        kind: MessageType::Success,
                        timestamp: std::time::Instant::now(),
                    };
                    self.needs_render = true;
                } else {
                    self.activity = ActivityState::Idle;
                }
            }
            ActionResult::Navigate(..) => {
                // Navigation handled by caller
                self.activity = ActivityState::Idle;
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

        // Clear search when navigating to new page via action
        self.global_search.clear();

        // Navigate to new page
        self.current_page = target_page.to_string();
        self.selected_index = 0;
        self.scroll_offset = 0;

        // Update protected pages in context cache (prevent eviction of active nav path)
        self.update_protected_pages();

        // Load new page data
        self.load_current_page().await;
    }

    fn move_down(&mut self) {
        // Check if we're in a text view
        if let Some(page) = globals::config().pages.get(&self.current_page)
            && matches!(page.view, ConfigView::Text(_))
        {
            // Text view: scroll down by one line
            self.scroll_offset += 1;
            self.needs_render = true;
            return;
        }

        // Logs view with filter: jump to next matching line
        if let Some(filtered) = self.get_logs_filtered_indices() {
            if let Some(&next_idx) = filtered.iter().find(|&&idx| idx > self.selected_index) {
                self.selected_index = next_idx;
                self.needs_render = true;
            }
            return;
        }

        let max_index = if self.stream_active || !self.stream_buffer.is_empty() {
            // Stream mode: use display buffer (frozen snapshot if paused)
            let display_buffer_len = if self.stream_paused
                && self
                    .stream_frozen_snapshot
                    .as_ref()
                    .is_some_and(|s| !s.is_empty())
            {
                self.stream_frozen_snapshot.as_ref().unwrap().len()
            } else {
                self.stream_buffer.len()
            };
            if display_buffer_len == 0 {
                return;
            }
            display_buffer_len - 1
        } else {
            // Table mode: use filtered data
            if self.filtered_indices.is_empty() {
                return;
            }
            self.filtered_indices.len() - 1
        };

        if self.selected_index < max_index {
            self.selected_index += 1;
            // Always render cursor movement, even when paused
            self.needs_render = true;
        }
    }

    fn move_up(&mut self) {
        // Check if we're in a text view
        if let Some(page) = globals::config().pages.get(&self.current_page)
            && matches!(page.view, ConfigView::Text(_))
        {
            // Text view: scroll up by one line
            if self.scroll_offset > 0 {
                self.scroll_offset -= 1;
                self.needs_render = true;
            }
            return;
        }

        // Logs view with filter: jump to previous matching line
        if let Some(filtered) = self.get_logs_filtered_indices() {
            if let Some(&prev_idx) = filtered.iter().rev().find(|&&idx| idx < self.selected_index) {
                self.selected_index = prev_idx;
                self.needs_render = true;
            }
            return;
        }

        if self.selected_index > 0 {
            self.selected_index -= 1;
            // Always render cursor movement, even when paused
            self.needs_render = true;
        }
    }

    fn move_top(&mut self) {
        // Check if we're in a text view
        if let Some(page) = globals::config().pages.get(&self.current_page)
            && matches!(page.view, ConfigView::Text(_))
        {
            // Text view: scroll to top
            self.scroll_offset = 0;
            self.needs_render = true;
            return;
        }

        // Logs view with filter: jump to first matching line
        if let Some(filtered) = self.get_logs_filtered_indices() {
            if let Some(&first_idx) = filtered.first() {
                self.selected_index = first_idx;
                self.needs_render = true;
            }
            return;
        }

        self.selected_index = 0;
        // Always render cursor movement, even when paused
        self.needs_render = true;
    }

    fn move_bottom(&mut self) {
        // Check if we're in a text view
        if let Some(page) = globals::config().pages.get(&self.current_page)
            && matches!(page.view, ConfigView::Text(_))
        {
            // Text view: scroll to bottom (will be clamped in render_text)
            self.scroll_offset = usize::MAX;
            self.needs_render = true;
            return;
        }

        // Logs view with filter: jump to last matching line
        if let Some(filtered) = self.get_logs_filtered_indices() {
            if let Some(&last_idx) = filtered.last() {
                self.selected_index = last_idx;
                self.needs_render = true;
            }
            return;
        }

        if self.stream_active || !self.stream_buffer.is_empty() {
            // Stream mode - jumping to bottom does NOT change pause state
            // Use display buffer (frozen snapshot if paused)
            let display_buffer_len = if self.stream_paused
                && self
                    .stream_frozen_snapshot
                    .as_ref()
                    .is_some_and(|s| !s.is_empty())
            {
                self.stream_frozen_snapshot.as_ref().unwrap().len()
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
            if !self.filtered_indices.is_empty() {
                self.selected_index = self.filtered_indices.len() - 1;
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

            self.current_page = frame.page_id.clone();
            self.selected_index = frame.selected_index;
            self.scroll_offset = frame.scroll_offset;

            // Update protected pages in context cache (popped page is no longer protected)
            self.update_protected_pages();

            // Check if we have cached data for this page
            if let Some(cached_data) = self.page_cache.get(&frame.page_id) {
                // Use cached data immediately for instant navigation
                self.current_data = cached_data.clone();
                self.apply_sort_and_filter();
                self.activity = ActivityState::Idle;
                self.needs_render = true;

                // Load fresh data in background with spinner
                self.load_current_page_background();
            } else {
                // No cache, load with spinner
                self.load_current_page().await;
            }
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
                let mut default_found = None;

                // Get selected row for condition evaluation
                let selected_row = self.get_selected_row();

                for cond in conditionals {
                    if cond.default {
                        default_found = Some((&cond.page, &cond.context));
                        continue;
                    }

                    // Evaluate condition if present
                    if let Some(condition) = &cond.condition
                        && let Some(row) = selected_row
                    {
                        let ctx = self.create_template_context(Some(row));
                        let matches = globals::template_engine()
                            .render_string(condition, &ctx)
                            .map(|result| result.trim() == "true")
                            .unwrap_or(false);

                        if matches {
                            found = Some((&cond.page, &cond.context));
                            break;
                        }
                    }
                }

                // Use first matching condition, or fall back to default
                match found.or(default_found) {
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
        if let Some(selected_row) = self.get_selected_row().cloned() {
            for (key, json_path) in context_map {
                if let Ok(extractor) = JsonPathExtractor::new(json_path)
                    && let Ok(Some(value)) = extractor.extract_single(&selected_row)
                {
                    self.nav_context.set_page_context(key.clone(), value);
                }
            }

            // Also store the entire selected row under the current page name
            self.nav_context
                .set_page_context(self.current_page.clone(), selected_row);
        }

        // Clear search when navigating to next page
        self.global_search.clear();

        // Navigate to next page
        self.current_page = next_page.clone();

        // Update protected pages in context cache (prevent eviction of active nav path)
        self.update_protected_pages();

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

        // Render action menu on top if active
        if self.show_action_menu {
            self.render_action_menu(frame, area);
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
        use ratatui::layout::{Alignment, Constraint, Direction, Layout};

        // Left side: breadcrumb navigation
        let mut left_spans = vec![
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
                left_spans.push(Span::raw(" > "));
            }
            left_spans.push(Span::styled(
                &nav_frame.page_id,
                Style::default().fg(Color::White),
            ));
        }

        // Add separator before current page if there are previous pages
        if !self.nav_stack.frames().is_empty() {
            left_spans.push(Span::raw(" > "));
        }

        // Add current page with distinct color
        left_spans.push(Span::styled(
            &self.current_page,
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ));

        // Right side: unified activity indicator
        let right_text = match &self.activity {
            ActivityState::Loading { message } => {
                let spinner_char = crate::ui::loading::get_spinner_char(self.spinner_frame);
                format!(" {} {} ", spinner_char, message)
            }
            ActivityState::Result { message, kind, .. } => {
                let icon = match kind {
                    MessageType::Success => "\u{2713}",
                    MessageType::Error => "\u{2717}",
                    MessageType::Info => "\u{2139}",
                    MessageType::Warning => "\u{26a0}",
                };
                format!(" {} {} ", icon, message)
            }
            ActivityState::Idle => String::new(),
        };

        // Cap right_text width to prevent overflow
        let max_right_width = 45_usize;
        let right_text = if right_text.chars().count() > max_right_width {
            let truncated: String = right_text.chars().take(max_right_width - 1).collect();
            format!("{}\u{2026}", truncated)
        } else {
            right_text
        };

        let right_style = match &self.activity {
            ActivityState::Loading { .. } => Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
            ActivityState::Result { kind: MessageType::Success, .. } => Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
            ActivityState::Result { kind: MessageType::Error, .. } => Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ActivityState::Result { kind: MessageType::Warning, .. } => Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
            ActivityState::Result { kind: MessageType::Info, .. } => Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD),
            ActivityState::Idle => Style::default(),
        };

        // Split the header area into left and right sections
        let header_block = Block::default().borders(Borders::ALL);
        let inner_area = header_block.inner(area);

        // Create layout for left-aligned breadcrumb and right-aligned activity
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Min(0),
                Constraint::Length(right_text.len() as u16),
            ])
            .split(inner_area);

        // Render the border block
        frame.render_widget(header_block, area);

        // Render left-aligned breadcrumb
        let breadcrumb = Paragraph::new(Line::from(left_spans)).alignment(Alignment::Left);
        frame.render_widget(breadcrumb, chunks[0]);

        // Render right-aligned activity indicator
        if !right_text.is_empty() {
            let activity_widget = Paragraph::new(right_text)
                .alignment(Alignment::Right)
                .style(right_style);
            frame.render_widget(activity_widget, chunks[1]);
        }
    }

    fn render_search_input(&self, frame: &mut Frame, area: Rect) {
        // Only renders during active input
        let search_text = format!("{}_", self.global_search.query);

        let case_indicator = if self.global_search.case_sensitive {
            " [Case-sensitive]"
        } else {
            ""
        };

        // Show column-specific or global search mode
        let scope_indicator = match &self.global_search.mode {
            SearchMode::Global => {
                if self.global_search.query.starts_with('!') {
                    " (All columns, Regex)".to_string()
                } else {
                    " (All columns)".to_string()
                }
            }
            SearchMode::ColumnSpecific { column_display_name, search_term, .. } => {
                if search_term.starts_with('!') {
                    format!(" (Column: {}, Regex)", column_display_name)
                } else {
                    format!(" (Column: {})", column_display_name)
                }
            }
        };

        let title = format!(
            "Search{}{} - Enter to apply, Esc to cancel",
            scope_indicator, case_indicator
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

        if self.filtered_indices.is_empty() {
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

        // Build rows with styling (optimized - using indices)
        let _ctx = self.create_template_context(None);
        let rows: Vec<Row> = self
            .filtered_indices
            .iter()
            .filter_map(|&data_idx| self.current_data.get(data_idx))
            .map(|item| {
                let cells: Vec<Cell> = table_config
                    .columns
                    .iter()
                    .map(|col| {
                        // Extract value using JSONPath
                        let (value_str, extracted_value) =
                            if let Ok(extractor) = JsonPathExtractor::new(&col.path) {
                                if let Ok(Some(value)) = extractor.extract_single(item) {
                                    // Apply transform if present
                                    let display_str = if let Some(transform) = &col.transform {
                                        // Create context with full row for transform
                                        let mut row_ctx = self.create_template_context(Some(item));
                                        // Add the extracted value as "value" page context for easy access in transforms
                                        row_ctx = row_ctx
                                            .with_page_context("value".to_string(), value.clone());
                                        // Also add the full row as "row" for conditions
                                        row_ctx = row_ctx
                                            .with_page_context("row".to_string(), item.clone());

                                        globals::template_engine()
                                            .render_string(transform, &row_ctx)
                                            .unwrap_or_else(|_| value_to_string(&value))
                                    } else {
                                        value_to_string(&value)
                                    };
                                    (display_str, Some(value))
                                } else {
                                    ("".to_string(), None)
                                }
                            } else {
                                ("".to_string(), None)
                            };

                        // Apply column styling
                        let cell_style = self.apply_column_style(col, &extracted_value, item);

                        // Highlight search matches in cell text
                        if self.global_search.filter_active {
                            let should_highlight = match &self.global_search.mode {
                                SearchMode::Global => true,
                                SearchMode::ColumnSpecific { column_path, .. } => col.path == *column_path,
                            };
                            if should_highlight {
                                let spans = vec![Span::styled(value_str, cell_style)];
                                let highlighted = self.global_search.highlight_search_in_spans(spans);
                                Cell::from(Line::from(highlighted))
                            } else {
                                Cell::from(value_str).style(cell_style)
                            }
                        } else {
                            Cell::from(value_str).style(cell_style)
                        }
                    })
                    .collect();

                // Apply row-level styling
                let row_style = self.apply_row_style(table_config, item);
                Row::new(cells).style(row_style)
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

    /// Apply column-level conditional styling
    fn apply_column_style(
        &self,
        col: &crate::config::TableColumn,
        value: &Option<Value>,
        row: &Value,
    ) -> Style {
        let mut style = Style::default();

        // Find the first matching style rule
        for style_rule in &col.style {
            let matches = if let Some(condition) = &style_rule.condition {
                // Evaluate condition template
                let mut ctx = self.create_template_context(Some(row));
                if let Some(val) = value {
                    ctx = ctx.with_page_context("value".to_string(), val.clone());
                }
                ctx = ctx.with_page_context("row".to_string(), row.clone());

                globals::template_engine()
                    .render_string(condition, &ctx)
                    .map(|result| result.trim() == "true")
                    .unwrap_or(false)
            } else {
                style_rule.default
            };

            if matches {
                // Apply this style
                if let Some(color_str) = &style_rule.color
                    && let Some(color) = Self::parse_color(color_str)
                {
                    style = style.fg(color);
                }
                if let Some(bg_str) = &style_rule.bg
                    && let Some(bg_color) = Self::parse_color(bg_str)
                {
                    style = style.bg(bg_color);
                }
                if style_rule.bold {
                    style = style.add_modifier(Modifier::BOLD);
                }
                if style_rule.dim {
                    style = style.add_modifier(Modifier::DIM);
                }
                break; // Use first matching rule
            }
        }

        style
    }

    /// Apply row-level conditional styling
    fn apply_row_style(&self, table_config: &crate::config::TableView, row: &Value) -> Style {
        let mut style = Style::default();

        // Find the first matching row style rule
        for style_rule in &table_config.row_style {
            let matches = if let Some(condition) = &style_rule.condition {
                // Evaluate condition template
                let ctx = self.create_template_context(Some(row));
                globals::template_engine()
                    .render_string(condition, &ctx)
                    .map(|result| result.trim() == "true")
                    .unwrap_or(false)
            } else {
                style_rule.default
            };

            if matches {
                // Apply this style
                if let Some(color_str) = &style_rule.color
                    && let Some(color) = Self::parse_color(color_str)
                {
                    style = style.fg(color);
                }
                if let Some(bg_str) = &style_rule.bg
                    && let Some(bg_color) = Self::parse_color(bg_str)
                {
                    style = style.bg(bg_color);
                }
                if style_rule.bold {
                    style = style.add_modifier(Modifier::BOLD);
                }
                if style_rule.dim {
                    style = style.add_modifier(Modifier::DIM);
                }
                break; // Use first matching rule
            }
        }

        style
    }

    /// Parse color string to ratatui Color
    fn parse_color(color_str: &str) -> Option<Color> {
        match color_str.to_lowercase().as_str() {
            "black" => Some(Color::Black),
            "red" => Some(Color::Red),
            "green" => Some(Color::Green),
            "yellow" => Some(Color::Yellow),
            "blue" => Some(Color::Blue),
            "magenta" => Some(Color::Magenta),
            "cyan" => Some(Color::Cyan),
            "gray" | "grey" => Some(Color::Gray),
            "darkgray" | "darkgrey" => Some(Color::DarkGray),
            "lightred" => Some(Color::LightRed),
            "lightgreen" => Some(Color::LightGreen),
            "lightyellow" => Some(Color::LightYellow),
            "lightblue" => Some(Color::LightBlue),
            "lightmagenta" => Some(Color::LightMagenta),
            "lightcyan" => Some(Color::LightCyan),
            "white" => Some(Color::White),
            _ => None,
        }
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

                // Highlight search matches over syntax colors
                if self.global_search.filter_active {
                    spans = self.global_search.highlight_search_in_spans(spans);
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
            let display_buffer: &VecDeque<LogLine> = if self.stream_paused {
                if let Some(ref snapshot) = self.stream_frozen_snapshot {
                    snapshot.as_ref()
                } else {
                    &self.stream_buffer
                }
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
                    .filter(|(_, log_line)| self.global_search.matches(&log_line.raw))
                    .map(|(idx, _)| idx)
                    .collect()
            } else {
                // No filter, use all indices
                (0..display_buffer.len()).collect()
            };

            // Calculate visible area
            let visible_height = area.height.saturating_sub(2) as usize; // Account for borders

            // When follow is enabled, snap to last filtered line (or last buffer line if no filter)
            if self.logs_follow && !self.stream_paused {
                if let Some(&last_idx) = filtered_indices.last() {
                    self.selected_index = last_idx;
                }
            }

            // Ensure selected_index is within bounds and lands on a filtered line
            if !filtered_indices.is_empty() {
                // Clamp to buffer bounds first
                if !display_buffer.is_empty() {
                    self.selected_index = self.selected_index.min(display_buffer.len() - 1);
                }
                // Snap to nearest filtered line if current index isn't in the filtered set
                if !filtered_indices.contains(&self.selected_index) {
                    // Find the closest filtered index
                    self.selected_index = *filtered_indices
                        .iter()
                        .min_by_key(|&&idx| (idx as isize - self.selected_index as isize).unsigned_abs())
                        .unwrap();
                }
            } else if !display_buffer.is_empty() {
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

            for &actual_idx in filtered_indices
                .iter()
                .skip(start_line)
                .take(total_lines.saturating_sub(start_line).min(visible_height))
            {
                // When wrapping is disabled, limit the number of lines to visible height
                // When wrapping is enabled, don't limit since lines may wrap to multiple rows
                if !self.logs_wrap && lines.len() >= visible_height {
                    break;
                }
                let log_line = &display_buffer[actual_idx];

                // Use pre-parsed spans (ANSI already parsed at insertion time)
                let mut parsed_line = log_line.parsed.clone();

                // Highlight search matches in log line
                if self.global_search.filter_active {
                    parsed_line = Line::from(self.global_search.highlight_search_in_spans(parsed_line.spans));
                }

                // Apply selection highlighting if this is the selected line
                if actual_idx == self.selected_index {
                    for span in &mut parsed_line.spans {
                        span.style = span.style.bg(Color::DarkGray).add_modifier(Modifier::BOLD);
                    }
                }

                // Handle wrapping if enabled
                if self.logs_wrap {
                    lines.push(parsed_line);
                } else {
                    // Single line with horizontal scroll support
                    let visual_width: usize = parsed_line.spans.iter().map(|s| UnicodeWidthStr::width(s.content.as_ref())).sum();

                    if visual_width > content_width {
                        let scroll = self.logs_horizontal_scroll.min(visual_width);
                        let has_left = scroll > 0;
                        let has_right_estimate = scroll + content_width < visual_width;
                        // Reserve columns for scroll indicators so content fits viewport
                        let indicator_cols = if has_left { 2 } else { 0 } + if has_right_estimate { 2 } else { 0 };
                        let available = content_width.saturating_sub(indicator_cols);

                        let mut result_spans: Vec<Span> = Vec::new();

                        if has_left {
                            result_spans.push(Span::styled("< ", Style::default().fg(Color::DarkGray)));
                        }

                        let truncated = Self::format_log_line(&parsed_line, scroll, available);
                        let cols_taken: usize = truncated.spans.iter().map(|s| UnicodeWidthStr::width(s.content.as_ref())).sum();
                        result_spans.extend(truncated.spans);

                        if scroll + cols_taken < visual_width {
                            result_spans.push(Span::styled(" >", Style::default().fg(Color::DarkGray)));
                        }

                        lines.push(Line::from(result_spans));
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
        // Build navigation shortcuts based on view type
        let view_kind = globals::config()
            .pages
            .get(&self.current_page)
            .map(|p| match &p.view {
                ConfigView::Table(_) => "table",
                ConfigView::Logs(_) => "logs",
                ConfigView::Text(_) => "text",
            });

        let nav_shortcuts = match view_kind.unwrap_or("table") {
            "logs" => {
                let has_buffer = self.stream_active || !self.stream_buffer.is_empty();
                if has_buffer && !self.logs_wrap {
                    "j/k: Scroll  |  h/l: Side-scroll  |  g/G: Top/Bottom  |  /: Search  |  f: LIVE/Pause  |  w: Wrap  |  r: Restart  |  ESC: Back  |  q: Quit"
                } else if has_buffer {
                    "j/k: Scroll  |  g/G: Top/Bottom  |  /: Search  |  f: LIVE/Pause  |  w: Wrap  |  r: Restart  |  ESC: Back  |  q: Quit"
                } else {
                    "q/ESC: Quit  |  r: Refresh"
                }
            }
            "text" => {
                if self.current_data.is_empty() {
                    "q/ESC: Quit  |  r: Refresh"
                } else {
                    "j/k: Scroll  |  g/G: Top/Bottom  |  /: Search  |  ESC: Back  |  r: Refresh  |  q: Quit"
                }
            }
            _ => {
                // Table view (default)
                if self.current_data.is_empty() {
                    "q/ESC: Quit  |  r: Refresh"
                } else {
                    "j/k: Move  |  g/G: Top/Bottom  |  Enter: Select  |  /: Search (%col% term)  |  ESC: Back  |  r: Refresh  |  q: Quit"
                }
            }
        };

        let row_info = if (self.stream_active || !self.stream_buffer.is_empty())
            && self.global_search.filter_active
        {
            // Logs view with filter: show filtered count
            let buffer_len = if self.stream_paused
                && self
                    .stream_frozen_snapshot
                    .as_ref()
                    .is_some_and(|s| !s.is_empty())
            {
                self.stream_frozen_snapshot.as_ref().unwrap().len()
            } else {
                self.stream_buffer.len()
            };
            if let Some(filtered) = self.get_logs_filtered_indices() {
                let filter_pos = filtered
                    .iter()
                    .position(|&idx| idx == self.selected_index)
                    .map(|p| p + 1)
                    .unwrap_or(0);
                format!(
                    "Filtered: {}/{} | Line {}/{}",
                    filtered.len(),
                    buffer_len,
                    filter_pos,
                    filtered.len()
                )
            } else {
                format!("Lines: {} | Line {}/{}", buffer_len, self.selected_index + 1, buffer_len)
            }
        } else if self.stream_active || !self.stream_buffer.is_empty() {
            // Logs view without filter
            let buffer_len = if self.stream_paused
                && self
                    .stream_frozen_snapshot
                    .as_ref()
                    .is_some_and(|s| !s.is_empty())
            {
                self.stream_frozen_snapshot.as_ref().unwrap().len()
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
                self.filtered_indices.len(),
                self.current_data.len(),
                self.selected_index + 1,
                self.filtered_indices.len()
            )
        } else {
            format!(
                "Row {}/{}",
                self.selected_index + 1,
                self.filtered_indices.len()
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

        // Build hints line (next page indicator + action hint)
        let action_line = if let Some(page) = globals::config().pages.get(&self.current_page) {
            use crate::config::Navigation;
            let mut hint_spans: Vec<Span> = Vec::new();

            // Next page hint
            if let Some(nav) = &page.next {
                let next_label = match nav {
                    Navigation::Simple(s) => s.page.clone(),
                    Navigation::Conditional(conds) => {
                        if conds.len() == 1 {
                            conds[0].page.clone()
                        } else if !conds.is_empty() {
                            format!("{}|...", conds[0].page)
                        } else {
                            String::new()
                        }
                    }
                };
                if !next_label.is_empty() {
                    hint_spans.push(Span::styled(
                        "Enter",
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD),
                    ));
                    hint_spans.push(Span::styled(
                        format!(" → {}", next_label),
                        Style::default().fg(Color::DarkGray),
                    ));
                }
            }

            // Action hint
            if page.actions.as_ref().map(|a| !a.is_empty()).unwrap_or(false) {
                if !hint_spans.is_empty() {
                    hint_spans.push(Span::styled(
                        "  |  ",
                        Style::default().fg(Color::DarkGray),
                    ));
                }
                hint_spans.push(Span::styled("Press ", Style::default().fg(Color::DarkGray)));
                hint_spans.push(Span::styled(
                    "Shift+A",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ));
                hint_spans.push(Span::styled(" for actions", Style::default().fg(Color::DarkGray)));
            }

            Line::from(hint_spans)
        } else {
            Line::from("")
        };

        let status = Paragraph::new(vec![nav_line, action_line])
            .style(Style::default().fg(Color::White))
            .block(Block::default().borders(Borders::ALL).title("Status"));

        frame.render_widget(status, area);
    }


    fn render_action_menu(&self, frame: &mut Frame, area: Rect) {
        use ratatui::layout::Alignment;
        use ratatui::widgets::Clear;

        // Get actions for current page
        let page = match globals::config().pages.get(&self.current_page) {
            Some(p) => p,
            None => return,
        };

        let actions = match &page.actions {
            Some(a) if !a.is_empty() => a,
            _ => return,
        };

        // Get selected row to show resource context in title
        let resource_name = self.get_selected_row().and_then(|row| {
            // Try common name fields in order of preference
            row.get("name")
                .or_else(|| row.pointer("/metadata/name"))
                .or_else(|| row.get("id"))
                .or_else(|| row.get("title"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        });

        // Calculate popup size based on number of actions
        let num_actions = actions.len();
        let popup_height = (num_actions + 5).min(area.height.saturating_sub(4) as usize) as u16;
        let popup_width = 70.min(area.width.saturating_sub(4));
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

        // Build the menu lines
        let mut menu_lines = vec![Line::from("")];

        for (idx, action) in actions.iter().enumerate() {
            // Parse the key to display it properly
            let key_display = action.parse_key()
                .map(|k| k.display())
                .unwrap_or_else(|_| action.key.clone());

            let description = action.description.as_deref().unwrap_or(&action.name);
            let line_text = format!("  {} - {}", key_display, description);

            // Highlight selected action
            let line = if idx == self.action_menu_selected {
                Line::from(Span::styled(
                    format!("> {}", line_text.trim_start()),
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ))
            } else {
                Line::from(Span::styled(
                    line_text,
                    Style::default().fg(Color::White),
                ))
            };

            menu_lines.push(line);
        }

        // Add navigation instructions
        menu_lines.push(Line::from(""));
        menu_lines.push(Line::from(Span::styled(
            "↑↓/jk: Navigate | Enter/Ctrl+Key: Execute | Esc: Cancel",
            Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::ITALIC),
        )));

        // Build title with resource context if available
        let title = if let Some(name) = resource_name {
            format!(" Actions for: {} ", name)
        } else {
            " Actions ".to_string()
        };

        let menu = Paragraph::new(menu_lines)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Cyan))
                    .style(Style::default().bg(Color::Black))
                    .title(Span::styled(
                        title,
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD),
                    )),
            )
            .alignment(Alignment::Left);

        frame.render_widget(menu, popup_area);
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

        if confirm.executing {
            // Show executing state with spinner
            let spinner_char = crate::ui::loading::get_spinner_char(self.spinner_frame);
            let action_name = match &self.activity {
                ActivityState::Loading { message } => message.as_str(),
                _ => "action",
            };
            let dialog_text = vec![
                Line::from(""),
                Line::from(Span::styled(
                    format!("{} {}", spinner_char, action_name),
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                )),
                Line::from(""),
                Line::from(Span::styled(
                    "Please wait...",
                    Style::default().fg(Color::DarkGray),
                )),
                Line::from(""),
            ];

            let dialog = Paragraph::new(dialog_text)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(Color::Yellow))
                        .style(Style::default().bg(Color::Black))
                        .title("Executing Action"),
                )
                .alignment(Alignment::Center);

            frame.render_widget(dialog, popup_area);
        } else {
            // Show confirmation prompt
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

    /// Update search mode based on current query and table columns (live as user types)
    fn update_search_mode(&mut self) {
        if let Some(page) = globals::config().pages.get(&self.current_page) {
            if let ConfigView::Table(table_view) = &page.view {
                self.global_search.mode = self.global_search.parse_mode(&table_view.columns);
                return;
            }
        }
        self.global_search.mode = SearchMode::Global;
    }

    fn apply_sort_and_filter(&mut self) {
        // Start with all indices (optimized - no cloning!)
        let mut indices: Vec<usize> = (0..self.current_data.len()).collect();

        // Apply global search filter if active
        if self.global_search.filter_active {
            // Get table columns if in table view
            let table_columns = if let Some(page) = globals::config().pages.get(&self.current_page) {
                if let ConfigView::Table(table_view) = &page.view {
                    Some(&table_view.columns)
                } else {
                    None
                }
            } else {
                None
            };

            // Parse search mode with column context
            if let Some(columns) = table_columns {
                self.global_search.mode = self.global_search.parse_mode(columns);
            } else {
                // Not a table view, force global search
                self.global_search.mode = SearchMode::Global;
            }

            indices = self.filter_data_indices(&indices);
        }

        // Apply sorting if configured
        if let Some(page) = globals::config().pages.get(&self.current_page)
            && let ConfigView::Table(table_view) = &page.view
            && let Some(sort_config) = &table_view.sort
        {
            self.sort_data_indices(&mut indices, sort_config);
        }

        self.filtered_indices = indices;
    }

    fn filter_data_indices(&self, indices: &[usize]) -> Vec<usize> {
        indices
            .iter()
            .filter(|&&idx| {
                if let Some(item) = self.current_data.get(idx) {
                    match &self.global_search.mode {
                        SearchMode::Global => {
                            // Existing behavior: search all fields
                            let item_text = self.item_to_searchable_text(item);
                            self.global_search.matches(&item_text)
                        }
                        SearchMode::ColumnSpecific { column_path, search_term, .. } => {
                            // New: search specific column only
                            self.matches_column_value(item, column_path, search_term)
                        }
                    }
                } else {
                    false
                }
            })
            .copied()
            .collect()
    }

    fn item_to_searchable_text(&self, item: &Value) -> String {
        use std::fmt::Write;

        let mut buffer = String::with_capacity(256); // Preallocate for typical item

        fn collect_values(val: &Value, buffer: &mut String) {
            match val {
                Value::String(s) => {
                    if !buffer.is_empty() {
                        buffer.push(' ');
                    }
                    buffer.push_str(s);
                }
                Value::Number(n) => {
                    if !buffer.is_empty() {
                        buffer.push(' ');
                    }
                    write!(buffer, "{}", n).unwrap();
                }
                Value::Bool(b) => {
                    if !buffer.is_empty() {
                        buffer.push(' ');
                    }
                    write!(buffer, "{}", b).unwrap();
                }
                Value::Array(arr) => {
                    for item in arr {
                        collect_values(item, buffer);
                    }
                }
                Value::Object(map) => {
                    for value in map.values() {
                        collect_values(value, buffer);
                    }
                }
                Value::Null => {}
            }
        }

        collect_values(item, &mut buffer);
        buffer
    }

    /// Match a specific column value against a search term
    fn matches_column_value(&self, item: &Value, column_path: &str, search_term: &str) -> bool {
        // Extract column value using JSONPath
        if let Ok(extractor) = JsonPathExtractor::new(column_path) {
            if let Ok(Some(value)) = extractor.extract_single(item) {
                // Convert value to string
                let value_str = match value {
                    Value::String(s) => s.to_string(),
                    Value::Number(n) => n.to_string(),
                    Value::Bool(b) => b.to_string(),
                    _ => return false,
                };

                // Check if search term starts with '!' for regex mode
                if search_term.starts_with('!') {
                    // Regex matching
                    let pattern = &search_term[1..];
                    if let Ok(regex) = Regex::new(pattern) {
                        return regex.is_match(&value_str);
                    }
                } else {
                    // Literal string matching (case-insensitive by default)
                    if self.global_search.case_sensitive {
                        return value_str.contains(search_term);
                    } else {
                        return value_str.to_lowercase().contains(&search_term.to_lowercase());
                    }
                }
            }
        }
        false
    }

    fn sort_data_indices(
        &self,
        indices: &mut [usize],
        sort_config: &crate::config::schema::TableSort,
    ) {
        use crate::config::schema::SortOrder;
        use crate::data::JsonPathExtractor;

        // Create extractor once for efficiency
        let extractor = match JsonPathExtractor::new(&sort_config.column) {
            Ok(ext) => ext,
            Err(_) => return, // Return unsorted if path is invalid
        };

        indices.sort_by(|&a, &b| {
            let a_item = self.current_data.get(a);
            let b_item = self.current_data.get(b);

            let cmp = match (a_item, b_item) {
                (Some(a_data), Some(b_data)) => {
                    let a_val = extractor.extract_single(a_data);
                    let b_val = extractor.extract_single(b_data);

                    match (&a_val, &b_val) {
                        (Ok(Some(av)), Ok(Some(bv))) => Self::compare_values(av, bv),
                        (Ok(Some(_)), Ok(None)) => std::cmp::Ordering::Less,
                        (Ok(None), Ok(Some(_))) => std::cmp::Ordering::Greater,
                        _ => std::cmp::Ordering::Equal,
                    }
                }
                _ => std::cmp::Ordering::Equal,
            };

            match sort_config.order {
                SortOrder::Asc => cmp,
                SortOrder::Desc => cmp.reverse(),
            }
        });
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
