# Architecture

The DuckDB GeoZarr extension is designed with a heavy focus on network I/O optimization and lock-free concurrency. It relies on the [zarrs](https://crates.io/crates/zarrs) crate for core Zarr decoding and the [opendal](https://crates.io/crates/opendal) crate for cloud storage abstraction.

## Flattened Relational Mapping

DuckDB operates on flat, relational tables. Zarr arrays are N-dimensional. The extension bridges this gap by "flattening" the N-dimensional array into a table.

For a 3D Zarr array with dimensions `[time, lat, lon]`, the extension yields four columns:
```
time | lat | lon | value
```

Each "cell" in the Zarr array becomes a single row in DuckDB. 

### Eager Coordinate Loading
To populate the coordinate columns (`time`, `lat`, `lon`), the extension inspects the `_ARRAY_DIMENSIONS` metadata. It then looks for 1D arrays matching those names in the same Zarr store (e.g., `/lat`). 

During the `bind` phase (before the query starts), these 1D coordinate arrays are eagerly loaded into memory. When yielding data chunks, the extension calculates the global N-dimensional index of each value, and uses that index to perform an O(1) lookup into the cached coordinate arrays.

## Parallel Scanning (Lock-Free I/O)

DuckDB's extension API typically requires table scans to execute synchronously. However, network I/O (fetching chunks from S3) is incredibly slow compared to DuckDB's in-memory vectorized processing.

To achieve maximum throughput, the extension implements a **simulated Thread-Local State** architecture:

1. **Global Dispatcher:** A lightweight, Mutex-protected `GlobalState` tracks which Zarr chunk needs to be fetched next.
2. **Local Processing:** The global `InitData` holds a `HashMap` mapping DuckDB's native `std::thread::ThreadId` to a `LocalState` buffer.
3. **Lock-Free Fetching:** When a thread needs a new chunk, it locks the `GlobalState` for nanoseconds just to claim its chunk coordinates. It then drops all locks and uses OpenDAL to download its chunk over the network. 

This architecture allows DuckDB to saturate the host's network bandwidth by downloading up to 16 chunks simultaneously on a standard 16-core machine.

## Ghost Row Pruning

Zarr chunks are fixed-size, meaning the edges of an array are often padded with dummy data if the array dimensions are not perfectly divisible by the chunk size. The extension mathematically detects these boundary violations and strictly prunes these "ghost rows" out of the yielding loop, ensuring no padding artifacts leak into analytical aggregates.
