// Input handling module for keyboard actions and key parsing
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// Represents a parsed action key from configuration
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActionKey {
    /// Simple single character key (legacy format)
    Simple(char),
    /// Control + character combination
    Ctrl(char),
}

impl ActionKey {
    /// Parse a key string from YAML configuration
    ///
    /// Supports formats:
    /// - Single char: "l", "d", "e" (legacy format)
    /// - Ctrl combination: "ctrl+l", "Ctrl+L", "CTRL+L" (case insensitive)
    ///
    /// # Examples
    /// ```
    /// let key = ActionKey::parse("l").unwrap();
    /// assert_eq!(key, ActionKey::Simple('l'));
    ///
    /// let key = ActionKey::parse("ctrl+l").unwrap();
    /// assert_eq!(key, ActionKey::Ctrl('l'));
    /// ```
    pub fn parse(s: &str) -> Result<Self, String> {
        let s = s.trim();

        if s.is_empty() {
            return Err("Key cannot be empty".to_string());
        }

        // Check for ctrl+ prefix (case insensitive)
        if let Some(stripped) = s.to_lowercase().strip_prefix("ctrl+") {
            if stripped.len() != 1 {
                return Err(format!(
                    "Invalid Ctrl combination '{}': expected single character after 'ctrl+'",
                    s
                ));
            }
            let ch = stripped.chars().next().unwrap();
            if !ch.is_ascii_alphanumeric() {
                return Err(format!(
                    "Invalid Ctrl combination '{}': character must be alphanumeric",
                    s
                ));
            }
            Ok(ActionKey::Ctrl(ch.to_ascii_lowercase()))
        } else if s.len() == 1 {
            // Single character (legacy format)
            let ch = s.chars().next().unwrap();
            Ok(ActionKey::Simple(ch))
        } else {
            Err(format!(
                "Invalid key format '{}': expected single character or 'ctrl+X'",
                s
            ))
        }
    }

    /// Check if a KeyEvent matches this ActionKey
    ///
    /// # Examples
    /// ```
    /// let key = ActionKey::Ctrl('l');
    ///
    /// // Matches Ctrl+L
    /// let event = KeyEvent::new(KeyCode::Char('l'), KeyModifiers::CONTROL);
    /// assert!(key.matches(&event));
    ///
    /// // Doesn't match plain 'l'
    /// let event = KeyEvent::new(KeyCode::Char('l'), KeyModifiers::NONE);
    /// assert!(!key.matches(&event));
    /// ```
    pub fn matches(&self, key: &KeyEvent) -> bool {
        match self {
            ActionKey::Simple(ch) => {
                matches!(key.code, KeyCode::Char(c) if c.to_ascii_lowercase() == *ch)
                    && key.modifiers.is_empty()
            }
            ActionKey::Ctrl(ch) => {
                matches!(key.code, KeyCode::Char(c) if c.to_ascii_lowercase() == *ch)
                    && key.modifiers.contains(KeyModifiers::CONTROL)
            }
        }
    }

    /// Format the key for display in UI
    ///
    /// # Examples
    /// ```
    /// assert_eq!(ActionKey::Simple('l').display(), "l");
    /// assert_eq!(ActionKey::Ctrl('l').display(), "Ctrl+L");
    /// ```
    pub fn display(&self) -> String {
        match self {
            ActionKey::Simple(ch) => ch.to_string(),
            ActionKey::Ctrl(ch) => format!("Ctrl+{}", ch.to_ascii_uppercase()),
        }
    }

    /// Get the character component of the key (without modifiers)
    pub fn char(&self) -> char {
        match self {
            ActionKey::Simple(ch) | ActionKey::Ctrl(ch) => *ch,
        }
    }

    /// Check if this is a Ctrl combination
    pub fn is_ctrl(&self) -> bool {
        matches!(self, ActionKey::Ctrl(_))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple() {
        assert_eq!(ActionKey::parse("l").unwrap(), ActionKey::Simple('l'));
        assert_eq!(ActionKey::parse("d").unwrap(), ActionKey::Simple('d'));
        assert_eq!(ActionKey::parse("1").unwrap(), ActionKey::Simple('1'));
    }

    #[test]
    fn test_parse_ctrl() {
        assert_eq!(ActionKey::parse("ctrl+l").unwrap(), ActionKey::Ctrl('l'));
        assert_eq!(ActionKey::parse("Ctrl+L").unwrap(), ActionKey::Ctrl('l'));
        assert_eq!(ActionKey::parse("CTRL+D").unwrap(), ActionKey::Ctrl('d'));
        assert_eq!(ActionKey::parse("ctrl+1").unwrap(), ActionKey::Ctrl('1'));
    }

    #[test]
    fn test_parse_errors() {
        assert!(ActionKey::parse("").is_err());
        assert!(ActionKey::parse("ctrl+").is_err());
        assert!(ActionKey::parse("ctrl+ll").is_err());
        assert!(ActionKey::parse("invalid").is_err());
    }

    #[test]
    fn test_display() {
        assert_eq!(ActionKey::Simple('l').display(), "l");
        assert_eq!(ActionKey::Ctrl('l').display(), "Ctrl+L");
        assert_eq!(ActionKey::Ctrl('d').display(), "Ctrl+D");
    }

    #[test]
    fn test_matches() {
        let simple_key = ActionKey::Simple('l');
        let ctrl_key = ActionKey::Ctrl('l');

        // Test simple key matches
        let event = KeyEvent::new(KeyCode::Char('l'), KeyModifiers::NONE);
        assert!(simple_key.matches(&event));
        assert!(!ctrl_key.matches(&event));

        // Test ctrl key matches
        let event = KeyEvent::new(KeyCode::Char('l'), KeyModifiers::CONTROL);
        assert!(!simple_key.matches(&event));
        assert!(ctrl_key.matches(&event));

        // Test case insensitivity
        let event = KeyEvent::new(KeyCode::Char('L'), KeyModifiers::CONTROL);
        assert!(ctrl_key.matches(&event));
    }
}
