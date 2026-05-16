# DuckDB GeoZarr Edge Cases Design

**Date:** 2026-05-15
**Status:** Approved

## 1. Purpose & Context
While the core fetching and yielding pipeline is highly performant, it currently makes several assumptions about the underlying Zarr data that can lead to data corruption or crashes on edge cases. This design addresses four critical data integrity flaws:
1. **Ghost Rows:** Yielding padded elements from chunks that extend past the array boundaries.
2. **Missing Data:** Yielding raw Zarr fill values (e.g., `NaN` or `-9999`) instead of true SQL `NULL`s.
3. **Endianness:** Assuming the Zarr bytes always match the host CPU architecture's endianness.
4. **Varchar Support:** Crashing when encountering variable-length string arrays.

## 2. Architecture: Edge Case Fixes

### 2.1 Partial Edge Chunks (Ghost Rows)
**Problem:** If an array has shape `[105]` and chunk shape `[10]`, the 11th chunk spans indices `100` to `109`. The extension currently yields all 10 elements, resulting in 5 "ghost rows" filled with padding data.
**Solution:**
- Inside the `dispatch_yield_loop!` macro, after calculating the `global_coords`, we will check if any coordinate exceeds `$state.bounds_max`.
- If an element is out of bounds, we simply skip yielding it to DuckDB.
- Because this introduces variable yield rates per batch, the macro will track a `valid_rows` counter and only advance the DuckDB `DataChunk` length by `valid_rows` instead of `batch_size`.

### 2.2 SQL NULL Mapping (Fill Values)
**Problem:** Zarr uses metadata `fill_value` to denote missing data. DuckDB uses a bitmap `ValidityMask`. By ignoring the metadata, missing data pollutes aggregations like `SUM()` and `AVG()`.
**Solution:**
- In `ReadZarrVTab::bind`, extract the `fill_value` from the array metadata and store it in `ReadZarrBindData` as raw bytes.
- In `dispatch_yield_loop!`, compare the raw bytes of the current element against the stored `fill_value` bytes.
- If they match, set the DuckDB `ValidityMask` for that row to `false` (NULL).

### 2.3 Endianness Mismatches
**Problem:** The extension currently uses `from_ne_bytes` (Native Endian), which corrupts floats if the Zarr store uses Big-Endian on a Little-Endian machine.
**Solution:**
- Instead of using `bytemuck` and manual `from_ne_bytes` casting on raw buffers, we will leverage the `zarrs` crate's built-in `retrieve_chunk_elements::<T>` API.
- The `zarrs` crate inherently understands the `bytes` codec and automatically performs endianness conversions during chunk retrieval. We will shift the generic macro to dispatch over `retrieve_chunk_elements` directly.

### 2.4 String/Varchar Support
**Problem:** Zarr string arrays (fixed or variable length) cause the extension to crash because `bytemuck` panics on variable layouts.
**Solution:**
- By moving to `retrieve_chunk_elements`, we can abstract away the byte-level casting.
- We will add a new match arm for `DataType::String` (and potentially `DataType::FixedSizeString`) that extracts the string values and writes them to DuckDB's `FlatVector` using the `duckdb::core::Inserter` or string insertion APIs, avoiding raw memory casting altogether.

## 3. Spec Self-Review
1. **Placeholder scan:** No TBDs.
2. **Internal consistency:** The shift from raw byte manipulation to `retrieve_chunk_elements` naturally solves both the Endianness and String Support issues simultaneously.
3. **Scope check:** This is a large refactor of the `func` loop. It should be executed carefully in stages.
4. **Ambiguity check:** The handling of `valid_rows` for ghost rows is explicit. The mechanism for NULL mapping via byte comparison is clear.
