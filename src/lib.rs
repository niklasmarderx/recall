//! recall — local-first long-term memory for AI coding agents.
//!
//! The crate exposes a small SQLite-backed [`store::Store`] and an
//! [`mcp`] module that speaks the Model Context Protocol over stdio.

pub mod mcp;
pub mod store;
