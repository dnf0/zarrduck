# DuckDB GeoZarr Extension Design (`duckdb_geozarr`)

**Date:** 2026-05-15

## 1. Purpose & Context
The project aims to transition from S2-indexed Parquet to Zarr to better support N-dimensional scientific datasets (time, depth, variables). The ultimate goal is to enable **Agentic Data Discovery**, where LLMs can intuitively explore and query massive geospatial arrays.

While existing tools like `duckdb_zarr` offer an MVP for SQL-over-Zarr, they lack robust support for complex Blosc codecs, native Zarr v3 features, and advanced spatial pushdown. To achieve state-of-the-art performance and reliability, we will build a custom, high-performance DuckDB extension in Rust.

## 2. Architecture & Stack
The project will be a native DuckDB extension built in Rust:
- **Language:** Rust (leveraging existing expertise from `polars_s2`).
- **DuckDB Bindings:** `duckdb-rs` crate for bridging DuckDB's C++ API.
- **Zarr Backend:** `zarrs` crate, which provides comprehensive, production-ready support for Zarr v2/v3, sharding, and complex compression codecs.
- **Target Repository:** To be open-sourced under the `dnf0` GitHub organization.

## 3. Data Flow & Execution
The extension acts as a high-performance bridge between N-dimensional cloud arrays and DuckDB's relational execution engine.

1. **Query Entry:** The user (or an LLM Agent) executes a query using the extension's table function:
   `SELECT * FROM read_zarr('s3://my-bucket/climate.zarr', bbox=[...], time_range=[...])`
2. **Filter Pushdown:** DuckDB parses the SQL and passes the bounding box and time filters down to the Rust extension.
3. **Metadata Resolution:** The extension uses `zarrs` to fetch the remote `zarr.json` or `.zmetadata` and resolves the array shape and chunk grid.
4. **Chunk Pruning:** It calculates exactly which spatial/temporal chunks overlap with the requested filters.
5. **Byte Fetch & Materialization:** It fetches only the required chunks directly from S3/HTTP, decompresses them, and materializes the N-dimensional data directly into DuckDB's 1D columnar vectors.

## 4. LLM Integration Strategy
By solving the N-dimensional data access problem at the database engine level, the LLM agent integration is drastically simplified.
- An LLM (e.g., via MCP or a custom agent framework) does not need to execute arbitrary Python/Xarray code.
- It simply uses standard DuckDB SQL (Text-to-SQL) to query the Zarr stores.
- The heavy lifting of spatial chunking and N-dimensional mapping is safely abstracted away inside the Rust database engine.

## 5. Development & Testing
- **Rust Unit Tests:** Validate chunk intersection logic, metadata parsing, and `zarrs` API integration locally.
- **E2E DuckDB Tests:** Execute DuckDB queries against mock local Zarr v2 and v3 stores to ensure vectors are materialized correctly.
- **Cloud Testing:** Benchmark against S3 using realistic climate datasets to ensure memory stability and performance.
