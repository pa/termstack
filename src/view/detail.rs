use crossterm::event::KeyEvent;
use ratatui::{Frame, layout::Rect};
use serde_json::Value;

use super::renderer::{ViewAction, ViewRenderer};

/// Detail view renderer (to be implemented)
pub struct DetailView {
    data: Option<Value>,
}

impl DetailView {
    pub fn new() -> Self {
        Self { data: None }
    }
}

impl Default for DetailView {
    fn default() -> Self {
        Self::new()
    }
}

impl ViewRenderer for DetailView {
    fn render(&mut self, _frame: &mut Frame, _area: Rect, data: &[Value]) {
        self.data = data.first().cloned();
        // TODO: Implement detail rendering
    }

    fn handle_input(&mut self, _key: KeyEvent) -> ViewAction {
        // TODO: Implement input handling
        ViewAction::None
    }

    fn get_selected(&self) -> Option<&Value> {
        self.data.as_ref()
    }
}
