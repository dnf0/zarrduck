# STAC Search API / FeatureCollection Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement safe STAC Search API pushdown by bounding box in DuckDB, enabling fast and constrained remote dataset queries.

**Architecture:** We invert the constraint parsing in `table_function.rs` to occur before the dataset opens. This allows us to validate the bounding box size and pass the constraints down to `resolve_sync_store`, which dynamically appends the `&bbox=...` parameter to the STAC Search API URL before fetching the metadata to build the `VirtualStacTimeStack`.

**Tech Stack:** Rust, DuckDB extension API, zarrs, reqwest

---

### Task 1: Update Core API Signatures to accept QueryConstraints

Update `geozarr_core` to allow passing constraints into the store resolution phase.

**Files:**
- Modify: `geozarr_core/src/dataset.rs`
- Modify: `geozarr_core/src/store.rs`
- Modify: `extension/src/metadata_vtab.rs`

- [ ] **Step 1: Update `resolve_sync_store` signature**
In `geozarr_core/src/store.rs`:
```rust
pub fn resolve_sync_store(
    path: &str,
    constraints: Option<&crate::query_planner::QueryConstraints>,
) -> std::result::Result<ResolvedStore, Box<dyn std::error::Error>> {
```

- [ ] **Step 2: Update `ZarrDataset::open` and `open_with_asset`**
In `geozarr_core/src/dataset.rs`:
```rust
    pub fn open(
        path: &str,
        constraints: Option<&crate::query_planner::QueryConstraints>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        Self::open_with_asset(path, None, constraints)
    }

    pub fn open_with_asset(
        path: &str,
        asset: Option<&str>,
        constraints: Option<&crate::query_planner::QueryConstraints>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let resolved_store = crate::store::resolve_sync_store(path, constraints)?;
```

- [ ] **Step 3: Fix callers to pass `None`**
In `extension/src/metadata_vtab.rs`:
```rust
        let store = geozarr_core::store::resolve_sync_store(&path, None).map_err(|e| e.to_string())?;
```
And in `geozarr_core/src/dataset.rs` (if any tests call `open`), pass `None`.

- [ ] **Step 4: Run `cargo check` to fix remaining test compilation errors**
Run `cargo check` and fix any `resolve_sync_store`, `open`, or `open_with_asset` test calls in `geozarr_core/src/store.rs` or `geozarr_core/src/dataset.rs` to pass `None`.

- [ ] **Step 5: Commit**
```bash
git add geozarr_core/src/dataset.rs geozarr_core/src/store.rs extension/src/metadata_vtab.rs
git commit -m "refactor: add constraints parameter to store resolution"
```

---

### Task 2: Pushdown BBox in STAC HTTP Arm

**Files:**
- Modify: `geozarr_core/src/store.rs`

- [ ] **Step 1: Build STAC URL**
In `geozarr_core/src/store.rs`, inside `resolve_sync_store`, locate the `reqwest::blocking::get(path)` call for the STAC logic (around line 404):
```rust
        if !is_cog && !path.ends_with(".zarr") && !path.ends_with(".zarr/") {
            // Check if it's a STAC Item
            let fetch_url = if let Some(c) = constraints {
                crate::feature_collection::build_stac_url(path, c)
            } else {
                path.to_string()
            };
            if let Ok(resp) = reqwest::blocking::get(&fetch_url) {
```

- [ ] **Step 2: Run tests to verify it builds**
Run `cargo check --workspace` to ensure no errors.

- [ ] **Step 3: Commit**
```bash
git add geozarr_core/src/store.rs
git commit -m "feat: pushdown bbox to stac API via build_stac_url"
```

---

### Task 3: Pre-parse Constraints and Validate BBox in DuckDB Bind

**Files:**
- Modify: `extension/src/table_function.rs`

