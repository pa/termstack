use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

/// Braille pattern spinner frames for smooth animation
/// Uses Unicode Braille patterns (U+2800 to U+28FF) for a professional look
pub const SPINNER_FRAMES: &[char] = &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];

/// Spinner widget for showing loading state
pub struct Spinner {
    /// Current frame index (0 to SPINNER_FRAMES.len() - 1)
    frame: usize,
    /// Style for the spinner character
    style: Style,
}

impl Spinner {
    /// Create a new spinner at frame 0
    pub fn new() -> Self {
        Self {
            frame: 0,
            style: Style::default().fg(Color::Yellow),
        }
    }

    /// Create a spinner with a specific frame
    pub fn with_frame(frame: usize) -> Self {
        Self {
            frame: frame % SPINNER_FRAMES.len(),
            style: Style::default().fg(Color::Yellow),
        }
    }

    /// Set the style for the spinner
    pub fn style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }

    /// Get the current spinner character
    pub fn current_char(&self) -> char {
        SPINNER_FRAMES[self.frame % SPINNER_FRAMES.len()]
    }

    /// Get the next frame index
    pub fn next_frame(frame: usize) -> usize {
        (frame + 1) % SPINNER_FRAMES.len()
    }
}

impl Default for Spinner {
    fn default() -> Self {
        Self::new()
    }
}

/// Render a fancy centered loading indicator with spinner in the given area
pub fn render_loading_indicator(frame: &mut Frame, area: Rect, spinner_frame: usize) {
    use ratatui::style::Modifier;
    use ratatui::text::{Line, Span};

    let spinner = Spinner::with_frame(spinner_frame);
    let spinner_char = spinner.current_char();

    // Create a fancy loading box with multiple lines
    let loading_lines = vec![
        Line::from(""),
        Line::from(vec![Span::styled(
            format!("   {}   ", spinner_char),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(""),
        Line::from(vec![
            Span::styled(
                "Loading",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("...", Style::default().fg(Color::DarkGray)),
        ]),
        Line::from(""),
    ];

    let loading = Paragraph::new(loading_lines)
        .alignment(Alignment::Center)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan))
                .style(Style::default()),
        );

    // Center both vertically and horizontally
    let vertical_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(40),
            Constraint::Length(7),
            Constraint::Percentage(53),
        ])
        .split(area);

    let horizontal_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(35),
            Constraint::Length(30),
            Constraint::Percentage(65),
        ])
        .split(vertical_chunks[1]);

    frame.render_widget(loading, horizontal_chunks[1]);
}

/// Get a spinner character for a given frame index
pub fn get_spinner_char(frame: usize) -> char {
    SPINNER_FRAMES[frame % SPINNER_FRAMES.len()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spinner_frames() {
        assert_eq!(SPINNER_FRAMES.len(), 10);
    }

    #[test]
    fn test_spinner_creation() {
        let spinner = Spinner::new();
        assert_eq!(spinner.frame, 0);
    }

    #[test]
    fn test_spinner_with_frame() {
        let spinner = Spinner::with_frame(5);
        assert_eq!(spinner.frame, 5);
        assert_eq!(spinner.current_char(), SPINNER_FRAMES[5]);
    }

    #[test]
    fn test_spinner_frame_wrapping() {
        let spinner = Spinner::with_frame(15); // Greater than SPINNER_FRAMES.len()
        assert_eq!(spinner.frame, 5); // Should wrap around
    }

    #[test]
    fn test_next_frame() {
        assert_eq!(Spinner::next_frame(0), 1);
        assert_eq!(Spinner::next_frame(9), 0); // Wraps around
        assert_eq!(Spinner::next_frame(5), 6);
    }

    #[test]
    fn test_get_spinner_char() {
        assert_eq!(get_spinner_char(0), '⠋');
        assert_eq!(get_spinner_char(9), '⠏');
        assert_eq!(get_spinner_char(10), '⠋'); // Wraps around
    }
}
