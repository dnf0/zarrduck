//! eider-mcp library: pure tool logic plus the rmcp stdio server adapter.
//!
//! The tool logic lives in rmcp-independent functions (`tools`/`result`/`guard`),
//! fully unit-tested. [`server::EiderServer`] is a thin rmcp adapter exposing
//! them as MCP tools; the `eider-mcp` binary serves it over stdio.

pub mod guard;
pub mod result;
pub mod server;
pub mod tools;
