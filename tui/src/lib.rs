//! TUI library for Grok Code, providing the terminal user interface with app structure, components, and event handling.

pub mod app;
pub mod components;
pub mod events;
pub mod handlers;
pub mod markdown;
pub mod state;
pub mod utils;

// Re-export main types for convenience
pub use app::App;
