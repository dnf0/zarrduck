# MCP Server Coverage Design

## Overview
Expand test coverage for the `EiderServer` in `mcp/src/server.rs` from ~37% to a robust level. The `EiderServer` encapsulates an `eider_session` inside an `rmcp` server and enforces query timeouts using a wall-clock watchdog. The testing strategy will focus on unit testing the Rust struct methods and parameter structures directly, avoiding the overhead of setting up a JSON-RPC network client.

## Components

1. **Parameter Serialization**
   - Unit tests for `Bbox::to_value()` and `TimeRange::to_value()` to ensure JSON representation matches expectations.
   - Tests for the `#[serde(default)]` capabilities of the parameter structs (`EstimateCostParams`, `ReadRegionParams`, etc.) to ensure optional constraints fall back correctly.

2. **Server Instantiation & Internal `run()` Wrapper**
   - A `setup_server()` test helper that spins up an in-memory `eider_session::session()` and wraps it in `EiderServer`.
   - Directly test the `EiderServer::run()` internal method by simulating both a successful DuckDB query and a failed query (to ensure `ErrorData` is properly propagated).
   - *Note on Timeout:* The watchdog timeout is 120s. We will rely on the unit tests firing fast enough to test the watchdog setup and `abort()` teardown, rather than pausing for 120s to test the duckdb interrupt, which would slow down CI unacceptably.

3. **Tool Dispatch Validation**
   - Directly invoke the async methods on `EiderServer` (`describe_dataset`, `run_sql`, `read_region`, etc.) passing in raw struct parameters.
   - Assert that they successfully acquire the lock, spawn the watchdog, call the underlying pure functions in `tools.rs`, and return a well-formed `CallToolResult`.

## Testing Environment
- Tests will be added in a `mod tests { ... }` block at the bottom of `mcp/src/server.rs`.
- `tokio::test` will be used for all async test functions.
- The `eider_session::session()` function will provide the underlying DuckDB instance.

## Scope Limits
- We are testing the `EiderServer` wrapper, not the business logic. Complex spatial queries or STAC aggregations are tested extensively in `geozarr_core`, `extension`, and `tools.rs`. The goal here is to ensure the routing, struct parsing, and watchdog wrapper behave correctly.
