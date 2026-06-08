//! eider-mcp binary: serve the [`eider_mcp::server::EiderServer`] over stdio.
//!
//! The tool logic and the rmcp adapter live in the `eider_mcp` library; this
//! binary just builds the session and runs the stdio transport.

use color_eyre::eyre::{Result, WrapErr};
use eider_mcp::server::EiderServer;
use rmcp::transport::stdio;
use rmcp::ServiceExt;

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;

    // Build the eider + spatial session once; it backs every tool call and keeps
    // temp-table result handles alive across calls.
    let conn = eider_session::open_session().wrap_err("open eider session")?;
    let server = EiderServer::new(conn);

    // Serve the MCP protocol over stdio (stdin/stdout); logs go to stderr.
    let service = server
        .serve(stdio())
        .await
        .wrap_err("start MCP stdio server")?;
    service.waiting().await.wrap_err("MCP server run loop")?;
    Ok(())
}
