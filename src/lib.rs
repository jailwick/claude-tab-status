//! Mirrors Claude Code's status glyph from pane titles into the names of tabs the user named by
//! hand.
//!
//! Everything here is plain Rust with no zellij dependency, so the naming logic is testable on
//! the host. The zellij plugin in `main.rs` only translates events into [`tracker::TabView`]s and
//! applies the [`tracker::Rename`]s it gets back.

pub mod config;
pub mod naming;
pub mod status;
pub mod ticker;
pub mod tracker;
