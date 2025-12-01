use std::sync::Arc;

use crate::config::Config;
use crate::error::Result;

/// Router for resolving page navigation
#[derive(Debug, Clone)]
pub struct Router {
    config: Arc<Config>,
}

impl Router {
    pub fn new(config: Arc<Config>) -> Self {
        Self { config }
    }

    pub fn get_page(&self, page_id: &str) -> Result<&crate::config::Page> {
        self.config.pages.get(page_id).ok_or_else(|| {
            crate::error::TermStackError::Navigation(format!("Page not found: {}", page_id))
        })
    }

    pub fn start_page(&self) -> &str {
        &self.config.start
    }
}
