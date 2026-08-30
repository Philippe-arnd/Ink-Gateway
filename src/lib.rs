//! `ink_core` — the shared engine behind `ink-cli` and `ink-gateway-mcp`.
//!
//! File/git-native book model: every book is a working copy on disk, synced
//! through git. This library also backs `Ink-Gateway-App`'s web API directly
//! (as a git dependency) — no subprocess shelling, no duplicated logic.

pub mod book;
pub mod comments;
pub mod config;
pub mod context;
pub mod edit;
pub mod git;
pub mod init;
pub mod maintenance;
pub mod state;
