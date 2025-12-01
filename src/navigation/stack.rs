use serde_json::Value;
use std::collections::HashMap;

/// A single frame in the navigation stack
#[derive(Debug, Clone)]
pub struct NavigationFrame {
    pub page_id: String,
    pub context: HashMap<String, Value>,
    pub scroll_offset: usize,
    pub selected_index: usize,
}

impl NavigationFrame {
    pub fn new(page_id: String) -> Self {
        Self {
            page_id,
            context: HashMap::new(),
            scroll_offset: 0,
            selected_index: 0,
        }
    }
}

/// Navigation stack for managing page history
#[derive(Debug, Clone)]
pub struct NavigationStack {
    frames: Vec<NavigationFrame>,
    max_size: usize,
}

impl NavigationStack {
    pub fn new(max_size: usize) -> Self {
        Self {
            frames: Vec::new(),
            max_size,
        }
    }

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

    pub fn current_mut(&mut self) -> Option<&mut NavigationFrame> {
        self.frames.last_mut()
    }

    pub fn len(&self) -> usize {
        self.frames.len()
    }

    pub fn is_empty(&self) -> bool {
        self.frames.is_empty()
    }
}

impl Default for NavigationStack {
    fn default() -> Self {
        Self::new(50)
    }
}
