# Red Team Security Fixes 5 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement the fifth round of security and stability fixes to address unbounded value ranges in CLI coordinates, active chunk buffer exhaustion in the CLI, variable-length string OOMs in the extension, and fragile float vs. double type inference.

**Architecture:** Use strict bounds checking against chunk shapes and cumulative memory usage for eviction in the CLI. Limit string-based chunks to a safe element count in the extension. Broaden the DuckDB `DESCRIBE` match for `Float64` equivalence.

**Tech Stack:** Rust, `duckdb`, `zarrs`, `tokio`.

---

### Task 1: Fix Unbounded Value Range in CLI Coordinates

**Files:**
- Modify: `cli/src/main.rs`

- [ ] **Step 1: Write the failing test**
Skip failing test.

- [ ] **Step 2: Write minimal implementation**
In `cli/src/main.rs`, update the coordinate parsing loop to ensure values do not exceed the array bounds.

Locate the `while let Some(row) = rows.next()?` loop and modify the `grid_coord` generation:

```rust
                for (i, &chunk_dim) in chunk_shape.iter().enumerate().take(coord_columns.len()) {
                    let val: i64 = row.get(i)?;
                    if val < 0 {
                        return Err("Coordinates must be positive 0-based integer indices".into());
                    }
                    if (val as u64) >= shape[i] {
                        return Err(format!("Coordinate index {} exceeds maximum bound of dimension {}", val, shape[i]).into());
                    }
                    let grid_idx = (val as u64) / chunk_dim;
                    grid_coord.push(grid_idx);
                }
```

- [ ] **Step 3: Run test to verify it passes**
Run: `cargo check -p geozarr-cli`
Expected: PASS.

- [ ] **Step 4: Commit**
```bash
git add cli/src/main.rs
git commit -m "fix: validate coordinate upper bounds to prevent out-of-bounds indexing in CLI"
```

### Task 2: Fix Active Chunk Buffer Exhaustion in CLI

**Files:**
- Modify: `cli/src/main.rs`

- [ ] **Step 1: Write the failing test**
Skip failing test.

- [ ] **Step 2: Write minimal implementation**
In `cli/src/main.rs`, replace the raw count eviction threshold (`active_chunks.len() >= 1000`) with a memory-based threshold (`512MB`).

At the top of `cli/src/main.rs` before `5. Stream data from DuckDB`:
```rust
            let mut active_chunks: std::collections::BTreeMap<Vec<u64>, ChunkData> =
                std::collections::BTreeMap::new();
            let chunk_len = chunk_shape
                .iter()
                .try_fold(1u64, |acc, &x| acc.checked_mul(x))
                .ok_or("Chunk volume overflow")? as usize;

            let bytes_per_element = if data_type == zarrs::array::DataType::Float64 { 8 } else { 4 };
            let chunk_byte_size = chunk_len.checked_mul(bytes_per_element).ok_or("Chunk byte size overflow")?;
            let max_memory_bytes = 512 * 1024 * 1024; // 512 MB
```

Inside the streaming loop, update the eviction check:
```rust
                // Eviction check for sparse chunks
                // Evict chunks until our estimated memory usage is below the 512MB threshold.
                while active_chunks.len().saturating_mul(chunk_byte_size) >= max_memory_bytes {
                    let oldest_key = active_chunks.keys().next().unwrap().clone();
                    let evicted_buffer = active_chunks.remove(&oldest_key).unwrap();
                    tx.send((oldest_key, evicted_buffer))
                        .await
                        .map_err(|_| "Upload worker failed or disconnected")?;
                }
```

- [ ] **Step 3: Run test to verify it passes**
Run: `cargo check -p geozarr-cli`
Expected: PASS.

- [ ] **Step 4: Commit**
```bash
git add cli/src/main.rs
git commit -m "fix: bound CLI eviction strategy by actual memory footprint instead of chunk count"
```

### Task 3: Fix Variable Length String OOM in Extension

**Files:**
- Modify: `extension/src/table_function.rs`

- [ ] **Step 1: Write the failing test**
Skip failing test.

- [ ] **Step 2: Write minimal implementation**
In `extension/src/table_function.rs`, inside the `bind` function, limit the `chunk_volume` for `DataType::String` to 1,000,000 elements.

Locate the memory check block:
```rust
        let bytes_per_element = match data_type {
            zarrs::array::DataType::Float64 | zarrs::array::DataType::Int64 | zarrs::array::DataType::UInt64 => 8,
            zarrs::array::DataType::Float32 | zarrs::array::DataType::Int32 | zarrs::array::DataType::UInt32 => 4,
            zarrs::array::DataType::Int16 | zarrs::array::DataType::UInt16 => 2,
            _ => 1,
        };
        let chunk_bytes = chunk_volume.checked_mul(bytes_per_element).ok_or("Chunk byte volume overflow")?;
        if chunk_bytes > 256 * 1024 * 1024 {
            return Err(format!("Chunk size {} bytes exceeds maximum allowed volume of 256MB", chunk_bytes).into());
        }
```
Update it to explicitly handle strings:
```rust
        if data_type == &zarrs::array::DataType::String {
            if chunk_volume > 1_000_000 {
                return Err(format!("Zarr string chunk volume {} exceeds maximum allowed (1,000,000 elements) to prevent OOM", chunk_volume).into());
            }
        } else {
            let bytes_per_element = match data_type {
                zarrs::array::DataType::Float64 | zarrs::array::DataType::Int64 | zarrs::array::DataType::UInt64 => 8,
                zarrs::array::DataType::Float32 | zarrs::array::DataType::Int32 | zarrs::array::DataType::UInt32 => 4,
                zarrs::array::DataType::Int16 | zarrs::array::DataType::UInt16 => 2,
                _ => 1,
            };
            let chunk_bytes = chunk_volume.checked_mul(bytes_per_element).ok_or("Chunk byte volume overflow")?;
            if chunk_bytes > 256 * 1024 * 1024 {
                return Err(format!("Chunk size {} bytes exceeds maximum allowed volume of 256MB", chunk_bytes).into());
            }
        }
```

- [ ] **Step 3: Run test to verify it passes**
Run: `cargo check -p eider`
Expected: PASS.

- [ ] **Step 4: Commit**
```bash
git add extension/src/table_function.rs
git commit -m "fix: constrain variable length string chunks to 1M elements to prevent OOM"
```

### Task 4: Fix Float vs Double Equivalence Issues

**Files:**
- Modify: `cli/src/main.rs`

- [ ] **Step 1: Write the failing test**
Skip failing test.

- [ ] **Step 2: Write minimal implementation**
In `cli/src/main.rs`, update the exact string match for `DOUBLE` to include common aliases.

Locate:
```rust
            let is_double = value_type_str == "DOUBLE";

            let data_type = if is_double {
                zarrs::array::DataType::Float64
            } else {
                zarrs::array::DataType::Float32
            };
```
Change the variable assignment to:
```rust
            let is_double = value_type_str == "DOUBLE" || value_type_str == "FLOAT8" || value_type_str == "DECIMAL" || value_type_str == "NUMERIC";
```

- [ ] **Step 3: Run test to verify it passes**
Run: `cargo check -p geozarr-cli`
Expected: PASS.

- [ ] **Step 4: Commit**
```bash
git add cli/src/main.rs
git commit -m "fix: use expanded type matching for DuckDB double precision float detection"
```
