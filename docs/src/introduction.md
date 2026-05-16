# Introduction

Welcome to the **DuckDB GeoZarr** extension!

This loadable DuckDB extension enables you to read N-dimensional [Zarr](https://zarr.readthedocs.io/) and GeoZarr arrays directly into flat relational DuckDB vectors. 

## Why DuckDB GeoZarr?

Geospatial and climate data are frequently stored in Zarr format because it enables efficient, chunked, and compressed storage of multi-dimensional arrays (like Time × Latitude × Longitude). However, querying this data traditionally required loading it into Python (via `xarray` or `zarr-python`) before performing analytics.

This extension bridges the gap by natively streaming Zarr chunks directly into DuckDB's vectorized execution engine.

### Key Features
- **Zero-Copy Streaming**: Chunks are loaded and decoded natively into DuckDB's in-memory format without going through Python or heavy IPC overhead.
- **Cloud Native**: Natively supports reading from local filesystems, `s3://`, `http://`, and `https://` via Apache OpenDAL.
- **Parallel Scanning**: Achieves maximum S3 throughput by utilizing DuckDB's multi-threaded worker pool to fetch and decode chunks completely lock-free.
- **Spatial Pruning**: Use named parameters (like `lat_min`, `lon_max`) to filter data at the chunk-level, preventing out-of-bounds S3 requests.
- **Universal Types**: Supports Floats, Integers, Booleans, and Varchar (Strings) seamlessly. Missing data (Zarr `fill_value`s) are mapped perfectly to SQL `NULL`s.
