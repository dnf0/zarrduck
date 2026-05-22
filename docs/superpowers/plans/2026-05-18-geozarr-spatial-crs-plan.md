# GeoZarr Spatial & CRS Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add parsing for GeoZarr affine transformations and CRS metadata, seamlessly projecting grid coordinates to spatial coordinates during table scans and exposing dataset-level metadata via a new function.

**Architecture:** We will introduce a new `metadata.rs` module that holds parsing logic. The `read_zarr` `bind` function will attach parsed affine transforms to spatial dimensions, forcing them to `DOUBLE`. The iteration loop will apply `translation + (index * scale)` to calculate coordinates on the fly. Finally, a new `ReadZarrMetadataVTab` will expose global attributes like `crs` and array shapes.

**Tech Stack:** Rust, `duckdb-rs`, `zarrs`, `serde_json`

---

### Task 1: Create `metadata.rs` and the `SpatialTransform` model

**Files:**
- Create: `extension/src/metadata.rs`
- Modify: `extension/src/lib.rs`

- [ ] **Step 1: Write the failing test**

```rust
// In extension/src/metadata.rs
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_parse_spatial_metadata() {
        let attrs = json!({
            "geozarr": {
                "crs": "EPSG:4326",
                "spatial_transform": {
                    "scale": [0.1, 0.1],
                    "translation": [-180.0, 90.0]
                }
            }
        });

        let meta = parse_geozarr_metadata(&attrs).unwrap();
        assert_eq!(meta.crs, Some("EPSG:4326".to_string()));
        let transform = meta.transform.unwrap();
        assert_eq!(transform.scale, vec![0.1, 0.1]);
        assert_eq!(transform.translation, vec![-180.0, 90.0]);
    }
}
```

- [ ] **Step 2: Add mod to lib.rs**

```rust
// In extension/src/lib.rs
pub mod metadata;
pub mod table_function;
pub use table_function::ReadZarrVTab;
// ...
```

- [ ] **Step 3: Run test to verify it fails**

Run: `cargo test -p zarrduck metadata`
Expected: FAIL (cannot find function `parse_geozarr_metadata`)

- [ ] **Step 4: Write minimal implementation**

```rust
// In extension/src/metadata.rs
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpatialTransform {
    pub scale: Vec<f64>,
    pub translation: Vec<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeoZarrMetadata {
    pub crs: Option<String>,
    pub transform: Option<SpatialTransform>,
}

pub fn parse_geozarr_metadata(attrs: &Value) -> Option<GeoZarrMetadata> {
    let geozarr_val = attrs.get("geozarr")?;

    let crs = geozarr_val.get("crs").and_then(|v| v.as_str()).map(|s| s.to_string());

    let transform = geozarr_val.get("spatial_transform").and_then(|t| {
        let scale = t.get("scale")?.as_array()?
            .iter()
            .filter_map(|v| v.as_f64())
            .collect::<Vec<f64>>();

        let translation = t.get("translation")?.as_array()?
            .iter()
            .filter_map(|v| v.as_f64())
            .collect::<Vec<f64>>();

        Some(SpatialTransform { scale, translation })
    });

    Some(GeoZarrMetadata { crs, transform })
}
```

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test -p zarrduck metadata`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add extension/src/lib.rs extension/src/metadata.rs
git commit -m "feat: add spatial transform parsing for geozarr"
```

---

### Task 2: Apply Spatial Transforms in `read_zarr` Coordinate Loop

**Files:**
- Modify: `extension/src/table_function.rs`

- [ ] **Step 1: Write the failing test**

```rust
// At the bottom of extension/src/table_function.rs
#[test]
fn test_spatial_transform_coordinate_generation() {
    // This is a direct test of the `func` iteration coordinate mapping logic.
    // However, since `func` requires DuckDB Context, we test it through an e2e test in test_extension.rs later.
    // For now, we will add a unit test for a new helper function `apply_transform`
    let transform = crate::metadata::SpatialTransform {
        scale: vec![0.1, -0.1],
        translation: vec![-180.0, 90.0]
    };

    assert_eq!(apply_transform(&transform, 0, 5), -180.0 + (5.0 * 0.1));
    assert_eq!(apply_transform(&transform, 1, 10), 90.0 + (10.0 * -0.1));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p zarrduck table_function::tests::test_spatial_transform_coordinate_generation`
