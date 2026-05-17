# Performance Metrics

The DuckDB GeoZarr extension is engineered specifically to eliminate the traditional bottlenecks associated with querying N-dimensional arrays over the network.

## Architectural Advantages

Traditional Python workflows (e.g., using `xarray` and `dask` to convert Zarr to Parquet before loading into DuckDB) suffer from severe IPC (Inter-Process Communication) overhead and memory duplication.

Our extension achieves **Zero-Copy Streaming**: Zarr chunks are fetched directly from S3, decompressed natively in Rust, and yielded directly into DuckDB's internal `DataChunk` vectors using a lock-free multi-threaded architecture.

## Automated E2E Benchmarking

To ensure the extension remains highly performant, this repository maintains an automated End-to-End (E2E) Docker Compose test suite that runs against every Pull Request.

The benchmark does the following:
1. Generates a multi-dimensional Zarr array with **1,000,000 elements**.
2. Introduces coordinate arrays and missing data (Fill Values).
3. Executes an aggregation query using DuckDB.
4. Measures the Wall Clock Time and Maximum Resident Set Size (RSS).

### Benchmark Constraints
- The testing framework deliberately bypasses host CPU caching by running inside isolated, ephemeral Linux containers.
- The reported Wall Clock Time includes the DuckDB initialization phase, extension loading, and query execution.

### Expected Performance (Local Baseline)
For a 1M element array aggregation (`SELECT SUM(value) FROM read_zarr(...)`), you should expect:
- **Wall Clock Time:** Under `0.05` seconds.
- **Memory Footprint:** Highly bounded (typically under 100MB), as chunks are streamed and aggregated lazily without the entire dataset ever residing in RAM at once.

*Actual cloud performance will scale primarily with your available network bandwidth and the number of logical cores DuckDB can utilize for parallel HTTP fetching.*
