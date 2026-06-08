# Design: eider MCP server

- **Date:** 2026-06-08
- **Status:** Approved (design); implementation pending
- **Scope:** A new `eider-mcp` Rust crate in the workspace — a Model Context Protocol (MCP) server (stdio) that lets an LLM/agent drive eider's SQL-style geospatial analytics over Zarr/GeoZarr/COG/STAC directly, via curated typed tools plus a `run_sql` escape hatch over a single, sandboxed, stateful DuckDB session. Rust workstream + a docs page.

## Context & value

eider today is a DuckDB loadable extension (`read_geo`, `plan_read_geo`, `read_zarr_metadata`) + a CLI. An agent *could* already drive it via raw SQL or by shelling the CLI, so an MCP must add value — it does, on three axes:

1. **Correctness by construction** — tools encode the patterns validated this session: bbox/time **pushdown** on reads (chunk pruning), and the **zonal convention** choice (centroid / all-touched / area-weighted ↔ `ST_Contains` / `ST_Intersects` / `ST_Intersection`). The agent picks intent, not fragile SQL.
2. **Safety for autonomous use** — a read-only, path-sandboxed session with timeouts and row caps; full results stay server-side (handle), never dumped into the agent's context.
3. **Stateful composition** — one DuckDB connection per server process, so `read_region` → temp table → `run_sql` → `zonal_stats` compose across calls.

The `run_sql` escape hatch covers anything the curated tools don't.

## Decisions (from brainstorming)

| Decision | Choice |
|---|---|
| Surface | Curated geo tools **+ `run_sql` escape hatch** over the same session. |
| Implementation | **Rust crate** in the workspace using the `rmcp` MCP SDK + the `duckdb` crate, reusing eider's connection/extension setup. |
| Safety/results | **Sandboxed read-only + capped results**: `GEOZARR_ALLOW_PATH` enforced, statement timeout, row cap on returned data, full result → session temp table (optional Parquet) referenced by handle. |
| Tool set (v1) | **Core read/analyze:** `describe_dataset`, `estimate_cost`, `read_region`, `zonal_stats`, `list_tables`, `describe_table`, `run_sql`. |
| Transport | **stdio** (the standard local MCP transport). |

## Architecture

- New workspace crate `mcp/` producing the `eider-mcp` binary.
- Uses `rmcp` (official Rust MCP SDK) over **stdio**; exposes a tool server implementing the v1 tools.
- Holds **one `duckdb::Connection`** for the process lifetime (the "session"). Temp tables created by one tool persist for later tools/`run_sql`.
- **Connection setup is the refactored, shared eider session** (see "Reuse" below): in-memory DuckDB, eider table functions loaded, `INSTALL spatial; LOAD spatial;`, `GEOZARR_ALLOW_PATH` honored, read-only filesystem posture, a configured statement timeout.
- Result discipline: every data-returning tool caps rows returned to the agent and materializes the full result to a session temp table (`mcp_result_<n>`), returning that name as a **handle** plus head rows + summary.

## Components

### 1. Shared session crate/module (refactor — prevents drift)
`cli/src/duckdb_utils.rs` already has `setup_duckdb()` + `load_geozarr_extension()` (and `inject_s3_secret`, `format_pins*`). Extract the connection-construction (eider extension load + spatial load + sandbox/timeout configuration) into a **shared library** both `cli` and `mcp` depend on — either a new `eider_session` crate or a `lib.rs` target on the existing `cli` crate (the plan picks the lower-churn option). Both `eider shell` and `eider-mcp` then build their connection through the same code path, so extension-loading and sandbox rules can't diverge. Add an MCP-oriented constructor variant that additionally applies: read-only filesystem posture, statement timeout, and exposes the row-cap config.

### 2. MCP tool server (`mcp/src`)
The `rmcp` server with these tool handlers (JSON-schema inputs; compact JSON outputs):