Expected: FAIL (cannot find function `apply_transform`)

- [ ] **Step 3: Write minimal implementation**

Add `apply_transform` to `extension/src/table_function.rs`:

```rust
// In extension/src/table_function.rs
pub fn apply_transform(transform: &crate::metadata::SpatialTransform, dim_index: usize, grid_index: u64) -> f64 {
    let scale = transform.scale.get(dim_index).copied().unwrap_or(1.0);
    let translation = transform.translation.get(dim_index).copied().unwrap_or(0.0);
    translation + (grid_index as f64 * scale)
}
```

- [ ] **Step 4: Update `ReadZarrBindData` and `bind`**

Modify `ReadZarrBindData` in `extension/src/table_function.rs` to store the transform:
```rust
pub struct ReadZarrBindData {
    pub store_path: String,
    pub shape: Vec<u64>,
    pub chunk_shape: Vec<u64>,
    pub data_type: zarrs::array::DataType,
    pub bounds_min: Vec<u64>,
    pub bounds_max: Vec<u64>,
    pub dim_names: Vec<String>,
    pub 1d_coords: HashMap<String, Vec<f64>>,
    pub spatial_transform: Option<crate::metadata::SpatialTransform>, // ADD THIS
}
```

Inside `ReadZarrVTab::bind`, parse it from the array metadata:
```rust
        let metadata = array.metadata();

        let mut spatial_transform = None;
        if let zarrs::array::ArrayMetadata::V2(meta) = metadata {
            if let Some(geozarr_meta) = crate::metadata::parse_geozarr_metadata(&meta.attributes) {
                spatial_transform = geozarr_meta.transform;
            }
        } else if let zarrs::array::ArrayMetadata::V3(meta) = metadata {
            if let Some(geozarr_meta) = crate::metadata::parse_geozarr_metadata(&meta.attributes) {
                spatial_transform = geozarr_meta.transform;
            }
        }
```
Store `spatial_transform: spatial_transform.clone()` in the returned `ReadZarrBindData`.

When creating `bind.add_result_column`, force `DOUBLE` if a spatial transform exists:
```rust
        // Add coordinate columns (DuckDB Double if physical or transformed, Bigint if fallback)
        for (i, name) in dim_names.iter().enumerate() {
            let has_transform = spatial_transform.as_ref().map_or(false, |t| i < t.scale.len());
            if coords.contains_key(name) || has_transform {
                bind.add_result_column(name, LogicalTypeId::Double.into());
            } else {
                bind.add_result_column(name, LogicalTypeId::Bigint.into());
            }
        }
```

- [ ] **Step 5: Update `func` to use the transform**

Inside `ReadZarrVTab::func` in `extension/src/table_function.rs` where we yield `coord_val`, apply the transform:
```rust
                            // If explicit physical 1D array coordinate exists
                            if let Some(coord_array) = bind_data.1d_coords.get(dim_name) {
                                let val = coord_array.get(global_index as usize).copied().unwrap_or(0.0);
                                output.write::<f64>(idx, row_idx, val);
                            }
                            // If spatial transform exists for this dimension
                            else if let Some(ref transform) = bind_data.spatial_transform {
                                if dim_idx < transform.scale.len() {
                                    let val = apply_transform(transform, dim_idx, global_index);
                                    output.write::<f64>(idx, row_idx, val);
                                } else {
                                    output.write::<i64>(idx, row_idx, global_index as i64);
                                }
                            }
                            // Fallback to integer grid index
                            else {
                                output.write::<i64>(idx, row_idx, global_index as i64);
                            }
```

- [ ] **Step 6: Run test to verify it passes**

Run: `cargo test -p zarrduck`
Expected: PASS

- [ ] **Step 7: Commit**

```bash
git add extension/src/table_function.rs
git commit -m "feat: apply spatial affine transforms during read_zarr coordinate mapping"
```

---

### Task 3: Implement `read_zarr_metadata` Table Function

