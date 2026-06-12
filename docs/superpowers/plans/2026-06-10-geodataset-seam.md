# GeoDataset Seam Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Refactor the codebase to introduce a `GeoDataset` interface that abstracts spatial dataset operations, decoupling DuckDB extension from the specific `ZarrDataset` implementation.

**Architecture:** Create an In-process Adapter pattern where `geozarr_core` defines `GeoDataset` and `ChunkStream` traits. The `ZarrDataset` implements these, and the DuckDB extension (`ReadGeoVTab`) uses a factory function `open_dataset` to acquire a dataset and stream chunks.

**Tech Stack:** Rust, DuckDB C-FFI, zarrs

---

### Task 1: Update Interfaces in `geo_dataset.rs`

**Files:**
- Modify: `geozarr_core/src/geo_dataset.rs`
- Modify: `geozarr_core/src/lib.rs` (if necessary to export the module properly)

- [ ] **Step 1: Write the updated interface definitions**

Update `geozarr_core/src/geo_dataset.rs` to contain the new traits defined in the spec:

```rust
use crate::query_planner::QueryConstraints;
use crate::types::ChunkBuffer;
use zarrs::array::DataType;

pub trait GeoDataset: Send + Sync {
    fn schema(&self) -> Result<Vec<(String, DataType)>, Box<dyn std::error::Error>>;

    fn scan(
        &self,
        constraints: &QueryConstraints
    ) -> Result<Box<dyn ChunkStream>, Box<dyn std::error::Error>>;
}

pub trait ChunkStream: Send + Sync {
    fn estimated_chunks(&self) -> Option<u64>;

    fn read_chunk(
        &self,
        chunk_idx: u64,
        buffer: &mut ChunkBuffer
    ) -> Result<bool, Box<dyn std::error::Error>>;
}
```

- [ ] **Step 2: Ensure it compiles**

Run: `cargo check -p geozarr_core`
Expected: PASS (or fail due to existing implementations, which we fix in the next task)

- [ ] **Step 3: Commit**

```bash
git add geozarr_core/src/geo_dataset.rs
git commit -m "refactor: define GeoDataset and ChunkStream traits"
```

### Task 2: Implement GeoDataset for ZarrDataset

**Files:**
- Modify: `geozarr_core/src/dataset.rs`

- [ ] **Step 1: Create ZarrChunkStream and implement GeoDataset**

In `geozarr_core/src/dataset.rs`, implement `GeoDataset` for `ZarrDataset`, and create `ZarrChunkStream` struct. Also add the `open_dataset` factory function.

