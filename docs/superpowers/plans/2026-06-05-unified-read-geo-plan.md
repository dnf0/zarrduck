# Unified read_geo Table Function Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Refactor the DuckDB extension to use a unified `read_geo` table function that supports both Zarr arrays and STAC `FeatureCollection` endpoints through a new `GeoDataset` trait boundary.

**Architecture:** We will introduce a `GeoDataset` trait in `geozarr_core` and implement it for `ZarrDataset` (formerly `GeoZarrDataset`) and `FeatureCollectionDataset`. The DuckDB `read_geo` function will use this trait to plan scans and stream chunks without knowing if the data is a single array or millions of virtualized COGs.

**Tech Stack:** Rust, DuckDB extensions, STAC, Zarr, Reqwest.

---

### Task 1: Define `GeoDataset` and `ScanPlan` Traits

**Files:**
- Create: `geozarr_core/src/geo_dataset.rs`
- Modify: `geozarr_core/src/lib.rs`

- [ ] **Step 1: Write the `GeoDataset` trait definition**

```rust
// geozarr_core/src/geo_dataset.rs
use crate::query_planner::QueryConstraints;
use zarrs::array::DataType;
use std::any::Any;

pub trait ScanPlan: Send + Sync {
    fn as_any(&self) -> &dyn Any;
}

pub trait GeoDataset: Send + Sync {
    fn schema(&self) -> Result<Vec<(String, DataType)>, Box<dyn std::error::Error>>;
    fn plan_scan(&self, constraints: &QueryConstraints) -> Result<Box<dyn ScanPlan>, Box<dyn std::error::Error>>;
    fn num_chunks(&self, plan: &dyn ScanPlan) -> u64;
    // We will pass the exact thread index to let the dataset yield its rows
    // For now, minimal method definition. The actual macro integration might require specific return types.
    fn read_chunk(&self, plan: &dyn ScanPlan, chunk_idx: u64, output_buffer: &mut crate::types::ChunkBuffer) -> Result<(), Box<dyn std::error::Error>>;
}
```

- [ ] **Step 2: Add module to lib.rs**

```rust
// geozarr_core/src/lib.rs
// Add:
pub mod geo_dataset;
```

- [ ] **Step 3: Run check**

Run: `cargo check -p geozarr_core`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add geozarr_core/src/geo_dataset.rs geozarr_core/src/lib.rs
git commit -m "feat: define GeoDataset and ScanPlan traits"
```

---

### Task 2: Rename and Prepare `GeoZarrDataset` to `ZarrDataset`

**Files:**
- Modify: `geozarr_core/src/dataset.rs`
- Modify: `extension/src/table_function.rs`

- [ ] **Step 1: Rename the struct**

Rename `GeoZarrDataset` to `ZarrDataset` in `geozarr_core/src/dataset.rs` and any associated test files. Update the implementation block as well.

- [ ] **Step 2: Update references in `extension/src/table_function.rs`**

Change `geozarr_core::dataset::GeoZarrDataset::open(&path)` to `geozarr_core::dataset::ZarrDataset::open(&path)` in `extension/src/table_function.rs`.

- [ ] **Step 3: Run tests to verify**

Run: `cargo test`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add geozarr_core/src/dataset.rs extension/src/table_function.rs
git commit -m "refactor: rename GeoZarrDataset to ZarrDataset"
```

---

### Task 3: Implement `FeatureCollectionDataset` scaffolding

**Files:**
- Create: `geozarr_core/src/feature_collection.rs`
- Modify: `geozarr_core/src/lib.rs`

- [ ] **Step 1: Create the basic struct and open function**

```rust
// geozarr_core/src/feature_collection.rs
pub struct FeatureCollectionDataset {
    pub url: String,
    pub asset_name: String,
}

impl FeatureCollectionDataset {
    pub fn open(url: &str) -> Result<Self, Box<dyn std::error::Error>> {
        // Simple extraction: assume URL might have asset name as fragment or parameter
        // For minimal scaffolding, just store the url.
        Ok(Self {
            url: url.to_string(),
            asset_name: "swir22".to_string(), // hardcoded for scaffolding
        })
    }
}
```

- [ ] **Step 2: Register module in `lib.rs`**

```rust
// geozarr_core/src/lib.rs
// Add:
pub mod feature_collection;
```

- [ ] **Step 3: Add a test to verify instantiation**

