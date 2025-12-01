use crossterm::event::KeyEvent;
use ratatui::{Frame, layout::Rect};
use serde_json::Value;

/// Action returned by view input handling
#[derive(Debug, Clone)]
pub enum ViewAction {
    None,
    Navigate,
    Back,
    Quit,
    Search,
    YamlView,
    Refresh,
    ExecuteAction(String),
}

/// Trait for view renderers
pub trait ViewRenderer {
    fn render(&mut self, frame: &mut Frame, area: Rect, data: &[Value]);
    fn handle_input(&mut self, key: KeyEvent) -> ViewAction;
    fn get_selected(&self) -> Option<&Value>;
}
