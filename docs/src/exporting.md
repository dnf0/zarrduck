# Exporting to Zarr (geozarr-cli)

While the Eider extension is designed for high-performance **reads** directly within the database engine, exporting relational data back out into N-dimensional Zarr arrays requires a different approach.

Due to the architectural limitations of DuckDB extensions (which cannot safely inspect `ClientContext` to orchestrate multi-pass analytical scans), we have built a **Companion CLI Tool** called `geozarr-cli`.

## The `geozarr-cli` Tool

`geozarr-cli` is a standalone Rust binary that natively embeds DuckDB. It allows you to run complex SQL queries against your data and stream the results directly to cloud storage as a fully compliant Zarr array.

### Key Features
- **Two-Pass Auto-Inference**: You don't need to manually calculate the shape of your N-dimensional data. The CLI automatically runs a hidden aggregation pass over your DuckDB query to infer the exact shape and bounding box of your Zarr array.
- **Lock-Free Async Uploads**: The CLI batches rows into Zarr chunks in-memory, and uses a bounded Tokio channel to upload full chunks to S3 asynchronously. This ensures your network bandwidth is saturated without risking Out-Of-Memory (OOM) crashes.
- **Cloud Native**: Like the read extension, the CLI uses Apache OpenDAL to natively stream chunks to `s3://` or the local filesystem using standard AWS credentials.

## Usage

```bash
geozarr-cli export \
  --db "my_database.duckdb" \
  --query "SELECT time, lat, lon, temperature FROM climate_model WHERE time > 2020" \
  --output "s3://my-bucket/climate_export.zarr" \
  --value-column "temperature"
```

### Arguments

- `--db <PATH>`: (Optional) The path to your DuckDB database file. If omitted, the CLI spins up an in-memory database.
- `--query <SQL>`: The SQL query to execute. The query can be as complex as you like (aggregations, joins, window functions).
- `--value-column <COL>`: **Required**. Zarr arrays hold a single N-dimensional grid of values. You must specify which column in your `SELECT` statement contains the actual data (e.g., `temperature`). **All other columns will be treated as coordinate axes** (e.g., `time`, `lat`, `lon`).
- `--output <URI>`: The destination path. Supports local paths or `s3://` URIs.
- `--chunks <JSON>`: (Optional) A JSON map defining the chunk shape for each dimension (e.g., `'{"time": 10, "lat": 100, "lon": 100}'`). If omitted, the CLI defaults to chunks of 100 per dimension.