- [ ] **Step 1: Move constraint parsing to the top of `bind`**
In `extension/src/table_function.rs`, inside `ReadGeoVTab::bind`, move the `bounds` and `pins` parsing to immediately after `let path = bind.get_parameter(0).to_string();`.
Also, we need to extract from `["lat", "lon", "time", "y", "x"]` since the dataset isn't open yet.

```rust
    fn bind(bind: &BindInfo) -> Result<Self::BindData, Box<dyn std::error::Error>> {
        if bind.get_parameter_count() < 1 {
            return Err("read_geo requires at least 1 parameter (path)".into());
        }

        let path = bind.get_parameter(0).to_string();

        let mut bounds = HashMap::new();
        for name in &["lat", "lon", "time", "y", "x"] {
            let min_param_name = format!("{}_min", name);
            let max_param_name = format!("{}_max", name);

            let min_val_opt = bind
                .get_named_parameter(&min_param_name)
                .and_then(|v| v.to_string().parse::<f64>().ok());
            let max_val_opt = bind
                .get_named_parameter(&max_param_name)
                .and_then(|v| v.to_string().parse::<f64>().ok());
            bounds.insert(name.to_string(), (min_val_opt, max_val_opt));
        }

        let mut pins = HashMap::new();
        if let Some(pins_val) = bind.get_named_parameter("pins") {
            let pins_str = pins_val.to_string();
            for pair in pins_str.split(',') {
                let mut parts = pair.splitn(2, '=');
                if let (Some(k), Some(v)) = (parts.next(), parts.next()) {
                    if let Ok(idx) = v.trim().parse::<u64>() {
                        pins.insert(k.trim().to_string(), idx);
                    }
                }
            }
        }

        let constraints = geozarr_core::query_planner::QueryConstraints { bounds, pins };
```

- [ ] **Step 2: Add validation check**
Right after `let constraints = ...`:

```rust
        let is_stac_api = path.starts_with("http") && (path.contains("search") || path.contains("collections"));
        if is_stac_api {
            let lat_bounds = constraints.bounds.get("lat").copied().unwrap_or((None, None));
            let lon_bounds = constraints.bounds.get("lon").copied().unwrap_or((None, None));
            match (lon_bounds.0, lat_bounds.0, lon_bounds.1, lat_bounds.1) {
                (Some(lon_min), Some(lat_min), Some(lon_max), Some(lat_max)) => {
                    let area = (lon_max - lon_min).abs() * (lat_max - lat_min).abs();
                    if area > 1000.0 {
                        return Err("Bounding box area too large for STAC API. Please provide a tighter bbox.".into());
                    }
                }
                _ => return Err("Bounding box (lat_min, lat_max, lon_min, lon_max) is required for STAC APIs.".into()),
            }
        }
```

- [ ] **Step 3: Update `open_with_asset` call and remove old bounds loop**
```rust
        let asset = bind.get_named_parameter("asset").map(|v| v.to_string());
        let dataset = geozarr_core::dataset::ZarrDataset::open_with_asset(&path, asset.as_deref(), Some(&constraints))?;

        let schema = dataset
            .schema()
            .map_err(|e| -> Box<dyn std::error::Error> { e.into() })?;
        for (name, data_type) in schema {
            let type_id = zarr_to_duckdb_logical_type(&data_type)
                .map_err(|e| -> Box<dyn std::error::Error> { e.into() })?;
            bind.add_result_column(&name, type_id.into());
        }

        // Delete the old `let mut bounds = HashMap::new();` loop from here down to `let constraints = ...`

        let (bounds_min, bounds_max) = dataset.compute_bounds(&constraints);
```

- [ ] **Step 4: Run tests**
Run `cargo test --workspace` to ensure all functionality compiles and tests pass. Note that `PlanReadGeoVTab` will also need its `open_with_asset` call updated, but it uses `ReadGeoVTab::bind(bind)` internally, so it automatically gets the fix!

- [ ] **Step 5: Commit**
```bash
git add extension/src/table_function.rs
git commit -m "feat: parse constraints first and validate stac bounding box"
```
