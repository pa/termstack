use serde_json::Value;
use std::collections::{HashMap, HashSet, VecDeque};
use std::fmt;

/// Default maximum number of page contexts to keep in memory
const DEFAULT_MAX_CONTEXTS: usize = 50;

/// Context for navigation and template rendering with LRU eviction
#[derive(Debug, Clone)]
pub struct NavigationContext {
    /// Global variables from config
    pub globals: HashMap<String, Value>,
    /// Page-specific contexts (selected row data from previous pages)
    pub page_contexts: HashMap<String, Value>,
    /// LRU tracking - ordered by access time (oldest first, newest last)
    access_order: VecDeque<String>,
    /// Pages that are protected from eviction (active navigation path)
    protected_pages: HashSet<String>,
    /// Maximum number of page contexts to keep
    max_size: usize,
    /// Estimated memory usage in bytes
    estimated_size_bytes: usize,
}

impl Default for NavigationContext {
    fn default() -> Self {
        Self::new()
    }
}

impl NavigationContext {
    pub fn new() -> Self {
        Self {
            globals: HashMap::new(),
            page_contexts: HashMap::new(),
            access_order: VecDeque::with_capacity(DEFAULT_MAX_CONTEXTS),
            protected_pages: HashSet::new(),
            max_size: DEFAULT_MAX_CONTEXTS,
            estimated_size_bytes: 0,
        }
    }

    /// Create a new context with a custom capacity limit
    pub fn with_capacity(max_size: usize) -> Self {
        Self {
            globals: HashMap::new(),
            page_contexts: HashMap::new(),
            access_order: VecDeque::with_capacity(max_size),
            protected_pages: HashSet::new(),
            max_size,
            estimated_size_bytes: 0,
        }
    }

    pub fn with_globals(mut self, globals: HashMap<String, Value>) -> Self {
        self.globals = globals;
        self
    }

    /// Set page context with LRU eviction
    pub fn set_page_context(&mut self, page: String, data: Value) {
        // Estimate size of new data
        let data_size = Self::estimate_value_size(&data) + page.len();

        // If page already exists, remove old size estimate
        if let Some(old_data) = self.page_contexts.get(&page) {
            let old_size = Self::estimate_value_size(old_data) + page.len();
            self.estimated_size_bytes = self.estimated_size_bytes.saturating_sub(old_size);
        }

        // Insert or update the page context
        self.page_contexts.insert(page.clone(), data);
        self.estimated_size_bytes += data_size;

        // Update access order (move to back if exists, or add new)
        if let Some(pos) = self.access_order.iter().position(|p| p == &page) {
            self.access_order.remove(pos);
        }
        self.access_order.push_back(page);

        // Evict LRU entries if we're over capacity
        while self.page_contexts.len() > self.max_size {
            if !self.evict_lru() {
                // If we can't evict (all pages protected), break to avoid infinite loop
                break;
            }
        }
    }

    pub fn get_page_context(&self, page: &str) -> Option<&Value> {
        self.page_contexts.get(page)
    }

    pub fn get_global(&self, key: &str) -> Option<&Value> {
        self.globals.get(key)
    }

    /// Mark a page as accessed (updates LRU order without inserting)
    pub fn mark_accessed(&mut self, page: &str) {
        if self.page_contexts.contains_key(page)
            && let Some(pos) = self.access_order.iter().position(|p| p == page) {
                self.access_order.remove(pos);
                self.access_order.push_back(page.to_string());
            }
    }

    /// Protect a page from LRU eviction (e.g., it's in the active navigation path)
    pub fn protect_page(&mut self, page: &str) {
        self.protected_pages.insert(page.to_string());
    }

    /// Remove protection from a page
    pub fn unprotect_page(&mut self, page: &str) {
        self.protected_pages.remove(page);
    }

    /// Clear all protected pages (useful when rebuilding protection list)
    pub fn clear_protected(&mut self) {
        self.protected_pages.clear();
    }

    /// Evict the least recently used unprotected page
    /// Returns true if a page was evicted, false if all pages are protected
    fn evict_lru(&mut self) -> bool {
        // Try to find an unprotected page to evict
        let mut checked = 0;
        let total = self.access_order.len();

        while checked < total {
            if let Some(candidate) = self.access_order.pop_front() {
                if !self.protected_pages.contains(&candidate) {
                    // Found an unprotected page - evict it
                    if let Some(data) = self.page_contexts.remove(&candidate) {
                        let size = Self::estimate_value_size(&data) + candidate.len();
                        self.estimated_size_bytes = self.estimated_size_bytes.saturating_sub(size);
                    }
                    return true;
                }
                // Page is protected - put it back at the end and try next
                self.access_order.push_back(candidate);
                checked += 1;
            } else {
                break;
            }
        }

        false // All pages are protected
    }

