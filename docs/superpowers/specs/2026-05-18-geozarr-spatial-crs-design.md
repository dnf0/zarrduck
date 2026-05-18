# GeoZarr Spatial & CRS Integration Design

**Date:** 2026-05-18

## 1. Context & Purpose
The DuckDB GeoZarr extension must align with the official GeoZarr specification to provide robust, out-of-the-box support for geospatial cloud-native datasets. The GeoZarr specification is modular. This sub-project focuses on the **Spatial Convention** (affine transformations) and the **Geospatial Projection Convention** (Coordinate Reference Systems).

The primary goal is to ensure that LLM Agents and end-users can query multidimensional arrays using real geographic coordinates (e.g., longitude, latitude) seamlessly without needing to manually reconstruct spatial grids using metadata equations.

## 2. Architecture & Data Flow
To integrate GeoZarr spatial metadata into DuckDB's relational (tabular) model, the extension will perform **On-the-Fly Coordinate Calculation**.

### 2.1 Metadata Parsing
- During the initialization of the `read_zarr` table function, the extension will read the `.zattrs` (Zarr v2) or `zarr.json` (Zarr v3) attributes.
- It will parse the GeoZarr standard keys for spatial transforms (typically an affine `scale` and `translation` array mapped to specific dimensions like `x` and `y`).

### 2.2 Vectorized Execution
- During the `DataChunk` yield loop, the extension generates coordinates for each dimension.
- For non-spatial or explicitly mapped 1D physical arrays (like `time`), the existing coordinate mapping logic remains.
- For spatial dimensions governed by an affine transform, the extension calculates the coordinate dynamically:
  `coordinate = translation + (grid_index * scale)`
- This projected coordinate is emitted as a `DOUBLE` column, replacing the raw `x_idx`/`y_idx` integers with actual `lon`/`lat` values.

## 3. Dataset-Level Metadata Extraction
Because properties like `Coordinate Reference System (CRS)` apply to the entire array rather than row-by-row, returning them on every pixel in `read_zarr` would be massively inefficient.

### 3.1 New Table Function: `read_zarr_metadata`
We will introduce a secondary table function: `read_zarr_metadata('store_path')`.
- It will return a single row describing the global properties of the array.
- Columns will include: `array_shape`, `chunk_shape`, `data_type`, and `crs`.
- The `crs` column will return the PROJJSON or WKT string parsed from the GeoZarr metadata.
- This allows tools (and the `duckdb_spatial` extension) to retrieve the CRS to define geometries when needed.

## 4. Error Handling & Constraints
- If a dataset lacks GeoZarr affine metadata, the extension falls back to yielding raw grid indices or explicit 1D coordinate arrays.
- Corrupted or malformed affine metadata arrays (e.g., mismatching the number of dimensions) will result in a descriptive DuckDB error during query preparation.

## 5. Testing Strategy
- Unit tests in Rust will verify the parsing of the affine transform arrays and CRS strings.
- Integration tests will generate a mock GeoZarr dataset with specific `scale` and `translation` offsets, verifying that a SQL query like `SELECT lon FROM read_zarr(...) LIMIT 1` returns the correctly transformed double value.