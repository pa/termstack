// Module for types and helper structs
mod types;

// Re-export types for use in app.rs
pub(crate) use types::{
    ActionConfirm, ActionMessage, GlobalSearch, MessageType, RefreshMessage, StreamStatus,
};

// TODO: Future refactoring can split app.rs further into:
// - render.rs: All rendering functions (render_table, render_content, etc.)
// - input.rs: Event handling (handle_key, move_up, move_down, etc.)
// - data.rs: Data fetching and management (fetch_page_data, apply_sort_and_filter, etc.)
// - state.rs: State management helpers
//
// For now, keeping App implementation in ../app.rs to minimize disruption
