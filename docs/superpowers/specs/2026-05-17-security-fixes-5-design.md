# Red Team Security Fixes 5 Design

**Date:** 2026-05-17
**Status:** Approved

## 1. Purpose & Context
A 5th Red Team assessment identified four edge cases and vulnerabilities in the `duckdb_geozarr` codebase. These include a potential out-of-bounds array access in the CLI, a memory exhaustion vector due to unbounded active chunks in the CLI, a potential OOM from variable-length strings in the extension, and fragile type inference. This design outlines the remediation strategy for these findings.

## 2. Remediation Architecture

### 2.1 [HIGH] Unbounded Value Range in CLI Coordinates
**Location:** `cli/src/main.rs` (Streaming Loop)
**Fix:** The CLI correctly validates that incoming coordinates are `>= 0`, but does not verify they are within the upper bounds of the array `shape`. If a user query returns an index `val >= shape[i]`, the resulting `flat_idx` will be out of bounds, causing a panic when indexing `b[flat_idx as usize]`.
We will add an explicit bounds check: `if (val as u64) >= shape[i] { return Err(...); }` inside the coordinate parsing loop before performing grid mapping.

### 2.2 [HIGH] Active Chunk Buffer Exhaustion in CLI
**Location:** `cli/src/main.rs` (Eviction Logic)
**Fix:** The eviction logic currently triggers when `active_chunks.len() >= 1000`. If chunks are automatically sized to 10M elements (40MB-80MB each), 1,000 chunks could consume up to 80GB of RAM, leading to an immediate OOM crash on most machines.
Instead of a raw count, we will calculate the byte size of each chunk (`chunk_len * bytes_per_element`), and maintain a `current_memory_usage` counter. The eviction threshold will be set to a hard limit of `1024 * 1024 * 512` (512 MB). The CLI will repeatedly evict the oldest chunks until memory usage falls below this threshold.

### 2.3 [MEDIUM] Variable Length String OOM in Extension
**Location:** `extension/src/table_function.rs` (`bind`)
**Fix:** Zarr string arrays can contain arbitrarily large elements. The current 256MB volume check assumes 1 byte per string element, meaning an array with `[100, 100]` shape passes the check, but could OOM the host if each string is 100MB.
Unfortunately, `zarrs` string retrieval loads the entire chunk into memory. To mitigate this without breaking string support, if `data_type` is `String`, we will limit the `chunk_volume` to `1_000_000` elements (down from the implicit ~256M limit), providing a safer reasonable bound for most geospatial string metadata (like station names or categorical labels).

### 2.4 [LOW] Float vs Double Equivalence Issues
**Location:** `cli/src/main.rs`
**Fix:** The type inference relies on `value_type_str == "DOUBLE"`. DuckDB's `DESCRIBE` can alias types depending on the environment or query structure.
We will broaden the check: `let is_double = value_type_str == "DOUBLE" || value_type_str == "FLOAT8" || value_type_str == "DECIMAL" || value_type_str == "NUMERIC";` to be more robust against DuckDB's type aliases.

## 3. Spec Self-Review
1. **Placeholder scan:** No TBDs.
2. **Internal consistency:** The memory tracking directly addresses the structural flaw of the chunk count eviction logic.
3. **Scope check:** Bounded exclusively to the four identified red team findings.
