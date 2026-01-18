use crossterm::event::KeyEvent;
use ratatui::{Frame, layout::Rect};
use serde_json::Value;

use super::renderer::{ViewAction, ViewRenderer};

/// Help overlay renderer (to be implemented)
pub struct HelpView;

impl HelpView {
    pub fn new() -> Self {
        Self
    }
}

impl Default for HelpView {
    fn default() -> Self {
        Self::new()
    }
}

impl ViewRenderer for HelpView {
    fn render(&mut self, _frame: &mut Frame, _area: Rect, _data: &[Value]) {
        // TODO: Implement help rendering
    }

    fn handle_input(&mut self, _key: KeyEvent) -> ViewAction {
        // TODO: Implement input handling
        ViewAction::None
    }

    fn get_selected(&self) -> Option<&Value> {
        None
    }
}