- **`describe_dataset(uri)`** → `SELECT * FROM read_zarr_metadata(uri)` → `{array_shape, chunk_shape, data_type, crs}`.
- **`estimate_cost(uri, {lat_min?,lat_max?,lon_min?,lon_max?,time_min?,time_max?})`** → `plan_read_geo(...)` → `{total_chunks, total_bytes}`. Lets the agent gate heavy reads.
- **`read_region(uri, {bbox?, time?, asset?, limit?=1000})`** → `read_geo(uri, lon_min:=…, …)` with the bbox/time **pushed down**; returns `{columns, rows: head≤limit, row_count, summary, table_handle}` and stores the full result in a temp table.
- **`zonal_stats({grid_uri, polygons, value?, metric, convention})`** — `polygons` is a path/GeoJSON (or a session table) read via `ST_Read`; `metric ∈ {max,min,mean,sum,count}`; `convention ∈ {centroid, all_touched, area_weighted}` mapping to `ST_Contains(asset, cell_center)` / `ST_Intersects(asset, cell_box)` / `ST_Intersection` area-weighting. Reads the grid via `read_geo` with the polygons' combined bbox pushed down (`ST_Extent_Agg`). Returns per-polygon results capped + handle. Errors clearly if `area_weighted` is requested with `max`/`min` (area weighting applies to mean/sum).
- **`list_tables()`** → session tables (name, row count). **`describe_table(name)`** → column schema + a few sample rows.
- **`run_sql(sql)`** → executes on the session connection (subject to the read-only/sandbox/timeout/row-cap rules); returns head rows + summary + handle.

Each tool's description tells the agent *when* to use it (e.g., "call `estimate_cost` before a large `read_region`"; "choose `all_touched` for worst-case hazard `max`, `area_weighted` for area-true `mean`") — encoding the guidance from the zonal-stats docs.

### 3. Safety model
- Connection opened with eider's unsigned-extension flag (needed to load eider) but **no other extensions installable** at runtime by `run_sql` (block `INSTALL`/`LOAD` of arbitrary extensions, and `ATTACH`/`COPY ... TO`/file writes outside a designated scratch dir).
- `GEOZARR_ALLOW_PATH` sandbox honored for all reads (inherited from eider/the local store).
- **Statement timeout** and **returned-row cap** enforced on every tool; full results live only in server-side temp tables / optional Parquet under the scratch dir, surfaced by handle.
- `run_sql` is parsed/guarded to reject write/DDL-to-disk and extension-install statements (allow `SELECT`, `CREATE TEMP TABLE`, `WITH`, read-side spatial functions). The exact allow/deny enforcement (statement-kind check vs read-only connection config) is finalized in the plan, preferring DuckDB's own read-only/config guarantees where they exist over hand-rolled SQL parsing.

## Testing (offline)
- **Tool-handler unit/integration tests** (in `mcp/`): call each handler directly against the committed `climate_data.zarr` sample + a committed polygon fixture; assert: `describe_dataset` returns the right dtype/CRS; `estimate_cost` returns chunk/byte counts; `read_region` honors `limit` + returns a usable handle; `zonal_stats` results match the conventions (cross-check a `centroid` vs `all_touched` difference, consistent with the zonal-stats docs); `run_sql` rejects a disallowed write and accepts a SELECT.
- **Protocol smoke test:** drive the server over stdio (or the rmcp in-process test harness) through `initialize` → `tools/list` (all 7 present with schemas) → a `tools/call` for `describe_dataset`.
- `GEOZARR_ALLOW_PATH` set to the repo for tests, mirroring the existing e2e tests. Full `cargo test`, clippy on touched crates, and `cargo fmt --check` pass.

## Docs
- New page (Guides or a top-level "MCP" entry): client config snippet (the `eider-mcp` command + args, and how to register it with an MCP client), the v1 tool reference, the safety/sandbox model, and a worked agent flow (`describe_dataset` → `estimate_cost` → `zonal_stats`). Cross-link the zonal-stats engineering note for the convention rationale. Docs build stays green.

## Non-goals (v1)
- HTTP/SSE/remote transport (stdio only) and any authentication (local-stdio trust model).
- CLI-mirror workflow tools (`extract`/`resample`/`plot`/`ingest`/`export`) — covered by `run_sql` + the core tools; revisit if demand appears.
- Writing/ingesting data *through* the MCP (read/analyze only in v1).
- Multi-session/multi-tenant servers; one connection per process.
- Any change to the eider extension or CLI behavior beyond the shared-setup refactor.
