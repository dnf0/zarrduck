# ADR 001: Unified `read_geo` Table Function with Lazy STAC Virtualization

## Context

The DuckDB extension currently provides `read_zarr`, which was designed to query a single Zarr group or an individual Cloud Optimized GeoTIFF (COG). However, spatial workflows often involve querying a STAC API, which returns a `FeatureCollection` containing multiple distinct STAC Items (potentially thousands or millions of COGs).

Trying to map an entire STAC catalog or `FeatureCollection` into a single, perfectly aligned virtual Zarr array at the storage layer is problematic:
1. **Performance**: Fetching the 16KB TIFF header for every item just to plan the query grid would require millions of HTTP requests.
2. **Alignment**: Overlapping bounds and varying grids across COGs make rigid multidimensional array stitching complex and brittle.
3. **Overloading**: Forcing `read_zarr` to become a STAC paginator and multi-dataset orchestrator violates the Single Responsibility Principle.

## Decision

We will introduce a new unified entry point for the DuckDB extension: `read_geo(url)`. 

Inside the extension, a "Decision Layer" will parse the URL and dispatch to a specific implementation of a new Rust `Dataset` trait:

1. **`ZarrDataset`**: Used if the URL points to a standard Zarr Group/Array or a single COG.
2. **`FeatureCollectionDataset`**: Used if the URL is a STAC Search API or Collection returning a `FeatureCollection`.

To ensure `FeatureCollectionDataset` can performantly query millions of items, it will implement three core optimizations:

1. **Filter Pushdown**: DuckDB spatial and temporal constraints (`WHERE lat > 40 AND time > '2023-01-01'`) will be intercepted during the bind phase and translated into STAC API query parameters (`&bbox=...&datetime=...`). This pushes the heavy filtering to the remote STAC server.
2. **Zero-Header Lazy Planning**: We will use the STAC Item JSON metadata extensions (`bbox`, `properties.datetime`, `proj:shape`, `proj:transform`) to construct the query execution plan and bounds. The actual `.tif` headers will *not* be fetched during planning. They will only be fetched lazily by the worker threads right before reading specific byte-ranges.
3. **Massively Parallel Row Streaming**: The dataset will expose all chunks across all matching COGs as a flat iteration space. DuckDB will assign threads to fetch chunks from different independent COGs concurrently. The worker threads will parse the data and yield tabular `(lat, lon, time, value)` rows. DuckDB's relational engine will naturally handle the union, grouping, and overlapping of these points.

## Consequences

- **Standalone Power**: The DuckDB extension becomes a highly capable standalone tool. Users can query STAC APIs directly via SQL without requiring the `eider` CLI as an orchestrator.
- **Extreme Performance**: Query planning remains instantaneous because we bypass upfront `.tif` header reads, relying solely on STAC JSON metadata.
- **Relational Simplicity**: We completely avoid the complexity of stitching multiple COGs into a single Zarr array. By streaming discrete points, we let DuckDB do what it does best: filter, group, and aggregate rows.
- **Refactoring Requirement**: We will need to define the `Dataset` trait boundary clearly so both `ZarrDataset` and `FeatureCollectionDataset` can seamlessly yield chunks to the DuckDB execution workers.
