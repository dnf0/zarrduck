# DuckDB GeoZarr Data Yielding Design

**Date:** 2026-05-15
**Status:** Approved

## 1. Purpose & Context
This design covers the final phase of the read pipeline: the `func` loop execution. It dictates how the extension retrieves chunks of bytes from `zarrs`, iterates over them using the `IterationState` chunk cursor, and strongly types the data before inserting it into DuckDB's `DataChunkHandle`.

## 2. Architecture: Generic Dispatch

Zarr chunks are retrieved as generic byte vectors (`Vec<u8>`), but DuckDB requires data to be inserted into strongly-typed `FlatVector<T>` structures (e.g., `f32`, `i64`).

To avoid duplicating the complex N-dimensional iteration loop for every possible data type, we will use a **Generic Rust Macro** (`dispatch_yield_loop!`).

### 2.1 The Execution Flow
1. **Type Resolution:** The `func` method checks the Zarr array's data type.
2. **Macro Expansion:** It invokes `dispatch_yield_loop!` which expands into a match block.
3. **The Inner Loop:** Inside the macro, the code converts the raw `Vec<u8>` chunk buffer into a strongly-typed Rust slice (e.g. `&[f32]`). It then loops up to `STANDARD_VECTOR_SIZE` (2048) times.
4. **Coordinate Mapping:** For each iteration, the cursor is mapped to N-dimensional indices, which are written into the first $N$ DuckDB vectors as `i64`.
5. **Value Insertion:** The strongly-typed value is read from the slice and written into the final value vector.

## 3. Differentiation vs. `WayScience/duckdb_zarr`

This yielding architecture provides several significant advantages over the existing `duckdb_zarr` extension:

### 3.1 Advanced Codecs and V3 Support (`zarrs` crate)
The `WayScience` extension is written in C++ and uses a custom, minimal Zarr implementation. It is primarily built for Zarr V2 and struggles with complex compression pipelines.
By utilizing the state-of-the-art Rust `zarrs` crate in our fetch loop, our yielding architecture automatically inherits support for **Zarr V3**, **Sharding**, and complex filter/compressor chains (e.g., Blosc, Gzip, Zstd) out-of-the-box.

### 3.2 Semantic Fallback Coordinates
The `WayScience` extension only outputs hardcoded `dim_0`, `dim_1` integers. Because our previous design phase resolved semantic column names (`time`, `lat`, `lon`), the tuples we yield in this loop map directly to physical dimension names. While we are yielding integers in this iteration phase, the foundation is laid to replace these with physical coordinate values (e.g., `45.5`) in a future update, something the generic extension cannot do natively.

### 3.3 Zero-Copy Type Transmutation
By using Rust macros to transmute the raw `Vec<u8>` into strongly-typed slices (`&[f32]`) before the loop, we eliminate runtime branching and intermediate data copies. The inner loop writes directly into the C++ memory space of DuckDB's `FlatVector`, providing near native-C performance.

## 4. Error Handling
- Invalid or unsupported data types will immediately return a `Result::Err`, failing the query gracefully.
- Out-of-bounds cursor anomalies will be caught by the macro logic and yield `0` rows to prevent segmentation faults.

## 5. Spec Self-Review
1. **Placeholder scan:** No TBDs or TODOs found.
2. **Internal consistency:** The macro architecture naturally solves the data typing requirement while preserving the chunk cursor state machine defined in the previous spec.
3. **Scope check:** Focused strictly on the `func` yielding loop and the differentiation explanation requested by the user.
4. **Ambiguity check:** The transmutation strategy (`Vec<u8>` to `&[T]`) is clearly identified as the core mechanism for type safety and performance.
