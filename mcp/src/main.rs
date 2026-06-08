//! eider-mcp: stdio MCP server exposing curated geospatial tools over a single
//! sandboxed, stateful DuckDB session (eider + spatial).
//!
//! The tool logic lives in pure, rmcp-independent functions (`tools`/`result`),
//! fully unit-tested. The rmcp stdio adapter is added in a later task.

mod result;
// The tool functions form the public surface consumed by the rmcp adapter
// (added in a later task); until then some are exercised only by unit tests.
#[allow(dead_code)]
mod tools;

fn main() {
    eprintln!("eider-mcp (scaffold)");
}
