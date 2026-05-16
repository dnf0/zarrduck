# DuckDB GeoZarr Performance Optimizations Design

**Date:** 2026-05-15
**Status:** Approved

## 1. Purpose & Context
With the core Zarr reading and parameter-based bounds pruning fully operational, `duckdb_geozarr` is functionally complete. However, to achieve bleeding-edge performance on massive datasets, we will implement three distinct performance optimizations targeting CPU utilization, memory bandwidth, and I/O wait times.

## 2. Optimization 1: Projection Pushdown (Column Pruning)
When DuckDB executes a query like `SELECT SUM(value) FROM read_zarr(...)`, it only needs the `value` column. Generating physical coordinates for `lat`, `lon`, and `time` is wasted work.

### Architecture
- DuckDB's `InitInfo` provides access to the requested column indices via `get_column_indices()`.
- We will store a `Vec<usize>` of `projected_columns` in `IterationState` (or `ReadZarrInitData`).
- Inside the `dispatch_yield_loop!` macro, we will wrap the coordinate generation and insertion logic in an `if projected_columns.contains(&dim) { ... }` block.
- This entirely bypasses the math and memory writes for unrequested columns.

## 3. Optimization 2: Coordinate Math Optimization
The current implementation unwraps the 1D `local_chunk_cursor` into N-dimensional `[z, y, x]` coordinates by performing division and modulo operations for every element. This is extremely expensive in a hot loop.

### Architecture
- Since we process chunks sequentially and Zarr arrays are C-contiguous, we can replace the division/modulo math with simple nested counters or a stride-based state tracker.
- However, since Optimization 1 (Projection Pushdown) will completely bypass this math for aggregation queries, we will implement a simpler optimization first:
- We will hoist the division/modulo out of the inner loop where possible, or replace it with a `FastDiv` implementation (e.g., strength reduction using bitshifts for known dimensions), or simply rely on LLVM's auto-vectorization.
- *Note:* Because DuckDB processes in batches of 2048, we can track the current `[z, y, x]` coordinate across batch boundaries and only increment the inner-most dimension (`x`), wrapping to `y` when it hits the chunk boundary. This requires stateful coordinate tracking in `IterationState` rather than pure recalculation from `local_chunk_cursor`.

## 4. Optimization 3: Parallelism and Async I/O
Currently, execution is blocked while waiting for `zarrs` to download a chunk over the network.

### Architecture
- `duckdb-rs` vtab supports parallel execution if `duckdb_table_function_set_local_init` is configured, allowing multiple DuckDB worker threads to pull chunks concurrently.
- However, since the Rust bindings for parallel VTabs are highly complex and sometimes unstable, an easier and more robust approach within the Rust ecosystem is to utilize an async `tokio` runtime to pre-fetch chunks.
- We will spawn a background thread/task that fetches the *next* chunk in the bounding box while DuckDB is processing the *current* chunk.
- We will use an `std::sync::mpsc::sync_channel` (bounded channel of size 1 or 2) to pass downloaded `Vec<u8>` buffers from the fetcher thread to the `func` execution thread.

## 5. Spec Self-Review
1. **Placeholder scan:** No TBDs.
2. **Internal consistency:** The three optimizations are independent and compose well together.
3. **Scope check:** This is a large scope. We must execute it in three distinct implementation plans to avoid regressions.
4. **Ambiguity check:** The exact implementation of Async I/O vs DuckDB multi-threading is clarified (we will use Rust-native channels for pre-fetching to avoid C++ ABI complexities).
