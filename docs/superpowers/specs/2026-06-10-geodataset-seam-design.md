# Deepening the GeoDataset Seam

## Overview

The DuckDB extension is currently tightly coupled to the `ZarrDataset` implementation. To fulfill the requirements of ADR 001 (Unified `read_geo` Table Function), we need to abstract the execution of spatial queries behind a `GeoDataset` interface. This allows the extension to query both `ZarrDataset` and `FeatureCollectionDataset` (STAC) without any storage-specific logic.

## Architecture

We are adopting an In-process Adapter pattern. The DuckDB extension will act as a thin C-FFI wrapper that depends only on the `GeoDataset` and `ChunkStream` interfaces. `geozarr_core` will own the implementations and provide a factory function to instantiate the correct adapter based on the URI.

### Interfaces

```rust
use crate::query_planner::QueryConstraints;
use crate::types::ChunkBuffer;
use zarrs::array::DataType;

/// The seam between DuckDB and the underlying storage module.
pub trait GeoDataset: Send + Sync {
    /// Entry Point 1: Discover the schema of the dataset without reading data.
    fn schema(&self) -> Result<Vec<(String, DataType)>, Box<dyn std::error::Error>>;

    /// Entry Point 2: Plan a spatial/temporal scan and prepare an executable stream.
    fn scan(
        &self, 
        constraints: &QueryConstraints
    ) -> Result<Box<dyn ChunkStream>, Box<dyn std::error::Error>>;
}

/// The adapter for DuckDB's parallel workers to pull data.
pub trait ChunkStream: Send + Sync {
    /// The estimated maximum number of chunks this scan will produce. 
    /// Used to size the DuckDB thread pool. Returns None if unknown.
    fn estimated_chunks(&self) -> Option<u64>;

    /// Fill the output buffer for a specific chunk index (0..num_chunks).
    /// Returns Ok(true) if data was read, Ok(false) if the stream is exhausted.
    fn read_chunk(
        &self, 
        chunk_idx: u64, 
        buffer: &mut ChunkBuffer
    ) -> Result<bool, Box<dyn std::error::Error>>;
}

/// Factory entry point exposed by geozarr_core
pub fn open_dataset(path: &str, asset: Option<&str>) -> Result<Box<dyn GeoDataset>, Box<dyn std::error::Error>>;
```

## DuckDB Lifecycle Mapping

1. **Bind Phase**: The extension calls `geozarr_core::dataset::open_dataset(uri)`, requests the `schema()`, and maps it to DuckDB logical types. It also calls `scan()` with the parsed query constraints and stores the resulting `ChunkStream` in its bind/init state.
2. **Init Phase**: The extension asks the `ChunkStream` for `estimated_chunks()` to set the `max_threads` for DuckDB.
3. **Func Phase**: DuckDB worker threads pull from an atomic counter (`chunk_idx`) and call `read_chunk(chunk_idx, &mut buffer)` on the shared `ChunkStream`. The workers terminate when `read_chunk` returns `Ok(false)` or the `chunk_idx` exceeds `estimated_chunks`.

## Benefits & Trade-offs

- **High Leverage**: DuckDB does not need to understand N-dimensional grids, STAC pagination, or spatial transforms. It just iterates an integer.
- **Locality**: Complex parsing and network chunking strategies are confined to the `ZarrDataset` and `FeatureCollectionDataset` implementations.
- **Trade-off**: The `ChunkBuffer` introduces an unavoidable memory copy when moving data from the `geozarr_core` domain into DuckDB's `DataChunkHandle`, since `ZarrDataset` cannot write directly into DuckDB's C++ memory allocations without breaking the abstraction.