**Files:**
- Create: `extension/src/metadata_vtab.rs`
- Modify: `extension/src/lib.rs`

- [ ] **Step 1: Add to lib.rs**

```rust
// In extension/src/lib.rs
pub mod metadata_vtab;
pub use metadata_vtab::ReadZarrMetadataVTab;

#[cfg(feature = "loadable-extension")]
#[duckdb::duckdb_entrypoint_c_api]
fn init(conn: duckdb::Connection) -> duckdb::Result<()> {
    conn.register_table_function::<ReadZarrVTab>("read_zarr")?;
    conn.register_table_function::<ReadZarrMetadataVTab>("read_zarr_metadata")?; // ADD THIS
    Ok(())
}
```

- [ ] **Step 2: Implement VTab in `metadata_vtab.rs`**

```rust
// In extension/src/metadata_vtab.rs
use duckdb::core::{DataChunkHandle, LogicalTypeHandle, LogicalTypeId};
use duckdb::vtab::{BindInfo, InitInfo, VTab};
use duckdb::Result;
use std::sync::Arc;
use zarrs::storage::store::FilesystemStore;

pub struct MetadataBindData {
    pub shape: String,
    pub chunk_shape: String,
    pub data_type: String,
    pub crs: String,
}

pub struct MetadataInitData {
    pub done: bool,
}

pub struct ReadZarrMetadataVTab;

impl VTab for ReadZarrMetadataVTab {
    type InitData = MetadataInitData;
    type BindData = MetadataBindData;

    fn parameters() -> Option<Vec<LogicalTypeHandle>> {
        Some(vec![LogicalTypeHandle::from(LogicalTypeId::Varchar)])
    }

    fn bind(bind: &BindInfo) -> Result<Self::BindData, Box<dyn std::error::Error>> {
        let path = bind.get_parameter(0).to_string();
        let store = Arc::new(FilesystemStore::new(path).map_err(|e| e.to_string())?);
        let array = zarrs::array::Array::open(store, "/").map_err(|e| e.to_string())?;

        let shape = format!("{:?}", array.shape());
        let chunk_shape = format!("{:?}", array.chunk_grid().chunk_shape(&vec![0; array.shape().len()], &array.shape().to_vec()).unwrap_or(None));
        let data_type = format!("{:?}", array.data_type());

        let mut crs = "UNKNOWN".to_string();
        let metadata = array.metadata();
        if let zarrs::array::ArrayMetadata::V2(meta) = metadata {
            if let Some(geozarr) = crate::metadata::parse_geozarr_metadata(&meta.attributes) {
                if let Some(c) = geozarr.crs { crs = c; }
            }
        } else if let zarrs::array::ArrayMetadata::V3(meta) = metadata {
            if let Some(geozarr) = crate::metadata::parse_geozarr_metadata(&meta.attributes) {
                if let Some(c) = geozarr.crs { crs = c; }
            }
        }

        bind.add_result_column("array_shape", LogicalTypeId::Varchar.into());
        bind.add_result_column("chunk_shape", LogicalTypeId::Varchar.into());
        bind.add_result_column("data_type", LogicalTypeId::Varchar.into());
        bind.add_result_column("crs", LogicalTypeId::Varchar.into());

        Ok(MetadataBindData { shape, chunk_shape, data_type, crs })
    }

    fn init(_init: &InitInfo) -> Result<Self::InitData, Box<dyn std::error::Error>> {
        Ok(MetadataInitData { done: false })
    }

    fn func(func: &duckdb::vtab::FunctionInfo, output: &mut DataChunkHandle) -> Result<(), Box<dyn std::error::Error>> {
        let init_data = func.get_init_data::<MetadataInitData>();
        if init_data.done {
            output.set_len(0);
            return Ok(());
        }

        let bind_data = func.get_bind_data::<MetadataBindData>();

        output.write_string(0, 0, &bind_data.shape);
        output.write_string(1, 0, &bind_data.chunk_shape);
        output.write_string(2, 0, &bind_data.data_type);
        output.write_string(3, 0, &bind_data.crs);
        output.set_len(1);

        let mut init_data_mut = func.get_init_data_mut::<MetadataInitData>();
        init_data_mut.done = true;

        Ok(())
    }
}
```

