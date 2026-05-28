//! # mycelium-mcp
//!
//! Model Context Protocol server for Mycelium.
//!
//! Exposes the engine to AI agents (Claude Code, Cursor, etc.) over the
//! MCP protocol. The server is a thin adapter — all real work lives in
//! `mycelium-core` and `mycelium-hyphae`.
//!
//! **Status: scaffold only.** Real implementation lands after RFC-0001
//! and RFC-0003 ship.

#![doc(html_root_url = "https://docs.rs/mycelium-mcp")]

/// MCP server placeholder.
#[derive(Debug, Default)]
pub struct Server {
    _placeholder: (),
}

impl Server {
    /// Construct an empty server.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}