```rust
use crate::geo_dataset::{GeoDataset, ChunkStream};
use crate::query_planner::QueryConstraints;
use crate::types::ChunkBuffer;
use crate::scanner::GridIterator;
use zarrs::array::DataType;
use std::sync::Arc;

pub struct ZarrChunkStream {
    dataset: Arc<ZarrDataset>,
    grid_iterator: GridIterator,
    num_chunks: u64,
}

impl ChunkStream for ZarrChunkStream {
    fn estimated_chunks(&self) -> Option<u64> {
        Some(self.num_chunks)
    }

    fn read_chunk(
        &self,
        chunk_idx: u64,
        buffer: &mut ChunkBuffer
    ) -> Result<bool, Box<dyn std::error::Error>> {
        if chunk_idx >= self.num_chunks {
            return Ok(false);
        }

        let grid_pos = self.grid_iterator.get_chunk_pos(chunk_idx);
        // Note: use the existing chunk reading logic here
        // to populate the buffer using grid_pos.
        crate::scanner::read_chunk_into_buffer(&self.dataset, &grid_pos, buffer)
            .map(|_| true)
            .map_err(|e| e.into())
    }
}

impl GeoDataset for Arc<ZarrDataset> {
    fn schema(&self) -> Result<Vec<(String, DataType)>, Box<dyn std::error::Error>> {
        let mut cols = Vec::new();
        for (i, name) in self.dim_names.iter().enumerate() {
            cols.push((name.clone(), DataType::Float64)); // coords are f64
        }
        cols.push(("value".to_string(), self.data_type.clone()));
        Ok(cols)
    }

    fn scan(
        &self,
        constraints: &QueryConstraints
    ) -> Result<Box<dyn ChunkStream>, Box<dyn std::error::Error>> {
        let (bounds_min, bounds_max) = self.compute_bounds(constraints);

        let rank = self.shape.len();
        let mut chunk_bounds_min = vec![0; rank];
        let mut chunk_bounds_max = vec![0; rank];
        for i in 0..rank {
            chunk_bounds_min[i] = bounds_min[i] / self.chunk_shape[i];
            chunk_bounds_max[i] = bounds_max[i] / self.chunk_shape[i];
        }

        let num_chunks: u64 = (0..rank)
            .map(|i| chunk_bounds_max[i].saturating_sub(chunk_bounds_min[i]) + 1)
            .product();

        let grid_iterator = GridIterator::new(
            &bounds_min,
            &bounds_max,
            &self.shape,
            &self.chunk_shape,
        );

        Ok(Box::new(ZarrChunkStream {
            dataset: Arc::clone(self),
            grid_iterator,
            num_chunks,
        }))
    }
}

pub fn open_dataset(path: &str, asset: Option<&str>) -> Result<Box<dyn GeoDataset>, Box<dyn std::error::Error>> {
    // Currently only Zarr is supported here, but this is the seam.
    let zarr = ZarrDataset::open_with_asset(path, asset)?;
    Ok(Box::new(Arc::new(zarr)))
}
```
*(Note: You will need to make sure `crate::scanner::read_chunk_into_buffer` or the equivalent existing logic is adapted or exposed to work with `ZarrChunkStream`)*

- [ ] **Step 2: Run tests**

Run: `cargo test -p geozarr_core`
Expected: Tests should pass (or may require minor updates to scanner tests)

- [ ] **Step 3: Commit**

```bash
git add geozarr_core/src/dataset.rs
git commit -m "feat: implement GeoDataset for ZarrDataset and add factory"
```

### Task 3: Refactor DuckDB Extension to use GeoDataset

**Files:**
- Modify: `extension/src/table_function.rs`

- [ ] **Step 1: Refactor `bind`, `init`, and `func` to use the interface**

Update `ReadGeoBindData` and `ReadGeoInitData` to work with the interfaces instead of `ZarrDataset`.

```rust
use geozarr_core::geo_dataset::{GeoDataset, ChunkStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

pub struct ReadGeoBindData {
    pub stream: Arc<dyn ChunkStream>,
}

pub struct ReadGeoInitData {
    pub next_chunk: AtomicU64,
}

// In bind:
let dataset = geozarr_core::dataset::open_dataset(&path, asset.as_deref())?;
let schema = dataset.schema()?;
for (name, data_type) in schema {
    let type_id = zarr_to_duckdb_logical_type(&data_type)?;
    bind.add_result_column(&name, type_id.into());
}

let constraints = /* build constraints from bind parameters... */;
let stream = dataset.scan(&constraints)?;

Ok(ReadGeoBindData { stream: Arc::from(stream) })

// In init:
let bind_data = unsafe { &*_init.get_bind_data::<ReadGeoBindData>() };
if let Some(chunks) = bind_data.stream.estimated_chunks() {
    _init.set_max_threads(chunks);
}

Ok(ReadGeoInitData { next_chunk: AtomicU64::new(0) })

// In func:
let chunk_idx = init_data.next_chunk.fetch_add(1, Ordering::Relaxed);
let mut buffer = geozarr_core::types::ChunkBuffer::new(); // Initialize properly
let has_data = bind_data.stream.read_chunk(chunk_idx, &mut buffer)?;

if !has_data {
    output.set_len(0);
    return Ok(());
}

// convert buffer to DuckDB output chunk...
```

- [ ] **Step 2: Fix compilation and tests**

Run: `cargo test`
Expected: Entire workspace should compile and tests should pass. You may need to massage the `ChunkBuffer` allocation and writing logic.

- [ ] **Step 3: Commit**

```bash
git add extension/src/table_function.rs
git commit -m "refactor: extension uses GeoDataset interface"
```
