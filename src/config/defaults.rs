use std::collections::HashMap;

/// Default keybindings for the application
pub fn default_keybindings() -> HashMap<String, String> {
    let mut bindings = HashMap::new();

    // Global
    bindings.insert("q".to_string(), "quit".to_string());
    bindings.insert("?".to_string(), "help".to_string());
    bindings.insert("Esc".to_string(), "back".to_string());
    bindings.insert("Ctrl+c".to_string(), "quit".to_string());

    // Navigation
    bindings.insert("j".to_string(), "down".to_string());
    bindings.insert("k".to_string(), "up".to_string());
    bindings.insert("g".to_string(), "top".to_string());
    bindings.insert("G".to_string(), "bottom".to_string());
    bindings.insert("Enter".to_string(), "select".to_string());
    bindings.insert("h".to_string(), "back".to_string());
    bindings.insert("l".to_string(), "forward".to_string());

    // Actions
    bindings.insert("r".to_string(), "refresh".to_string());
    bindings.insert("/".to_string(), "search".to_string());
    bindings.insert(":".to_string(), "command".to_string());
    bindings.insert("y".to_string(), "yaml_view".to_string());

    // Table specific
    bindings.insert("Space".to_string(), "toggle_select".to_string());
    bindings.insert("a".to_string(), "select_all".to_string());
    bindings.insert("s".to_string(), "sort".to_string());
    bindings.insert("S".to_string(), "sort_desc".to_string());

    // Detail view scrolling
    bindings.insert("Ctrl+d".to_string(), "page_down".to_string());
    bindings.insert("Ctrl+u".to_string(), "page_up".to_string());

    bindings
}

/// Default theme configuration
pub fn default_theme() -> ThemeConfig {
    ThemeConfig {
        name: "default".to_string(),
        colors: default_colors(),
    }
}

#[derive(Debug, Clone)]
pub struct ThemeConfig {
    pub name: String,
    pub colors: HashMap<String, String>,
}

fn default_colors() -> HashMap<String, String> {
    let mut colors = HashMap::new();

    colors.insert("fg".to_string(), "white".to_string());
    colors.insert("bg".to_string(), "black".to_string());
    colors.insert("border".to_string(), "gray".to_string());
    colors.insert("selected".to_string(), "blue".to_string());
    colors.insert("success".to_string(), "green".to_string());
    colors.insert("error".to_string(), "red".to_string());
    colors.insert("warning".to_string(), "yellow".to_string());
    colors.insert("info".to_string(), "cyan".to_string());

    colors
}
