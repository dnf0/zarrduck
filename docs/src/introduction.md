# Introduction

Welcome to the **DuckDB GeoZarr** extension!

This loadable DuckDB extension enables you to read N-dimensional [Zarr](https://zarr.readthedocs.io/) and GeoZarr arrays directly into flat relational DuckDB vectors. 

## The Problem: N-Dimensional Data in a Relational World

Geospatial and climate data (such as temperature, precipitation, or flood risk models) are almost universally stored as N-dimensional arrays. The standard dimensions are typically Time, Latitude, and Longitude. 

Because of the massive scale of these datasets, they are chunked into smaller blocks and compressed, often stored in the **Zarr** format on cloud object storage (like AWS S3). 

Historically, querying this data required a heavy Python-centric workflow:
1. Spin up a Dask cluster or large Python environment.
2. Load the Zarr array using `xarray` or `zarr-python`.
3. Perform in-memory slicing and filtering.
4. Convert the result to a Pandas DataFrame or Parquet file.
5. Finally, load that flattened data into a SQL engine like DuckDB to join against customer portfolios or business logic.

This traditional workflow introduces massive I/O overhead, memory duplication, and complexity.

## The Solution: Native Streaming

The **DuckDB GeoZarr** extension bridges this gap natively. It allows DuckDB to treat a remote Zarr array on S3 exactly like a local Parquet file or CSV. 

Instead of pre-processing the data in Python, you simply write SQL:

```sql
SELECT 
    time, 
    AVG(value) as mean_temp
FROM read_zarr('s3://climate-data/temperature_2026.zarr', lat_min := 45.0, lat_max := 55.0)
GROUP BY time
ORDER BY time;
```

### Key Features
- **Zero-Copy Streaming**: Chunks are loaded, decompressed, and decoded natively inside DuckDB's engine. Data is written directly into DuckDB's in-memory `DataChunk` vectors without going through Python or IPC (Inter-Process Communication) overhead.
- **Cloud Native**: Powered by Apache OpenDAL, the extension natively supports reading from local filesystems, `s3://`, `http://`, and `https://`.
- **Parallel Scanning**: Achieves maximum S3 throughput by utilizing DuckDB's multi-threaded worker pool. The extension simulates thread-local state to fetch and decode multiple chunks simultaneously, entirely lock-free.
- **Spatial Pruning**: Network I/O is the slowest part of cloud analytics. Use named parameters (like `lat_min`, `lon_max`) to filter data at the chunk-level. The extension strictly prunes out-of-bounds chunks before they are ever requested over the network.
- **Universal Types**: Supports all common Zarr primitives (`f32`, `f64`, `i8`, `i16`, `i32`, `i64`, `u8`, `u16`, `u32`, `u64`, `bool`, and `String`) seamlessly. 
- **Missing Data Awareness**: Missing data tokens (Zarr `fill_value`s like `-9999.0` or `NaN`) are mapped perfectly to true SQL `NULL`s via DuckDB's ValidityMask, ensuring aggregations like `SUM()` and `AVG()` are mathematically correct.
