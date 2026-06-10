# MCP Observability & Progress Streaming Design

## Context
The goal is to implement MCP progress notifications for heavy queries in the `eider-mcp` server. The `rmcp` crate supports sending progress notifications via `client.notify_progress()` if a `progressToken` is provided in the tool call `Meta`. Heavy queries in `eider-mcp` are executed via DuckDB, which exposes query progress via `duckdb_query_progress` in the C API.

## Approach
We will use DuckDB's internal `duckdb_query_progress` C API to track query progress and stream it via the MCP protocol.

### 1. Extracting Progress Token
We will update the tool handlers in `EiderServer` to receive `meta: Meta` and `client: Peer<RoleServer>`. We will extract the progress token using `meta.get_progress_token()`.

### 2. Spawning the Progress Watchdog
Currently, `EiderServer::run` spawns a tokio watchdog task to enforce a wall-clock timeout. We will extend this watchdog task to periodically poll the query progress.
Since `duckdb::Connection` doesn't safely expose `duckdb_query_progress`, we will cast `Arc<InterruptHandle>` to a mirror struct to access the raw `ffi::duckdb_connection`:

```rust
struct MirrorInterruptHandle {
    conn: std::sync::Mutex<duckdb::ffi::duckdb_connection>,
}
```

By obtaining the `db_conn` safely from the Mutex inside the mirrored struct, we can call `duckdb::ffi::duckdb_query_progress(db_conn)` inside an `unsafe` block.

### 3. Streaming Progress
The watchdog will loop every 500ms. In each iteration:
- Check if `elapsed >= TOOL_TIMEOUT` (interrupt if true).
- Read the query progress via `duckdb_query_progress`.
- If `progress.percentage > 0.0` and a progress token is present, we will send a notification using `client.notify_progress`.

### 4. Updating Handlers
All tool methods in `mcp/src/server.rs` (`describe_dataset`, `estimate_cost`, `read_region`, `zonal_stats`, `list_tables`, `describe_table`, `run_sql`, `extract_point_timeseries`) will be updated to match the new `rmcp` handler signature:
```rust
    pub async fn run_sql(
        &self,
        meta: Meta,
        client: Peer<RoleServer>,
        Parameters(p): Parameters<RunSqlParams>,
    ) -> Result<CallToolResult, ErrorData> {
        self.run(client, meta.get_progress_token(), move |c| { ... })
    }
```

## Considerations & Trade-offs
- **Unsafe Code**: We use `unsafe` to cast `Arc<InterruptHandle>` into `MirrorInterruptHandle` to access the connection mutex. This relies on the memory layout of `InterruptHandle` matching `MirrorInterruptHandle`. This is reliable since `InterruptHandle` only contains the `Mutex`.
- **Performance**: Polling every 500ms is cheap and won't affect query performance.
- **Protocol Compliance**: The progress notifications align perfectly with the MCP specification for long-running tool calls.
