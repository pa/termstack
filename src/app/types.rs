use regex::Regex;
use serde_json::Value;
use std::time::Instant;

/// Global search state that works across all views
#[derive(Debug, Clone)]
pub(super) struct GlobalSearch {
    /// Whether search input is active
    pub active: bool,
    /// The search query string
    pub query: String,
    /// Whether the filter is applied (search was confirmed)
    pub filter_active: bool,
    /// Compiled regex pattern (cached)
    pub regex_pattern: Option<Regex>,
    /// Whether to use case-sensitive search
    pub case_sensitive: bool,
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
    pub fn compile_pattern(&mut self) {
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
    pub fn matches(&self, text: &str) -> bool {
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
    pub fn activate(&mut self) {
        self.active = true;
    }

    /// Deactivate and apply filter
    pub fn apply(&mut self) {
        self.active = false;
        self.filter_active = !self.query.is_empty();
        self.compile_pattern();
    }

    /// Cancel search without applying
    pub fn cancel(&mut self) {
        self.active = false;
        self.query.clear();
        self.filter_active = false;
        self.regex_pattern = None;
    }

    /// Clear the search filter
    pub fn clear(&mut self) {
        self.query.clear();
        self.filter_active = false;
        self.regex_pattern = None;
    }

    /// Add character to query
    pub fn push_char(&mut self, c: char) {
        self.query.push(c);
    }

    /// Remove last character from query
    pub fn pop_char(&mut self) {
        self.query.pop();
    }

    /// Toggle case sensitivity
    pub fn toggle_case_sensitive(&mut self) {
        self.case_sensitive = !self.case_sensitive;
        if self.filter_active {
            self.compile_pattern();
        }
    }
}

#[derive(Debug)]
pub(super) struct RefreshMessage {
    pub page_name: String,
    pub data: Vec<Value>,
}

#[derive(Clone)]
pub(super) struct ActionConfirm {
    pub action: crate::config::schema::Action,
    pub message: String,
}

#[derive(Clone)]
pub(super) struct ActionMessage {
    pub message: String,
    pub message_type: MessageType,
    pub timestamp: Instant,
}

#[derive(Clone, Copy, PartialEq)]
#[allow(dead_code)]
pub(super) enum MessageType {
    Success,
    Error,
    Info,
    Warning,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) enum StreamStatus {
    Idle,
    Connected,
    Streaming,
    Stopped,
    Error(String),
}