```rust
// geozarr_core/src/feature_collection.rs
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_open_feature_collection() {
        let ds = FeatureCollectionDataset::open("https://example.com/stac").unwrap();
        assert_eq!(ds.url, "https://example.com/stac");
    }
}
```

- [ ] **Step 4: Run test**

Run: `cargo test -p geozarr_core test_open_feature_collection`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add geozarr_core/src/feature_collection.rs geozarr_core/src/lib.rs
git commit -m "feat: scaffold FeatureCollectionDataset"
```

---

### Task 4: Refactor DuckDB Extension to use `read_geo`

**Files:**
- Modify: `extension/src/table_function.rs`
- Modify: `extension/src/lib.rs`

- [ ] **Step 1: Rename `read_zarr` to `read_geo`**

In `extension/src/lib.rs`, change the registration:
```rust
    connection.register_table_function::<table_function::ReadGeoVTab>("read_geo")?;
    connection.register_table_function::<table_function::PlanReadGeoVTab>("plan_read_geo")?;
```

- [ ] **Step 2: Rename VTab structs in `table_function.rs`**

Rename `ReadZarrVTab` -> `ReadGeoVTab`
Rename `PlanReadZarrVTab` -> `PlanReadGeoVTab`
Rename `ReadZarrBindData` -> `ReadGeoBindData`
Rename `ReadZarrInitData` -> `ReadGeoInitData`

- [ ] **Step 3: Add dataset factory logic to `bind`**

In `ReadGeoVTab::bind`, wrap the initialization:
```rust
        let path = bind.get_parameter(0).to_string();

        // Very basic dispatch: if path contains "search", it's STAC
        let dataset = if path.contains("/search") || path.contains("items") {
             // For now just error out until full implementation
             return Err("STAC FeatureCollections not fully implemented yet".into());
        } else {
             geozarr_core::dataset::ZarrDataset::open(&path)?
        };
        // Keep the rest of the bind logic exactly as is for ZarrDataset
```

- [ ] **Step 4: Run extension tests**

Run: `cargo test -p extension`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add extension/src/table_function.rs extension/src/lib.rs
git commit -m "refactor: rename read_zarr to read_geo in DuckDB extension"
```

---

### Task 5: Implement STAC Filter Pushdown

**Files:**
- Modify: `geozarr_core/src/feature_collection.rs`

- [ ] **Step 1: Add a test for pushdown logic**

```rust
// geozarr_core/src/feature_collection.rs (in tests module)
#[test]
fn test_stac_filter_pushdown() {
    let mut bounds = std::collections::HashMap::new();
    bounds.insert("lat".to_string(), (Some(40.0), Some(45.0)));
    bounds.insert("lon".to_string(), (Some(-10.0), Some(10.0)));
    let constraints = crate::query_planner::QueryConstraints { bounds, pins: std::collections::HashMap::new() };

    let url = crate::feature_collection::build_stac_url("https://example.com/search", &constraints);
    assert!(url.contains("bbox=-10,40,10,45"));
}
```

- [ ] **Step 2: Implement `build_stac_url`**

```rust
// geozarr_core/src/feature_collection.rs
pub fn build_stac_url(base_url: &str, constraints: &crate::query_planner::QueryConstraints) -> String {
    let mut url = base_url.to_string();

    let lat_bounds = constraints.bounds.get("lat").copied().unwrap_or((None, None));
    let lon_bounds = constraints.bounds.get("lon").copied().unwrap_or((None, None));

    if let (Some(lon_min), Some(lat_min), Some(lon_max), Some(lat_max)) = (lon_bounds.0, lat_bounds.0, lon_bounds.1, lat_bounds.1) {
        let separator = if url.contains('?') { "&" } else { "?" };
        url = format!("{}{separator}bbox={},{},{},{}", url, lon_min, lat_min, lon_max, lat_max);
    }

    url
}
```

- [ ] **Step 3: Verify test passes**

Run: `cargo test -p geozarr_core test_stac_filter_pushdown`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add geozarr_core/src/feature_collection.rs
git commit -m "feat: implement STAC filter pushdown logic"
```

---

### Next Steps After Plan Execution

Once this foundation is in place, the `GeoDataset` trait integration into `table_function.rs` will be completed in a follow-up effort, abstracting the `dispatch_zarr_type!` macro entirely so that `read_geo` operates agnostically over `ZarrDataset` and `FeatureCollectionDataset`.
