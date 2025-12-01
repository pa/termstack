use crossterm::event::KeyEvent;
use ratatui::{Frame, layout::Rect};
use serde_json::Value;

use super::renderer::{ViewAction, ViewRenderer};

/// Table view renderer (to be implemented)
pub struct TableView {
    rows: Vec<Value>,
    selected_index: usize,
}

impl TableView {
    pub fn new() -> Self {
        Self {
            rows: Vec::new(),
            selected_index: 0,
        }
    }
}

impl Default for TableView {
    fn default() -> Self {
        Self::new()
    }
}

impl ViewRenderer for TableView {
    fn render(&mut self, _frame: &mut Frame, _area: Rect, data: &[Value]) {
        self.rows = data.to_vec();
        // TODO: Implement table rendering
    }

    fn handle_input(&mut self, _key: KeyEvent) -> ViewAction {
        // TODO: Implement input handling
        ViewAction::None
    }

    fn get_selected(&self) -> Option<&Value> {
        self.rows.get(self.selected_index)
    }
}