    /// Get statistics about the context cache
    pub fn stats(&self) -> ContextStats {
        ContextStats {
            total_pages: self.page_contexts.len(),
            protected_pages: self.protected_pages.len(),
            cache_pages: self
                .page_contexts
                .len()
                .saturating_sub(self.protected_pages.len()),
            estimated_mb: (self.estimated_size_bytes as f64 / (1024.0 * 1024.0)),
            max_pages: self.max_size,
        }
    }

    /// Estimate the memory size of a JSON value in bytes
    fn estimate_value_size(value: &Value) -> usize {
        match value {
            Value::Null => 8,
            Value::Bool(_) => 8,
            Value::Number(_) => 16,
            Value::String(s) => s.len() + 24, // String overhead
            Value::Array(arr) => {
                let base = 24; // Vec overhead
                let elements: usize = arr.iter().map(Self::estimate_value_size).sum();
                base + elements
            }
            Value::Object(obj) => {
                let base = 48; // HashMap overhead
                let keys: usize = obj.keys().map(|k| k.len() + 24).sum();
                let values: usize = obj.values().map(Self::estimate_value_size).sum();
                base + keys + values
            }
        }
    }
}

/// Statistics about the navigation context cache
#[derive(Debug, Clone)]
pub struct ContextStats {
    /// Total number of pages in cache
    pub total_pages: usize,
    /// Number of protected pages (active navigation path)
    pub protected_pages: usize,
    /// Number of cached off-path pages
    pub cache_pages: usize,
    /// Estimated memory usage in MB
    pub estimated_mb: f64,
    /// Maximum pages allowed
    pub max_pages: usize,
}

