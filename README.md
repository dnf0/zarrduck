# DuckDB GeoZarr

A high-performance, cloud-native [DuckDB](https://duckdb.org/) extension for reading and writing N-dimensional [Zarr](https://zarr.readthedocs.io/) and GeoZarr arrays directly as flat relational tables.

[![Semantic Release](https://github.com/dnf0/duckdb_geozarr/actions/workflows/release.yaml/badge.svg)](https://github.com/dnf0/duckdb_geozarr/actions/workflows/release.yaml)

📖 **[Read the Full Documentation here!](https://dnf0.github.io/duckdb_geozarr/)**

## Why DuckDB GeoZarr?

Geospatial and climate data are frequently stored in Zarr format because it enables efficient, chunked, and compressed storage of multi-dimensional arrays (like Time × Latitude × Longitude). However, querying this data traditionally required loading it into Python (via `xarray` or `zarr-python`) before performing analytics, introducing massive IPC (Inter-Process Communication) and memory overhead.

This project bridges the gap with two tools:
1. **The DuckDB Extension:** Natively streams remote Zarr chunks directly into DuckDB's vectorized execution engine for lightning-fast reads.
2. **`geozarr-cli`:** A companion CLI tool that executes SQL against your data and asynchronously uploads the results back to cloud storage as an N-dimensional Zarr array.

### Key Features
- **Zero-Copy Streaming**: Chunks are loaded, decompressed, and decoded natively inside DuckDB's engine.
- **Lock-Free Parallel Scanning**: Achieves maximum S3 throughput by utilizing DuckDB's multi-threaded worker pool to fetch and decode multiple chunks simultaneously.
- **Cloud Native**: Powered by Apache OpenDAL, natively supporting reading and writing from local filesystems, `s3://`, `http://`, and `https://` with standard AWS credentials.
- **Spatial Pruning**: Filter data at the chunk-level using bounding boxes (`lat_min`, `lon_max`), preventing out-of-bounds S3 requests before they are ever made.
- **Universal Types**: Supports all common Zarr primitives (`f32`, `f64`, `i8`, `i16`, `i32`, `i64`, `u8`, `u16`, `u32`, `u64`, `bool`, and `String`).
- **Missing Data Awareness**: Missing data tokens (Zarr `fill_value`s) are mapped perfectly to true SQL `NULL`s via DuckDB's `ValidityMask`.

## Quick Start (Reading)

Download the `.duckdb_extension` binary for your platform from the [Releases page](https://github.com/dnf0/duckdb_geozarr/releases), or build it from source.

```sql
-- Allow unsigned extensions
SET allow_unsigned_extensions = true;

-- Load the extension
LOAD '/path/to/duckdb_geozarr.duckdb_extension';

-- Query a remote Zarr array, aggregating over a specific spatial bounding box
SELECT 
    time, 
    AVG(value) as mean_temp
FROM read_zarr(
    's3://climate-data/temperature.zarr', 
    lat_min := 45.0, 
    lat_max := 55.0
)
GROUP BY time;
```

## Quick Start (Writing)

Use the `geozarr-cli` tool to execute a DuckDB query and export the results into a new Zarr array.

```bash
geozarr-cli export \
  --db "my_database.duckdb" \
  --query "SELECT time, lat, lon, temperature FROM climate_model" \
  --output "s3://my-bucket/climate_export.zarr" \
  --value-column "temperature"
```

## Development

The project is structured as a Cargo workspace:
- `extension/`: The core DuckDB loadable extension.
- `cli/`: The companion `geozarr-cli` export tool.

To build both:
```bash
git clone https://github.com/dnf0/duckdb_geozarr.git
cd duckdb_geozarr
cargo build --release
```

## Documentation

Full documentation on installation, advanced spatial pruning, and architecture details can be found at [dnf0.github.io/duckdb_geozarr/](https://dnf0.github.io/duckdb_geozarr/).
