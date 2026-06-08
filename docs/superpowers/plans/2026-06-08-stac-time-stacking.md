# STAC ItemCollection Time-Stacking Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `read_geo` on a STAC ItemCollection/FeatureCollection stacks the selected COG asset across its Items into a 3D `[time, lat, lon]` array, `time` = epoch seconds from each Item's `properties.datetime`.

**Architecture:** A net-new `VirtualStacTimeStack` Zarr-group store holds per-asset time-sorted `Vec<VirtualCogStore>` and synthesizes a 3D `.zarray` per asset (derived from the child's 2D `.zarray`) plus group-level `/time`,`/lat`,`/lon` coordinate arrays, so `CoordinateResolver` + `compute_bounds` pushdown work unchanged. `store.rs` parses the collection, sorts by datetime, validates collection-wide grid uniformity, and builds the store. `open_with_asset` and the extension are unchanged.

**Tech Stack:** Rust (`geozarr_core`), `zarrs`, `chrono` (RFC3339), `serde_json`.

---

## Conventions & prerequisites

Repo root `/Users/danielfisher/repos/zarrduck`, branch `feat/stac-time-stacking` (off `main`). TDD: failing test → run → implement → pass → commit. Conventional Commits; `--no-gpg-sign`; trailer `Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>`. Pre-commit runs fmt/clippy/whitespace. Never `git add -A`. Clippy on touched files only (`-p geozarr_core --lib --tests`, NOT `--all-targets` — pre-existing `scanner.rs:165` lint on `main`).

### Established source facts (don't re-derive)
- **`CoordinateResolver::resolve(path, store, shape, dim_names)`** (`geozarr_core/src/coordinate_resolver.rs`): for each dim name it does `Array::open(store, "/{name}")`, requires a 1-D array whose length == `shape[dim]`, and reads it via `retrieve_array_subset_elements` for `Float64`/`Float32`/`Int64`/`Int32`. So synthesized coordinate arrays must be real zarrs-readable arrays at root keys `/time`,`/lat`,`/lon`. Filling `coords` for every dim makes `compute_bounds` use binary-search pushdown for all of them (the `time_min`/`time_max`/`lat`/`lon` params bind by dim name).
- **`VirtualCogStore`** (`geozarr_core/src/virtual_store.rs`): `ReadableStorageTraits` + `ListableStorageTraits`. `new(operator, filename, meta) -> Result<Self, String>`. `get()` serves `.zmetadata`/`.zarray`/`.zattrs` and a chunk key `"y.x"` → the tile bytes (Deflate-decoded). Its synthesized 2D `.zarray` is `{shape:[H,W],chunks:[tileH,tileW],dtype,fill_value,...}` and `.zattrs` has `_ARRAY_DIMENSIONS` + `geozarr` (crs + spatial_transform). `CogMetadata` exposes `image_width/length`, `tile_width/length`, `zarr_dtype()`, `spatial_transform() -> Option<SpatialTransform{scale,translation}>`, `crs() -> Option<String>`, `dim_names() -> Vec<String>`.
- **3D chunk byte-equivalence:** a 3D chunk `[1, tileH, tileW]` has the identical byte layout to the child's 2D `[tileH, tileW]` tile. So `get("{asset}/{t}.{y}.{x}")` → `children[asset][t].get("{y}.{x}")` returns exactly the bytes zarrs wants — no byte surgery.
- **`store.rs` local branch:** canonicalizes + `GEOZARR_ALLOW_PATH` sandbox; `build_local_cog_child(&Path) -> Result<VirtualCogStore, String>` builds a local COG store; the `FeatureCollection` arm currently returns the "not yet supported" error (local ~line 519; HTTP ~line 250). `ResolvedStore { store, is_remote, stac_assets: Option<Vec<String>> }`. The single-Item arm builds `VirtualStacStore` and sets `stac_assets`.
- **`apply_transform(&SpatialTransform, dim_index, grid_index) -> f64`** (`coordinates.rs`) = `translation[dim] + grid_index*scale[dim]`.

## File structure
- Create: `geozarr_core/src/datetime.rs` (RFC3339→epoch), `geozarr_core/src/virtual_stac_time_stack.rs` (store) — register both in `geozarr_core/src/lib.rs`.
- Modify: `geozarr_core/Cargo.toml` (chrono), `geozarr_core/src/store.rs` (FeatureCollection branch local+HTTP), `geozarr_core/src/lib.rs` (mod decls).
- Create: `geozarr_core/tests/fixtures/stac_itemcollection.json` (replace placeholder), `geozarr_core/tests/stac_timestack_e2e.rs`, `geozarr_core/tests/stac_timestack_store.rs`.
- Modify: `docs/docs/engineering/cog_virtualization.mdx`, `docs/docs/usage/sql_read_geo.md`.

---

## Task 1: datetime → epoch seconds (`chrono`)

**Files:** Create `geozarr_core/src/datetime.rs`; Modify `geozarr_core/Cargo.toml`, `geozarr_core/src/lib.rs`.

- [ ] **Step 1: Add chrono**

In `geozarr_core/Cargo.toml` `[dependencies]`:
```toml
chrono = { version = "0.4", default-features = false, features = ["alloc"] }
```
Run `cargo build -p geozarr_core` → builds.

- [ ] **Step 2: Write the failing test**

Create `geozarr_core/src/datetime.rs`:
```rust
//! RFC3339 datetime → epoch-seconds for STAC `properties.datetime`.

/// Parse an RFC3339 timestamp into seconds since the Unix epoch.
pub fn rfc3339_to_epoch_seconds(s: &str) -> Result<f64, String> {
    chrono::DateTime::parse_from_rfc3339(s.trim())
        .map(|dt| dt.timestamp() as f64)
        .map_err(|e| format!("invalid RFC3339 datetime {s:?}: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_utc_z() {
        assert_eq!(rfc3339_to_epoch_seconds("2026-01-01T00:00:00Z").unwrap(), 1767225600.0);
    }
    #[test]
    fn parses_offset() {
        // 2026-01-01T01:00:00+01:00 == 2026-01-01T00:00:00Z
        assert_eq!(
            rfc3339_to_epoch_seconds("2026-01-01T01:00:00+01:00").unwrap(),
            1767225600.0
        );
    }
    #[test]
    fn rejects_garbage() {
        assert!(rfc3339_to_epoch_seconds("not-a-date").is_err());
        assert!(rfc3339_to_epoch_seconds("").is_err());
    }
}
```
Add `pub mod datetime;` to `geozarr_core/src/lib.rs`.

- [ ] **Step 3: Run (verify the epoch constants)**

Run: `cargo test -p geozarr_core datetime::`
Expected: PASS. (If the exact epoch constant differs, correct the literal to the value chrono returns — the test asserts real parsing, the constant is just the known value for 2026-01-01T00:00:00Z.)

- [ ] **Step 4: Commit**
```bash
git add geozarr_core/Cargo.toml geozarr_core/Cargo.lock geozarr_core/src/datetime.rs geozarr_core/src/lib.rs
git commit --no-gpg-sign -m "feat(stac): add RFC3339 datetime to epoch-seconds helper

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 2: `VirtualStacTimeStack` store

**Files:** Create `geozarr_core/src/virtual_stac_time_stack.rs`; Modify `geozarr_core/src/lib.rs`.

- [ ] **Step 1: Write the store**

Create `geozarr_core/src/virtual_stac_time_stack.rs`:
```rust
//! A virtual Zarr group stacking one COG asset across N STAC Items along time.
//! Per asset: a 3D `[N, H, W]` array whose `[1, tileH, tileW]` chunks route to
//! each item's COG tile. Group-level `/time`, `/lat`, `/lon` coordinate arrays
//! make the stack open fully coordinate-resolved.
use crate::virtual_store::VirtualCogStore;
use bytes::Bytes;
use std::collections::HashMap;
use zarrs::storage::{ListableStorageTraits, ReadableStorageTraits, StoreKey, StorePrefix};

pub struct VirtualStacTimeStack {
    /// asset name -> time-sorted per-item COG stores (len N).
    assets: HashMap<String, Vec<VirtualCogStore>>,
    /// asset name -> synthesized 3D `.zarray` / `.zattrs` bytes.
    asset_zarray: HashMap<String, Bytes>,
    asset_zattrs: HashMap<String, Bytes>,
    /// coordinate name -> (`.zarray` bytes, chunk `0` bytes). Keys: time/lat/lon (or y/x).
    coords: HashMap<String, (Bytes, Bytes)>,
    spatial_dims: [String; 2],
    zgroup_bytes: Bytes,
    zmetadata_bytes: Bytes,
}

fn coord_zarray(len: usize) -> String {
    format!(
        r#"{{"zarr_format":2,"shape":[{len}],"chunks":[{len}],"dtype":"<f8","compressor":null,"fill_value":0.0,"filters":null,"order":"C"}}"#
    )
}
fn coord_bytes(vals: &[f64]) -> Bytes {
    let mut b = Vec::with_capacity(vals.len() * 8);
    for v in vals {
        b.extend_from_slice(&v.to_le_bytes());
    }
    Bytes::from(b)
}

impl VirtualStacTimeStack {
    /// `assets`: per-asset time-sorted item stores (each len == times.len()).
    /// `times`: epoch seconds (sorted). `lat`/`lon`: spatial coordinate values
    /// (len H / W). `spatial_dims`: ["lat","lon"] or ["y","x"].
    pub fn new(
        assets: HashMap<String, Vec<VirtualCogStore>>,
        times: Vec<f64>,
        lat: Vec<f64>,
        lon: Vec<f64>,
        spatial_dims: [String; 2],
    ) -> Result<Self, String> {
        let n = times.len();
        let h = lat.len();
        let w = lon.len();
        if assets.is_empty() {
            return Err("time-stack has no assets".into());
        }

        let mut asset_zarray = HashMap::new();
        let mut asset_zattrs = HashMap::new();
        let mut meta_map = serde_json::Map::new();
        meta_map.insert(".zgroup".into(), serde_json::json!({"zarr_format": 2}));

        for (name, items) in &assets {
            if items.len() != n {
                return Err(format!("asset {name}: {} items, expected {n}", items.len()));
            }
            // Derive the 3D .zarray from the child's 2D .zarray (carries dtype/fill).
            let child0 = &items[0];
            let z2 = child0
                .get(&StoreKey::new(".zarray").unwrap())
                .map_err(|e| e.to_string())?
                .ok_or_else(|| format!("asset {name}: child has no .zarray"))?;
            let mut z: serde_json::Value =
                serde_json::from_slice(&z2).map_err(|e| e.to_string())?;
            let chunks_2d = z["chunks"].clone();
            let (tile_h, tile_w) = (
                chunks_2d[0].as_u64().unwrap_or(h as u64),
                chunks_2d[1].as_u64().unwrap_or(w as u64),
            );
            z["shape"] = serde_json::json!([n, h, w]);
            z["chunks"] = serde_json::json!([1, tile_h, tile_w]);
            let z_str = z.to_string();

            let zattrs = serde_json::json!({
                "_ARRAY_DIMENSIONS": ["time", spatial_dims[0], spatial_dims[1]],
            })
            .to_string();

            meta_map.insert(format!("{name}/.zarray"), serde_json::from_str::<serde_json::Value>(&z_str).unwrap());
            meta_map.insert(format!("{name}/.zattrs"), serde_json::from_str::<serde_json::Value>(&zattrs).unwrap());
            asset_zarray.insert(name.clone(), Bytes::from(z_str));
            asset_zattrs.insert(name.clone(), Bytes::from(zattrs));
        }

        let mut coords = HashMap::new();
        for (cname, vals) in [
            ("time".to_string(), &times),
            (spatial_dims[0].clone(), &lat),
            (spatial_dims[1].clone(), &lon),
        ] {
            let za = coord_zarray(vals.len());
            meta_map.insert(format!("{cname}/.zarray"), serde_json::from_str::<serde_json::Value>(&za).unwrap());
            coords.insert(cname, (Bytes::from(za), coord_bytes(vals)));
        }

        let zmetadata = serde_json::json!({
            "metadata": meta_map,
            "zarr_consolidated_format": 1
        })
        .to_string();

        Ok(Self {
            assets,
            asset_zarray,
            asset_zattrs,
            coords,
            spatial_dims,
            zgroup_bytes: Bytes::from(r#"{"zarr_format": 2}"#),
            zmetadata_bytes: Bytes::from(zmetadata),
        })
    }

    /// All asset names (sorted) — for `ResolvedStore.stac_assets`.
    pub fn asset_names(&self) -> Vec<String> {
        let mut v: Vec<String> = self.assets.keys().cloned().collect();
        v.sort();
        v
    }
}

impl ReadableStorageTraits for VirtualStacTimeStack {
    fn get(&self, key: &StoreKey) -> Result<Option<Bytes>, zarrs::storage::StorageError> {
        let k = key.as_str();
        if k == ".zgroup" {
            return Ok(Some(self.zgroup_bytes.clone()));
        }
        if k == ".zmetadata" {
            return Ok(Some(self.zmetadata_bytes.clone()));
        }
        // Coordinate arrays: "{name}/.zarray" or "{name}/0".
        if let Some((name, sub)) = k.split_once('/') {
            if let Some((za, data)) = self.coords.get(name) {
                if sub == ".zarray" {
                    return Ok(Some(za.clone()));
                }
                if sub == "0" {
                    return Ok(Some(data.clone()));
                }
            }
            // Asset metadata.
            if sub == ".zarray" {
                if let Some(b) = self.asset_zarray.get(name) {
                    return Ok(Some(b.clone()));
                }
            }
            if sub == ".zattrs" {
                if let Some(b) = self.asset_zattrs.get(name) {
                    return Ok(Some(b.clone()));
                }
            }
            // Asset chunk "t.y.x" -> children[name][t].get("y.x").
            if let Some(items) = self.assets.get(name) {
                let mut parts = sub.splitn(2, '.');
                if let (Some(t_str), Some(yx)) = (parts.next(), parts.next()) {
                    if let Ok(t) = t_str.parse::<usize>() {
                        if let Some(child) = items.get(t) {
                            if let Ok(child_key) = StoreKey::new(yx) {
                                return child.get(&child_key);
                            }
                        }
                    }
                }
            }
        }
        Ok(None)
    }

    fn get_partial_values_key(
        &self,
        key: &StoreKey,
        byte_ranges: &[zarrs::byte_range::ByteRange],
    ) -> Result<Option<Vec<Bytes>>, zarrs::storage::StorageError> {
        if let Some(bytes) = self.get(key)? {
            let mut out = Vec::new();
            for r in byte_ranges {
                let start = match r {
                    zarrs::byte_range::ByteRange::FromStart(o, _) => *o,
                    _ => 0,
                };
                let end = match r {
                    zarrs::byte_range::ByteRange::FromStart(o, Some(l)) => *o + *l,
                    _ => bytes.len() as u64,
                };
                out.push(bytes.slice(start as usize..end as usize));
            }
            Ok(Some(out))
        } else {
            Ok(None)
        }
    }

    fn size_key(&self, key: &StoreKey) -> Result<Option<u64>, zarrs::storage::StorageError> {
        Ok(self.get(key)?.map(|b| b.len() as u64))
    }
}

impl ListableStorageTraits for VirtualStacTimeStack {
    fn list(&self) -> Result<zarrs::storage::StoreKeys, zarrs::storage::StorageError> {
        let mut keys = vec![
            StoreKey::new(".zgroup").unwrap(),
            StoreKey::new(".zmetadata").unwrap(),
        ];
        for name in self.coords.keys() {
            keys.push(StoreKey::new(&format!("{name}/.zarray")).unwrap());
        }
        for name in self.assets.keys() {
            keys.push(StoreKey::new(&format!("{name}/.zarray")).unwrap());
            keys.push(StoreKey::new(&format!("{name}/.zattrs")).unwrap());
        }
        Ok(keys)
    }
    fn list_prefix(&self, prefix: &StorePrefix) -> Result<zarrs::storage::StoreKeys, zarrs::storage::StorageError> {
        let p = prefix.as_str();
        Ok(self.list()?.into_iter().filter(|k| k.as_str().starts_with(p)).collect())
    }
    fn list_dir(&self, _prefix: &StorePrefix) -> Result<zarrs::storage::StoreKeysPrefixes, zarrs::storage::StorageError> {
        zarrs::storage::store::MemoryStore::new().list_dir(_prefix)
    }
    fn size_prefix(&self, _prefix: &StorePrefix) -> Result<u64, zarrs::storage::StorageError> {
        Ok(0)
    }
}
```
Add `pub mod virtual_stac_time_stack;` to `lib.rs`. (`list_dir` mirrors the single-Item store's MemoryStore fallback; match whatever that file used if the API differs.)

- [ ] **Step 2: Write the unit test**

Append to `virtual_stac_time_stack.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::cog::CogMetadata;

    fn child() -> VirtualCogStore {
        let meta = CogMetadata {
            image_width: 4, image_length: 2, tile_width: 4, tile_length: 2,
            tile_offsets: vec![0], tile_byte_counts: vec![16],
            is_little_endian: true, bits_per_sample: 16, sample_format: 2,
            samples_per_pixel: 1, compression: 1, predictor: 1, ..Default::default()
        };
        let op = opendal::Operator::new(opendal::services::Memory::default()).unwrap().finish();
        VirtualCogStore::new(op, "".into(), meta).unwrap()
    }

    fn stack() -> VirtualStacTimeStack {
        let mut assets = HashMap::new();
        assets.insert("band".to_string(), vec![child(), child()]);
        VirtualStacTimeStack::new(
            assets,
            vec![1000.0, 2000.0],
            vec![90.0, 88.0],            // H = 2
            vec![-180.0, -178.0, -176.0, -174.0], // W = 4
            ["lat".into(), "lon".into()],
        )
        .unwrap()
    }

    #[test]
    fn asset_zarray_is_3d() {
        let s = stack();
        let z = String::from_utf8(s.get(&StoreKey::new("band/.zarray").unwrap()).unwrap().unwrap().to_vec()).unwrap();
        assert!(z.contains("\"shape\":[2,2,4]"), "{z}");
        assert!(z.contains("\"chunks\":[1,2,4]"), "{z}");
        assert!(z.contains("\"<i2\""), "{z}");
    }

    #[test]
    fn time_coord_array_roundtrips() {
        let s = stack();
        let za = String::from_utf8(s.get(&StoreKey::new("time/.zarray").unwrap()).unwrap().unwrap().to_vec()).unwrap();
        assert!(za.contains("\"shape\":[2]") && za.contains("\"<f8\""));
        let data = s.get(&StoreKey::new("time/0").unwrap()).unwrap().unwrap();
        let v0 = f64::from_le_bytes(data[0..8].try_into().unwrap());
        let v1 = f64::from_le_bytes(data[8..16].try_into().unwrap());
        assert_eq!((v0, v1), (1000.0, 2000.0));
    }

    #[test]
    fn chunk_routes_to_item() {
        let s = stack();
        // both items share the same synthetic tile; assert routing returns Some bytes
        assert!(s.get(&StoreKey::new("band/0.0.0").unwrap()).unwrap().is_some());
        assert!(s.get(&StoreKey::new("band/1.0.0").unwrap()).unwrap().is_some());
        // out-of-range time index -> None
        assert!(s.get(&StoreKey::new("band/9.0.0").unwrap()).unwrap().is_none());
    }

    #[test]
    fn zattrs_dims_are_time_lat_lon() {
        let s = stack();
        let za = String::from_utf8(s.get(&StoreKey::new("band/.zattrs").unwrap()).unwrap().unwrap().to_vec()).unwrap();
        assert!(za.contains("_ARRAY_DIMENSIONS") && za.contains("time") && za.contains("lat") && za.contains("lon"));
    }
}
```
> Note: the synthetic child tiles here have no real backing bytes in the Memory store, so `band/0.0.0` may return `Ok(None)` if the child's tile read fails — the assertion checks routing reaches the child, not tile contents. If the child returns `None` for a missing tile, weaken `chunk_routes_to_item` to assert the call does not error and the out-of-range index is `None` (routing correctness is what matters; real tile bytes are covered by the e2e against fixtures).

- [ ] **Step 3: Run + iterate**

Run: `cargo test -p geozarr_core virtual_stac_time_stack::`. Expected: PASS (adjust the chunk-routing assertion per the note if needed). Then `cargo clippy -p geozarr_core --lib -- -D warnings` + `cargo fmt --check` clean.

- [ ] **Step 4: Commit**
```bash
git add geozarr_core/src/virtual_stac_time_stack.rs geozarr_core/src/lib.rs
git commit --no-gpg-sign -m "feat(stac): add VirtualStacTimeStack 3D group store with synthesized coords

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 3: Collection-wide grid-uniformity validation

**Files:** Modify `geozarr_core/src/virtual_stac_time_stack.rs` (add a validation helper + test).

- [ ] **Step 1: Write the failing test**

Append to the `tests` module:
```rust
use crate::cog::CogMetadata as M;

fn m(w: u32, h: u32, epsg: Option<u32>) -> M {
    M { image_width: w, image_length: h, tile_width: w, tile_length: h,
        is_little_endian: true, bits_per_sample: 16, sample_format: 2,
        samples_per_pixel: 1, compression: 1, predictor: 1, epsg, ..Default::default() }
}

#[test]
fn uniformity_passes_for_identical_and_fails_on_mismatch() {
    let a = m(4, 2, Some(4326));
    let b = m(4, 2, Some(4326));
    assert!(super::validate_grid_uniform(&[&a, &b]).is_ok());

    let diff_shape = m(8, 2, Some(4326));
    let e = super::validate_grid_uniform(&[&a, &diff_shape]).unwrap_err();
    assert!(e.contains("shape") || e.contains("1"), "{e}");

    let diff_crs = m(4, 2, Some(32633));
    assert!(super::validate_grid_uniform(&[&a, &diff_crs]).is_err());
}
```

- [ ] **Step 2: Run (fail — helper missing)**; Expected: FAIL.

- [ ] **Step 3: Implement**

Add to `virtual_stac_time_stack.rs` (module level):
```rust
use crate::cog::CogMetadata;

/// Verify every COG shares item 0's grid: shape, tile shape, affine, and CRS.
/// (dtype is validated per asset by the caller; this checks the shared grid.)
pub fn validate_grid_uniform(metas: &[&CogMetadata]) -> Result<(), String> {
    let Some(first) = metas.first() else { return Ok(()); };
    let f_tf = first.spatial_transform();
    for (i, m) in metas.iter().enumerate().skip(1) {
        if (m.image_width, m.image_length) != (first.image_width, first.image_length) {
            return Err(format!(
                "item {i}: shape {}x{} != {}x{}",
                m.image_length, m.image_width, first.image_length, first.image_width
            ));
        }
        if (m.tile_width, m.tile_length) != (first.tile_width, first.tile_length) {
            return Err(format!("item {i}: tile shape differs"));
        }
        if m.epsg != first.epsg {
            return Err(format!("item {i}: CRS {:?} != {:?}", m.crs(), first.crs()));
        }
        let tf = m.spatial_transform();
        if tf.as_ref().map(|t| (&t.scale, &t.translation))
            != f_tf.as_ref().map(|t| (&t.scale, &t.translation))
        {
            return Err(format!("item {i}: affine transform differs"));
        }
    }
    Ok(())
}
```

- [ ] **Step 4: Run to pass**; Expected: PASS. clippy/fmt clean.
- [ ] **Step 5: Commit**
```bash
git add geozarr_core/src/virtual_stac_time_stack.rs
git commit --no-gpg-sign -m "feat(stac): validate collection-wide grid uniformity for time-stacks

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 4: Fixtures (ItemCollection + error fixtures)

**Files:** Create `geozarr_core/tests/fixtures/stac_itemcollection.json` (replace placeholder), `geozarr_core/tests/fixtures/stac_itemcollection_nodatetime.json`, `geozarr_core/tests/fixtures/stac_itemcollection_heterogeneous.json`.

- [ ] **Step 1: Write the main fixture** (2 items, asset `band` → the two existing COG fixtures, distinct datetimes)

`geozarr_core/tests/fixtures/stac_itemcollection.json`:
```json
{
  "stac_version": "1.0.0",
  "type": "FeatureCollection",
  "features": [
    { "stac_version": "1.0.0", "type": "Feature", "id": "t0",
      "properties": { "datetime": "2026-01-01T00:00:00Z" },
      "geometry": null, "bbox": [-180, 82, -172, 90],
      "assets": { "band": { "href": "./cog_int16_uncompressed.tif",
        "type": "image/tiff; application=geotiff; profile=cloud-optimized" } }, "links": [] },
    { "stac_version": "1.0.0", "type": "Feature", "id": "t1",
      "properties": { "datetime": "2026-02-01T00:00:00Z" },
      "geometry": null, "bbox": [-180, 82, -172, 90],
      "assets": { "band": { "href": "./cog_int16_deflate.tif",
        "type": "image/tiff; application=geotiff; profile=cloud-optimized" } }, "links": [] }
  ]
}
```

- [ ] **Step 2: Error fixtures**

`stac_itemcollection_nodatetime.json` — same as above but the second feature's `properties` is `{}` (no `datetime`).
`stac_itemcollection_heterogeneous.json` — second feature's `band.href` points at a differently-shaped COG. Since only the two 4×2 fixtures exist, instead make the second item reference a COG that does not exist OR (preferred) reuse `cog_int16_uncompressed.tif` for both items but in Task 5's test assert the heterogeneous case differently. **Simplest reliable heterogeneous fixture:** omit it here and cover the mismatch via a `virtual_stac_time_stack::validate_grid_uniform` unit test (Task 3 already does) — drop this fixture. (Only create the no-datetime and empty cases as JSON fixtures.)
`stac_itemcollection_empty.json`: `{ "stac_version":"1.0.0", "type":"FeatureCollection", "features":[] }`.

- [ ] **Step 3: Commit**
```bash
git add geozarr_core/tests/fixtures/stac_itemcollection.json geozarr_core/tests/fixtures/stac_itemcollection_nodatetime.json geozarr_core/tests/fixtures/stac_itemcollection_empty.json
git commit --no-gpg-sign -m "test(stac): add ItemCollection fixtures (2-item stack + error cases)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 5: `store.rs` FeatureCollection branch (local + HTTP)

**Files:** Modify `geozarr_core/src/store.rs`; Create `geozarr_core/tests/stac_timestack_store.rs`.

- [ ] **Step 1: Write the failing integration test**

Create `geozarr_core/tests/stac_timestack_store.rs`:
```rust
use geozarr_core::store::resolve_sync_store;
use zarrs::storage::{ReadableStorageTraits, StoreKey};

fn fixt(name: &str) -> String {
    format!("{}/tests/fixtures/{}", env!("CARGO_MANIFEST_DIR"), name)
}

#[test]
fn itemcollection_resolves_to_timestack_group() {
    std::env::set_var("GEOZARR_ALLOW_PATH", env!("CARGO_MANIFEST_DIR"));
    let r = resolve_sync_store(&fixt("stac_itemcollection.json")).expect("should resolve");
    assert_eq!(r.stac_assets.as_deref(), Some(&["band".to_string()][..]));
    let zmeta = String::from_utf8(
        r.store.get(&StoreKey::new(".zmetadata").unwrap()).unwrap().unwrap().to_vec()
    ).unwrap();
    assert!(zmeta.contains("band/.zarray"));
    assert!(zmeta.contains("time/.zarray"));
}

#[test]
fn empty_collection_errors() {
    std::env::set_var("GEOZARR_ALLOW_PATH", env!("CARGO_MANIFEST_DIR"));
    let e = resolve_sync_store(&fixt("stac_itemcollection_empty.json")).err().expect("error");
    assert!(format!("{e}").to_lowercase().contains("empty") || format!("{e}").contains("no features"));
}

#[test]
fn missing_datetime_errors() {
    std::env::set_var("GEOZARR_ALLOW_PATH", env!("CARGO_MANIFEST_DIR"));
    let e = resolve_sync_store(&fixt("stac_itemcollection_nodatetime.json")).err().expect("error");
    assert!(format!("{e}").to_lowercase().contains("datetime"));
}
```

- [ ] **Step 2: Run (fail)**; Expected: FAIL (still the "not yet supported" error).

- [ ] **Step 3: Implement the local FeatureCollection branch**

In `store.rs`, replace the local `Some("FeatureCollection") => { return Err(...) }` arm with logic that:
1. reads `features` (array); errors `"STAC FeatureCollection has no features (empty)"` if missing/empty;
2. for each feature: read `properties.datetime` (string) — error `"STAC item <id>: missing properties.datetime"` if absent/non-string; parse via `crate::datetime::rfc3339_to_epoch_seconds`;
3. collect `(epoch, feature)` and **sort ascending by epoch**;
4. determine the COG-asset names from the FIRST feature (media-type/href filter, as the single-Item arm does); error if none;
5. for each asset, for each (sorted) feature: resolve the asset href relative to the JSON dir (reuse the single-Item arm's `base.join(href)` + `is_absolute`), `build_local_cog_child(&abs)?`, collect into `assets: HashMap<String, Vec<VirtualCogStore>>`; error if a feature lacks an expected asset;
6. validate grid uniformity: collect each built child's `&CogMetadata` (needs a `pub fn meta(&self) -> &CogMetadata` getter on `VirtualCogStore` — add it) across ALL assets×items, call `virtual_stac_time_stack::validate_grid_uniform`;
7. derive spatial coords from the first child's `CogMetadata`: `dim_names = meta.dim_names()`; if `meta.spatial_transform()` is `Some(t)`, `lat[i] = apply_transform(&t, 0, i)` for `i in 0..H`, `lon[j] = apply_transform(&t, 1, j)`; else `lat = (0..H).map(f64)`, `lon = (0..W).map(f64)` and dim_names `["y","x"]`. `times` = the sorted epochs;
8. `let store = Arc::new(VirtualStacTimeStack::new(assets, times, lat, lon, [dim0, dim1])?);`
9. `return Ok(ResolvedStore { store, is_remote: false, stac_assets: Some(store_asset_names) })` — compute `stac_assets` from the assets map keys (sorted) before moving into the store, or call `store.asset_names()`.

Add to `VirtualCogStore` (`virtual_store.rs`): `pub fn meta(&self) -> &crate::cog::CogMetadata { &self.meta }`.

Mirror the same logic in the **HTTP** branch's `FeatureCollection` handling (it currently errors right after JSON parse): build children via the existing concurrent HTTP/S3 header-fetch (one `VirtualCogStore` per asset per item), same datetime/sort/validate/coords, `is_remote: true`. (The HTTP path has no offline test; mirror the local logic structurally.)

- [ ] **Step 4: Run to pass**

Run: `cargo test -p geozarr_core --test stac_timestack_store`. Iterate until green. Then `cargo test -p geozarr_core` (whole crate; live STAC test may need network — note separately), clippy `--lib --tests`, fmt.

- [ ] **Step 5: Commit**
```bash
git add geozarr_core/src/store.rs geozarr_core/src/virtual_store.rs geozarr_core/tests/stac_timestack_store.rs
git commit --no-gpg-sign -m "feat(stac): resolve ItemCollection into a time-stack (local + HTTP)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 6: End-to-end time-stack read

**Files:** Create `geozarr_core/tests/stac_timestack_e2e.rs`.

- [ ] **Step 1: Write the e2e tests**

```rust
use geozarr_core::dataset::ZarrDataset;
use geozarr_core::query_planner::QueryConstraints;
use std::collections::HashMap;

fn fixt(name: &str) -> String {
    format!("{}/tests/fixtures/{}", env!("CARGO_MANIFEST_DIR"), name)
}
fn allow() { std::env::set_var("GEOZARR_ALLOW_PATH", env!("CARGO_MANIFEST_DIR")); }

#[test]
fn timestack_opens_as_3d_with_time_coords() {
    allow();
    let ds = ZarrDataset::open_with_asset(&fixt("stac_itemcollection.json"), Some("band")).unwrap();
    assert_eq!(ds.dim_names, vec!["time".to_string(), "lat".to_string(), "lon".to_string()]);
    assert_eq!(ds.shape, vec![2, 2, 4]);
    let time = ds.coords.get("time").expect("time coords present");
    // 2026-01-01 and 2026-02-01 in epoch seconds, ascending
    assert_eq!(time.len(), 2);
    assert!(time[0] < time[1]);
    assert_eq!(time[0], 1767225600.0);
    // value dtype is Int16
    let (vname, vtype) = ds.schema().unwrap().pop().unwrap();
    assert_eq!(vname, "value");
    assert_eq!(format!("{vtype:?}"), format!("{:?}", zarrs::array::DataType::Int16));
}

#[test]
fn timestack_time_pushdown_prunes_to_one_slice() {
    allow();
    let ds = ZarrDataset::open_with_asset(&fixt("stac_itemcollection.json"), Some("band")).unwrap();
    // bracket only the first datetime
    let mut bounds = HashMap::new();
    bounds.insert("time".to_string(), (Some(1767225600.0 - 10.0), Some(1767225600.0 + 10.0)));
    let constraints = QueryConstraints { bounds, pins: HashMap::new() };
    let (bmin, bmax) = ds.compute_bounds(&constraints);
    assert_eq!((bmin[0], bmax[0]), (0, 0), "time should prune to index 0 only");
}
```

- [ ] **Step 2: Run + iterate**

Run: `cargo test -p geozarr_core --test stac_timestack_e2e`. Most likely fix points if it fails: ensure `CoordinateResolver` opens `/time` (the store serves `time/.zarray` + `time/0`), and that `Array::open(store, "/band")` reads `band/.zattrs` for dims (the store serves it). If `ds.coords`/`ds.shape`/`ds.dim_names` aren't `pub`, use the public accessors. Iterate on Task 2/5 code only if a real integration gap appears (keep assertions intact).

- [ ] **Step 3: Full crate test + lints**

Run: `cargo test -p geozarr_core` (0 failures; live STAC ignored), `cargo clippy -p geozarr_core --lib --tests -- -D warnings`, `cargo fmt --check`.

- [ ] **Step 4: Commit**
```bash
git add geozarr_core/tests/stac_timestack_e2e.rs
git commit --no-gpg-sign -m "test(stac): end-to-end 3D time-stack read with temporal pushdown

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 7: Docs

**Files:** Modify `docs/docs/engineering/cog_virtualization.mdx`, `docs/docs/usage/sql_read_geo.md`.

- [ ] **Step 1: Engineering page** — extend the STAC section: a STAC **ItemCollection** (local or one HTTP response) stacks the selected asset across its Items into a 3D `[time, lat, lon]` array, `time` = epoch seconds from `properties.datetime`; the collection must be **grid-uniform** (all items/assets share shape/affine/CRS), else a clear error; **not yet supported:** pagination (`rel:next` ignored), multi-resolution collections, regridding/reprojection, items without `datetime`.

- [ ] **Step 2: SQL reference** — note a STAC ItemCollection is a supported `read_geo` source: `asset` selects the stacked asset; the result has a `time` dimension; `time_min`/`time_max` are **epoch seconds**. Same honest limits.

- [ ] **Step 3: Build** — `cd docs && (test -d node_modules || npm ci) && npm run build 2>&1 | tail -5` → `[SUCCESS]`, no broken links.

- [ ] **Step 4: Commit**
```bash
git add docs/docs/engineering/cog_virtualization.mdx docs/docs/usage/sql_read_geo.md
git commit --no-gpg-sign -m "docs: document STAC ItemCollection time-stacking in read_geo

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 8: Verification

- [ ] **Step 1: Workspace** — `cargo test` (0 failures; live STAC ignored), `cargo fmt --check`, `cargo clippy -p geozarr_core --lib --tests -- -D warnings` (clean for touched files), `cargo build -p eider_extension` (compiles against unchanged `open_with_asset`).
- [ ] **Step 2: Docs** — `cd docs && npm run build 2>&1 | tail -3` → `[SUCCESS]`.
- [ ] **Step 3: Scope** — `git diff --name-status main..HEAD | grep -v superpowers` should show only: `geozarr_core/Cargo.toml`, `Cargo.lock`, `geozarr_core/src/{datetime.rs,virtual_stac_time_stack.rs,lib.rs,store.rs,virtual_store.rs}`, the three `tests/fixtures/stac_itemcollection*.json`, `tests/stac_timestack_store.rs`, `tests/stac_timestack_e2e.rs`, and the two docs files. Confirm `extension/` and `dataset.rs` are **not** changed (open_with_asset reused as-is).

---

## Self-review notes (author checklist, already applied)

- **Spec coverage:** epoch-seconds `/time` (Task 1+2); 3D `VirtualStacTimeStack` routing `{asset}/t.y.x` + synthesized `/time`,`/lat`,`/lon` (Task 2); collection-wide grid-uniformity validation with clear errors (Task 3); FeatureCollection branch parsing/datetime/sort/eager-fetch local+HTTP (Task 5); chrono (Task 1); `open_with_asset` unchanged (verified Task 8); 2-item fixture reusing COG fixtures + error fixtures (Task 4); offline e2e incl. temporal pushdown + dtype + value (Task 6); error tests (Task 5); live test stays ignored; docs (Task 7).
- **Consistency:** `VirtualStacTimeStack::new(assets, times, lat, lon, [d0,d1]) -> Result`, `asset_names()`, `validate_grid_uniform(&[&CogMetadata])`, `VirtualCogStore::meta()` getter, `rfc3339_to_epoch_seconds` — defined where first used, consumed in Task 5. Coordinate-array `.zarray` is V2 `<f8` shape `[len]` chunks `[len]` chunk `0`, exactly what `CoordinateResolver` reads. 3D chunk `[1,tileH,tileW]` byte-equals the child 2D tile.
- **Placeholders:** none. Explicit verification points: the synthetic-child chunk-routing assertion (Task 2 Step 2 note), `list_dir` API shape (mirror single-Item store), and `ZarrDataset` field visibility in e2e — each names exactly what to confirm. The HTTP branch has no offline test (documented); the local path is the tested one.
- **Non-goals honored:** no pagination, no regridding, no multi-resolution, no extension change, no `dataset.rs` change.