impl fmt::Display for ContextStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Context: {}/{} pages ({} protected, {} cached) | ~{:.2} MB",
            self.total_pages,
            self.max_pages,
            self.protected_pages,
            self.cache_pages,
            self.estimated_mb
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_basic_set_and_get() {
        let mut ctx = NavigationContext::new();
        ctx.set_page_context("page1".to_string(), json!({"id": 1}));

        assert!(ctx.get_page_context("page1").is_some());
        assert_eq!(ctx.page_contexts.len(), 1);
    }

    #[test]
    fn test_lru_eviction() {
        let mut ctx = NavigationContext::with_capacity(3);

        // Fill cache to capacity
        ctx.set_page_context("page1".to_string(), json!({"id": 1}));
        ctx.set_page_context("page2".to_string(), json!({"id": 2}));
        ctx.set_page_context("page3".to_string(), json!({"id": 3}));

        assert_eq!(ctx.page_contexts.len(), 3);

        // Add 4th page - should evict oldest (page1)
        ctx.set_page_context("page4".to_string(), json!({"id": 4}));

        assert_eq!(ctx.page_contexts.len(), 3);
        assert!(
            !ctx.page_contexts.contains_key("page1"),
            "page1 should be evicted"
        );
        assert!(ctx.page_contexts.contains_key("page2"));
        assert!(ctx.page_contexts.contains_key("page3"));
        assert!(ctx.page_contexts.contains_key("page4"));
    }

    #[test]
    fn test_protected_pages_not_evicted() {
        let mut ctx = NavigationContext::with_capacity(3);

        ctx.set_page_context("page1".to_string(), json!({"id": 1}));
        ctx.protect_page("page1");

        ctx.set_page_context("page2".to_string(), json!({"id": 2}));
        ctx.set_page_context("page3".to_string(), json!({"id": 3}));
        ctx.set_page_context("page4".to_string(), json!({"id": 4}));

        // page1 should still exist (protected)
        assert!(
            ctx.page_contexts.contains_key("page1"),
            "page1 should be protected"
        );
        // page2 should be evicted (oldest unprotected)
        assert!(
            !ctx.page_contexts.contains_key("page2"),
            "page2 should be evicted"
        );
        assert!(ctx.page_contexts.contains_key("page3"));
        assert!(ctx.page_contexts.contains_key("page4"));
    }

    #[test]
    fn test_access_order_updates() {
        let mut ctx = NavigationContext::with_capacity(3);

        ctx.set_page_context("page1".to_string(), json!({"id": 1}));
        ctx.set_page_context("page2".to_string(), json!({"id": 2}));

        // Access page1 again (should move to back of LRU)
        ctx.mark_accessed("page1");

        ctx.set_page_context("page3".to_string(), json!({"id": 3}));
        ctx.set_page_context("page4".to_string(), json!({"id": 4}));

        // page2 should be evicted (page1 was accessed more recently)
        assert!(
            !ctx.page_contexts.contains_key("page2"),
            "page2 should be evicted"
        );
        assert!(
            ctx.page_contexts.contains_key("page1"),
            "page1 should remain (accessed recently)"
        );
        assert!(ctx.page_contexts.contains_key("page3"));
        assert!(ctx.page_contexts.contains_key("page4"));
    }

    #[test]
    fn test_update_existing_page() {
        let mut ctx = NavigationContext::with_capacity(3);

        ctx.set_page_context("page1".to_string(), json!({"id": 1}));
        ctx.set_page_context("page2".to_string(), json!({"id": 2}));

        // Update page1
        ctx.set_page_context("page1".to_string(), json!({"id": 1, "updated": true}));

        assert_eq!(ctx.page_contexts.len(), 2);
        assert_eq!(
            ctx.get_page_context("page1").unwrap()["updated"],
            json!(true)
        );
    }

    #[test]
    fn test_protection_toggle() {
        let mut ctx = NavigationContext::with_capacity(2);

        ctx.set_page_context("page1".to_string(), json!({"id": 1}));
        ctx.protect_page("page1");

        ctx.set_page_context("page2".to_string(), json!({"id": 2}));
        ctx.set_page_context("page3".to_string(), json!({"id": 3}));

        // page1 protected, page2 evicted
        assert!(ctx.page_contexts.contains_key("page1"));
        assert!(!ctx.page_contexts.contains_key("page2"));
        assert!(ctx.page_contexts.contains_key("page3"));

        // Unprotect page1
        ctx.unprotect_page("page1");
        ctx.set_page_context("page4".to_string(), json!({"id": 4}));

        // page3 is evicted (oldest unprotected), page1 remains (was moved to end during earlier eviction check)
        assert!(
            ctx.page_contexts.contains_key("page1"),
            "page1 should remain (moved to end during protection)"
        );
        assert!(
            !ctx.page_contexts.contains_key("page3"),
            "page3 should be evicted (oldest unprotected)"
        );
        assert!(ctx.page_contexts.contains_key("page4"));
    }

    #[test]
    fn test_clear_protected() {
        let mut ctx = NavigationContext::new();

        ctx.protect_page("page1");
        ctx.protect_page("page2");
        ctx.protect_page("page3");

        assert_eq!(ctx.protected_pages.len(), 3);

        ctx.clear_protected();

        assert_eq!(ctx.protected_pages.len(), 0);
    }

    #[test]
    fn test_stats() {
        let mut ctx = NavigationContext::with_capacity(10);

        ctx.set_page_context("page1".to_string(), json!({"id": 1}));
        ctx.set_page_context("page2".to_string(), json!({"id": 2}));
        ctx.protect_page("page1");

        let stats = ctx.stats();

        assert_eq!(stats.total_pages, 2);
        assert_eq!(stats.protected_pages, 1);
        assert_eq!(stats.cache_pages, 1);
        assert_eq!(stats.max_pages, 10);
        assert!(stats.estimated_mb > 0.0);
    }

    #[test]
    fn test_memory_estimation() {
        let mut ctx = NavigationContext::new();

        // Create a large array manually
        let items: Vec<Value> = (0..100).map(|i| json!({"id": i, "name": "test"})).collect();

        let large_data = json!({
            "items": items,
            "metadata": {
                "count": 100,
                "description": "A large object with many items"
            }
        });

        ctx.set_page_context("large_page".to_string(), large_data);

        let stats = ctx.stats();
        assert!(stats.estimated_mb > 0.001); // Should be at least a few KB
    }

    #[test]
    fn test_all_protected_no_eviction() {
        let mut ctx = NavigationContext::with_capacity(2);

        ctx.set_page_context("page1".to_string(), json!({"id": 1}));
        ctx.set_page_context("page2".to_string(), json!({"id": 2}));

        // Protect both pages
        ctx.protect_page("page1");
        ctx.protect_page("page2");

        // Try to add a 3rd - page3 itself is not protected, so it will be evicted
        ctx.set_page_context("page3".to_string(), json!({"id": 3}));

        // page1 and page2 should still exist (protected)
        // page3 should be evicted (not protected, and we're over capacity)
        assert!(ctx.page_contexts.contains_key("page1"));
        assert!(ctx.page_contexts.contains_key("page2"));
        assert!(
            !ctx.page_contexts.contains_key("page3"),
            "page3 should be evicted (not protected)"
        );
        assert_eq!(ctx.page_contexts.len(), 2);
    }
}