- [ ] **Step 3: Run compilation check**

Run: `cargo build -p zarrduck`
Expected: SUCCESS

- [ ] **Step 4: Commit**

```bash
git add extension/src/metadata_vtab.rs extension/src/lib.rs
git commit -m "feat: add read_zarr_metadata table function for dataset-level attributes"
```

---

### Task 4: Add E2E Tests for Spatial and Metadata

**Files:**
- Modify: `extension/tests/test_extension.rs`

- [ ] **Step 1: Write E2E Test**

Add the following to `test_extension.rs`:

```rust
// In extension/tests/test_extension.rs
#[test]
fn test_geozarr_spatial_metadata() -> duckdb::Result<()> {
    let conn = duckdb::Connection::open_in_memory()?;
    conn.register_table_function::<geozarr::ReadZarrVTab>("read_zarr")?;
    conn.register_table_function::<geozarr::metadata_vtab::ReadZarrMetadataVTab>("read_zarr_metadata")?;

    let temp_dir = tempfile::tempdir_in(std::env::current_dir().unwrap()).unwrap();
    let store_path = temp_dir.path().join("test_spatial.zarr");

    use std::sync::Arc;
    use zarrs::array::{ArrayBuilder, DataType, FillValue};
    use zarrs::storage::store::FilesystemStore;

    let store = Arc::new(FilesystemStore::new(&store_path).unwrap());
    let mut builder = ArrayBuilder::new(
        vec![2, 2],
        DataType::Float32,
        vec![2, 2].try_into().unwrap(),
        FillValue::from(0.0f32),
    );

    let mut attributes = serde_json::Map::new();
    attributes.insert("_ARRAY_DIMENSIONS".to_string(), serde_json::json!(["y", "x"]));
    attributes.insert("geozarr".to_string(), serde_json::json!({
        "crs": "EPSG:3857",
        "spatial_transform": {
            "scale": [10.0, -10.0],
            "translation": [-180.0, 90.0]
        }
    }));
    builder.attributes(attributes);

    let array = builder.build(Arc::clone(&store), "/").unwrap();
    array.store_metadata().unwrap();
    array.store_chunk_elements(&[0, 0], &[1.0f32, 2.0, 3.0, 4.0]).unwrap();

    // 1. Test Metadata
    let query_meta = format!("SELECT crs FROM read_zarr_metadata('{}')", store_path.display());
    let mut stmt_meta = conn.prepare(&query_meta)?;
    let crs: String = stmt_meta.query_row([], |row| row.get(0))?;
    assert_eq!(crs, "EPSG:3857");

    // 2. Test Spatial Coordinates Projection
    // y_idx=0, x_idx=0 -> y: 90 + (0 * -10) = 90.0 | x: -180 + (0 * 10) = -180.0
    // y_idx=0, x_idx=1 -> y: 90 + (0 * -10) = 90.0 | x: -180 + (1 * 10) = -170.0
    // y_idx=1, x_idx=0 -> y: 90 + (1 * -10) = 80.0 | x: -180 + (0 * 10) = -180.0

    let query_data = format!("SELECT y, x, value FROM read_zarr('{}') ORDER BY y DESC, x ASC", store_path.display());
    let mut stmt_data = conn.prepare(&query_data)?;
    let mut rows = stmt_data.query([])?;

    let row1 = rows.next()?.unwrap();
    assert_eq!(row1.get::<_, f64>(0)?, 90.0); // y
    assert_eq!(row1.get::<_, f64>(1)?, -180.0); // x
    assert_eq!(row1.get::<_, f32>(2)?, 1.0); // value

    let row2 = rows.next()?.unwrap();
    assert_eq!(row2.get::<_, f64>(0)?, 90.0); // y
    assert_eq!(row2.get::<_, f64>(1)?, -170.0); // x
    assert_eq!(row2.get::<_, f32>(2)?, 2.0); // value

    Ok(())
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test -p zarrduck test_geozarr_spatial_metadata`
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add extension/tests/test_extension.rs
git commit -m "test: add E2E tests for geozarr spatial coordinate mapping and metadata vtab"
```
