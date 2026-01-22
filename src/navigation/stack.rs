use serde_json::Value;
use std::collections::{HashMap, VecDeque};

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

/// Navigation stack for managing page history (optimized with VecDeque for O(1) pop_front)
#[derive(Debug, Clone)]
pub struct NavigationStack {
    frames: VecDeque<NavigationFrame>,
    max_size: usize,
}

impl NavigationStack {
    pub fn new(max_size: usize) -> Self {
        Self {
            frames: VecDeque::new(),
            max_size,
        }
    }

    pub fn push(&mut self, frame: NavigationFrame) {
        if self.frames.len() >= self.max_size {
            self.frames.pop_front(); // O(1) operation with VecDeque
        }
        self.frames.push_back(frame);
    }

    pub fn pop(&mut self) -> Option<NavigationFrame> {
        self.frames.pop_back()
    }

    pub fn current(&self) -> Option<&NavigationFrame> {
        self.frames.back()
    }

    pub fn current_mut(&mut self) -> Option<&mut NavigationFrame> {
        self.frames.back_mut()
    }

    pub fn len(&self) -> usize {
        self.frames.len()
    }

    pub fn is_empty(&self) -> bool {
        self.frames.is_empty()
    }

    pub fn frames(&self) -> &VecDeque<NavigationFrame> {
        &self.frames
    }
}

impl Default for NavigationStack {
    fn default() -> Self {
        Self::new(50)
    }
}
