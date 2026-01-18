use crossterm::event::KeyEvent;
use ratatui::{Frame, layout::Rect};
use serde_json::Value;

use super::renderer::{ViewAction, ViewRenderer};

/// YAML view renderer (to be implemented)
pub struct YamlView {
    data: Option<Value>,
}

impl YamlView {
    pub fn new() -> Self {
        Self { data: None }
    }
}

impl Default for YamlView {
    fn default() -> Self {
        Self::new()
    }
}

impl ViewRenderer for YamlView {
    fn render(&mut self, _frame: &mut Frame, _area: Rect, data: &[Value]) {
        self.data = data.first().cloned();
        // TODO: Implement YAML rendering
    }

    fn handle_input(&mut self, _key: KeyEvent) -> ViewAction {
        // TODO: Implement input handling
        ViewAction::None
    }

    fn get_selected(&self) -> Option<&Value> {
        self.data.as_ref()
    }
}
