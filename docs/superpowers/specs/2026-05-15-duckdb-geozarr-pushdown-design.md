# DuckDB GeoZarr Physical Coordinates & Pushdown Filtering Design

**Date:** 2026-05-15
**Status:** Approved

## 1. Purpose & Context
This design addresses two tightly coupled features:
1. **Physical Coordinates:** Emitting actual coordinate values (e.g., `45.5`) instead of raw integer indices (e.g., `10`) for semantic dimensions like `lat`, `lon`, and `time`.
2. **Pushdown Filtering:** Intercepting DuckDB SQL `WHERE` clauses to spatially prune chunks before they are downloaded from the Zarr store.

Because pushdown filtering against physical values requires knowing the physical coordinates upfront, these two features must be designed together.

## 2. Architecture

### 2.1 Eager Coordinate Loading (`bind`)
Since 1D coordinate arrays defined in `_ARRAY_DIMENSIONS` are typically very small, they will be loaded entirely into memory during the `bind` phase.
- **Lookup:** For each semantic dimension, the extension attempts to open a corresponding 1D array in the Zarr group.
- **Cache:** The data is fetched, cast to `f64` (or `i64`), and stored in `ReadZarrBindData` within a `HashMap<String, Vec<f64>>`.
- **Yielding:** During the `func` loop, the local integer cursor is still calculated as normal. However, before inserting into DuckDB's `FlatVector`, the integer index is used to perform an O(1) lookup in the cached coordinate arrays to yield the physical value.

### 2.2 Range Pushdown Translation (`filter_pushdown`)
DuckDB exposes `WHERE` clause filters via the `filter_pushdown` method.
- **Intercept:** The extension intercepts exact equality (`=`) and range bounds (`<`, `>`, `<=`, `>=`).
- **Translation:** The physical constraints (e.g., `lat >= 40.0 AND lat <= 50.0`) are mapped to integer index bounds by binary-searching the cached 1D coordinate arrays.
- **Bounding Box:** This results in an N-dimensional integer bounding box (e.g., `lat: [10..20], lon: [5..15]`).

### 2.3 Chunk Pruning (`init` & `func`)
The integer bounding box is used to restrict the chunk iteration state machine.
- **Grid Bounds:** The start and end chunk grid coordinates are calculated based on the bounding box.
- **Initialization:** `IterationState.current_chunk_grid` begins at the start of the bounding box rather than `[0, ..., 0]`.
- **Iteration:** The `increment_chunk_grid` function is modified to wrap around at the edge of the bounding box, rather than the edge of the total array. This guarantees that only chunks containing data relevant to the query are fetched from the network.

## 3. Error Handling and Fallbacks
- If a 1D coordinate array is missing from the Zarr store, the extension falls back to yielding integer indices (as established in ADR 0001).
- If a filter involves complex operations (e.g., mathematical functions) that cannot be pushed down, the extension gracefully declines the pushdown for that specific filter, and DuckDB will apply it post-yield.

## 4. Spec Self-Review
1. **Placeholder scan:** No TBDs or TODOs found.
2. **Internal consistency:** The eager loading mechanism directly enables the translation of physical bounds to integer bounds required by chunk pruning.
3. **Scope check:** This scope is large but cohesive. It may be broken down into two distinct subagent execution plans: 1. Coordinate Loading, 2. Pushdown Pruning.
4. **Ambiguity check:** The data type for cached coordinates (`f64`) is clearly specified. The bounding box representation is clear.
