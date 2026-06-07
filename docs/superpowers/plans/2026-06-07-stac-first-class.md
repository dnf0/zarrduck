# First-Class STAC (single Item) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make a single STAC Item a first-class `read_geo` source: open the Item, select one COG asset (`asset :=` param), and return it as a georeferenced 2D array — reusing the merged first-class COG path.

**Architecture:** `ZarrDataset::open_with_asset` opens the chosen asset **by path** (`Array::open(store, "/{asset}")`) so the child `VirtualCogStore`'s real dtype/affine/CRS flow through unchanged, sidestepping the group-vs-single-array crash. `store.rs` gains local STAC Item JSON support (offline tests) and clear errors for ItemCollections. The extension drops its over-broad short-circuit and adds an `asset` param.

**Tech Stack:** Rust (`geozarr_core`, `eider_extension`), `zarrs`, `serde_json`, committed STAC Item JSON fixture referencing the existing COG fixtures.

---

## Conventions & prerequisites

Repo root `/Users/danielfisher/repos/zarrduck`, branch `feat/stac-first-class` (off `main`, which has the merged COG work). TDD: failing test → run → implement → pass → commit. Conventional Commits; every commit `--no-gpg-sign` ending with:
`Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>`
Pre-commit runs fmt/clippy/whitespace. Never `git add -A`; stage only named files. Tests: `cargo test -p geozarr_core`, `cargo test -p eider_extension` (or `cargo test`); `cargo clippy -p <crate> --lib --tests -- -D warnings` (NOT `--all-targets` — pre-existing unrelated failures on `main`); `cargo fmt --check`. Docs: `cd docs && (test -d node_modules || npm ci) && npm run build`.

### Established source facts (don't re-derive)
- `extension/src/table_function.rs:101-106` short-circuit:
  ```rust
  let dataset = if path.contains("/search") || path.contains("items") {
      return Err("STAC FeatureCollections not fully implemented yet".into());
  } else {
      geozarr_core::dataset::ZarrDataset::open(&path)?
  };
  ```
  Fixed named params are registered around `:83-86` (`lat_min`/`lat_max`/`lon_min`/`lon_max`, etc.). Bounds are built by `format!("{}_min", name)` per dim (`:117-128`).
- `geozarr_core/src/dataset.rs:21-26`: `open` calls `resolve_sync_store(path)` then `Array::open(store_arc, "/")`. It then reads `array.metadata()` → `parse_geozarr_metadata` (transform), `resolve_dimension_names` (`_ARRAY_DIMENSIONS`), `CoordinateResolver::resolve(path, …)` (finds `/lat` etc.; for COG/STAC these are absent → empty coords → affine used). Struct fields are all `pub`.
- `geozarr_core/src/virtual_stac_store.rs`: `VirtualStacStore` is a Zarr **group**. `new(children: HashMap<String, VirtualCogStore>)` builds `.zgroup` + a consolidated `.zmetadata` containing only `"{name}/.zarray"` entries (NOT `.zattrs`). `get()` routes `"{name}/{key}"` → child. `ListableStorageTraits` methods all return `Err(StorageError::Other(...))`. `StorageError::Other(String)` is the error idiom used throughout.
- `geozarr_core/src/store.rs:179-308`: HTTP branch detects a STAC Item (`stac_version` + `type=="Feature"`), filters COG assets (media type contains `tiff`/`cog` or href ends `.tif`/`.tiff`), resolves relative hrefs against the doc's base URL, fetches each COG header (16 KiB) concurrently, builds `VirtualCogStore::new(operator, "", meta)?` children (fallible since #126 — a bad child fails the whole open), and returns a `VirtualStacStore`. There is a local-FS branch later (handles `.zarr` and `.tif`, with the COG header read clamped to file size). `VirtualCogStore::new` returns `Result<Self, String>`.

## File structure
- Modify: `geozarr_core/src/virtual_stac_store.rs` (Task 1)
- Create: `geozarr_core/tests/fixtures/stac_item.json`, `stac_itemcollection.json` (Task 2)
- Modify: `geozarr_core/src/store.rs` (Task 3)
- Modify: `geozarr_core/src/dataset.rs` (Task 4)
- Create: `geozarr_core/tests/stac_e2e.rs` (Task 4)
- Modify: `extension/src/table_function.rs` (Task 5); Create: `extension/tests/test_stac_eval.rs` (Task 5)
- Modify: `geozarr_core/tests/test_stac_fallback.rs` (Task 6 — `#[ignore]`)
- Modify: `docs/docs/engineering/cog_virtualization.mdx`, `docs/docs/usage/sql_read_geo.md` (Task 7)
- Verify: Task 8

---

## Task 1: `VirtualStacStore` — propagate child `.zattrs` + implement listing

**Files:** Modify `geozarr_core/src/virtual_stac_store.rs`.

- [ ] **Step 1: Write failing tests**

Add a `tests` module at the end of `virtual_stac_store.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::cog::CogMetadata;

    fn child() -> VirtualCogStore {
        let mut meta = CogMetadata {
            image_width: 4, image_length: 2, tile_width: 4, tile_length: 2,
            tile_offsets: vec![0], tile_byte_counts: vec![16],
            is_little_endian: true, bits_per_sample: 16, sample_format: 2,
            samples_per_pixel: 1, compression: 1, predictor: 1, ..Default::default()
        };
        meta.pixel_scale = vec![2.0, 2.0, 0.0];
        meta.tiepoint = vec![0.0, 0.0, 0.0, -180.0, 90.0, 0.0];
        meta.epsg = Some(4326);
        let op = opendal::Operator::new(opendal::services::Memory::default()).unwrap().finish();
        VirtualCogStore::new(op, "".to_string(), meta).unwrap()
    }

    #[test]
    fn zmetadata_includes_child_zattrs() {
        let mut m = HashMap::new();
        m.insert("band".to_string(), child());
        let store = VirtualStacStore::new(m);
        let zmeta = String::from_utf8(
            store.get(&StoreKey::new(".zmetadata").unwrap()).unwrap().unwrap().to_vec()
        ).unwrap();
        assert!(zmeta.contains("band/.zarray"));
        assert!(zmeta.contains("band/.zattrs"), "group metadata must carry child .zattrs: {zmeta}");
        assert!(zmeta.contains("spatial_transform"));
    }

    #[test]
    fn routes_child_zattrs_key() {
        let mut m = HashMap::new();
        m.insert("band".to_string(), child());
        let store = VirtualStacStore::new(m);
        let za = store.get(&StoreKey::new("band/.zattrs").unwrap()).unwrap();
        assert!(za.is_some(), "band/.zattrs must route to the child");
    }

    #[test]
    fn list_returns_child_keys_not_error() {
        let mut m = HashMap::new();
        m.insert("band".to_string(), child());
        let store = VirtualStacStore::new(m);
        let keys = store.list().expect("list must succeed");
        let set: Vec<String> = keys.iter().map(|k| k.as_str().to_string()).collect();
        assert!(set.iter().any(|k| k == "band/.zarray"));
        assert!(set.iter().any(|k| k == "band/.zattrs"));
    }
}
```

- [ ] **Step 2: Run (fail)** — `cargo test -p geozarr_core virtual_stac_store::` → FAIL (`.zattrs` not in metadata; `list` errors).

- [ ] **Step 3: Implement**

In `VirtualStacStore::new`, after inserting `"{name}/.zarray"`, also insert `.zattrs`:
```rust
for (name, child) in &children {
    if let Ok(Some(bytes)) = child.get(&StoreKey::new(".zarray").unwrap()) {
        if let Ok(json) = serde_json::from_slice::<serde_json::Value>(&bytes) {
            metadata_map.insert(format!("{}/.zarray", name), json);
        }
    }
    if let Ok(Some(bytes)) = child.get(&StoreKey::new(".zattrs").unwrap()) {
        if let Ok(json) = serde_json::from_slice::<serde_json::Value>(&bytes) {
            metadata_map.insert(format!("{}/.zattrs", name), json);
        }
    }
}
```
Replace the four `ListableStorageTraits` method bodies:
```rust
fn list(&self) -> Result<zarrs::storage::StoreKeys, zarrs::storage::StorageError> {
    let mut keys = vec![
        StoreKey::new(".zgroup").unwrap(),
        StoreKey::new(".zmetadata").unwrap(),
    ];
    for name in self.children.keys() {
        keys.push(StoreKey::new(&format!("{}/.zarray", name)).unwrap());
        keys.push(StoreKey::new(&format!("{}/.zattrs", name)).unwrap());
    }
    Ok(keys)
}
fn list_prefix(&self, prefix: &StorePrefix) -> Result<zarrs::storage::StoreKeys, zarrs::storage::StorageError> {
    let p = prefix.as_str();
    Ok(self.list()?.into_iter().filter(|k| k.as_str().starts_with(p)).collect())
}
fn list_dir(&self, _prefix: &StorePrefix) -> Result<zarrs::storage::StoreKeysPrefixes, zarrs::storage::StorageError> {
    // Group discovery uses consolidated .zmetadata; a precise dir listing isn't needed.
    Ok(zarrs::storage::StoreKeysPrefixes::new(vec![], vec![]))
}
fn size_prefix(&self, _prefix: &StorePrefix) -> Result<u64, zarrs::storage::StorageError> {
    Ok(0)
}
```
> If `StoreKeysPrefixes::new` has a different constructor/signature in this zarrs version, build it via whatever public constructor exists (check `zarrs::storage::StoreKeysPrefixes`); the intent is an empty result, not `Err`.

- [ ] **Step 4: Run (pass)** — `cargo test -p geozarr_core virtual_stac_store::` → PASS. Then `cargo clippy -p geozarr_core --lib -- -D warnings` and `cargo fmt --check` clean.

- [ ] **Step 5: Commit**
```bash
git add geozarr_core/src/virtual_stac_store.rs
git commit --no-gpg-sign -m "feat(stac): propagate child .zattrs into group metadata and implement listing

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 2: STAC fixtures

**Files:** Create `geozarr_core/tests/fixtures/stac_item.json`, `geozarr_core/tests/fixtures/stac_itemcollection.json`.

- [ ] **Step 1: Write the Item fixture**

`geozarr_core/tests/fixtures/stac_item.json` — references the two committed COG fixtures via relative hrefs:
```json
{
  "stac_version": "1.0.0",
  "type": "Feature",
  "id": "fixture-item",
  "geometry": {"type": "Polygon", "coordinates": [[[-180,82],[-172,82],[-172,90],[-180,90],[-180,82]]]},
  "bbox": [-180, 82, -172, 90],
  "properties": {"datetime": "2026-06-07T00:00:00Z"},
  "assets": {
    "band_uncompressed": {
      "href": "./cog_int16_uncompressed.tif",
      "type": "image/tiff; application=geotiff; profile=cloud-optimized"
    },
    "band_deflate": {
      "href": "./cog_int16_deflate.tif",
      "type": "image/tiff; application=geotiff; profile=cloud-optimized"
    }
  },
  "links": []
}
```

- [ ] **Step 2: Write the ItemCollection fixture (for the not-supported-error test)**

`geozarr_core/tests/fixtures/stac_itemcollection.json`:
```json
{ "stac_version": "1.0.0", "type": "FeatureCollection", "features": [] }
```

- [ ] **Step 3: Commit**
```bash
git add geozarr_core/tests/fixtures/stac_item.json geozarr_core/tests/fixtures/stac_itemcollection.json
git commit --no-gpg-sign -m "test(stac): add local STAC Item and ItemCollection fixtures

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 3: `store.rs` — local STAC Item JSON support + ItemCollection error

**Files:** Modify `geozarr_core/src/store.rs`.

- [ ] **Step 1: Write the failing integration test**

Create `geozarr_core/tests/stac_store.rs`:
```rust
use geozarr_core::store::resolve_sync_store;
use zarrs::storage::{ReadableStorageTraits, StoreKey};

fn fixt(name: &str) -> String {
    format!("{}/tests/fixtures/{}", env!("CARGO_MANIFEST_DIR"), name)
}

#[test]
fn local_stac_item_resolves_to_group_with_assets() {
    std::env::set_var("GEOZARR_ALLOW_PATH", env!("CARGO_MANIFEST_DIR"));
    let resolved = resolve_sync_store(&fixt("stac_item.json")).expect("STAC item should resolve");
    let zmeta = resolved.store
        .get(&StoreKey::new(".zmetadata").unwrap()).unwrap().unwrap();
    let s = String::from_utf8(zmeta.to_vec()).unwrap();
    assert!(s.contains("band_uncompressed/.zarray"));
    assert!(s.contains("band_deflate/.zarray"));
}

#[test]
fn local_itemcollection_is_clear_error() {
    std::env::set_var("GEOZARR_ALLOW_PATH", env!("CARGO_MANIFEST_DIR"));
    let err = resolve_sync_store(&fixt("stac_itemcollection.json")).unwrap_err();
    let msg = format!("{err}");
    assert!(msg.contains("ItemCollection") || msg.contains("not yet supported"), "got: {msg}");
}
```

- [ ] **Step 2: Run (fail)** — `cargo test -p geozarr_core --test stac_store` → FAIL (local STAC JSON not recognized; today it would try to open the `.json` as a Zarr/COG and error differently).

- [ ] **Step 3: Implement the local STAC branch**

First read the existing **local-FS branch** of `resolve_sync_store` (the `else` block after the `http(s)://` branch — it handles `.zarr` and `.tif`, builds an `opendal::services::Fs` operator, and reads the COG header clamped to file size). Mirror its operator/clamp idiom in a small helper, then add a STAC-JSON detection branch.

Add a helper (module-level in `store.rs`) that builds a local COG child from an absolute file path, mirroring the existing local-COG operator + clamped header read:
```rust
fn build_local_cog_child(abs_path: &std::path::Path) -> Result<crate::virtual_store::VirtualCogStore, String> {
    let parent = abs_path.parent().ok_or("bad COG path")?;
    let fname = abs_path.file_name().and_then(|f| f.to_str()).ok_or("bad COG filename")?.to_string();
    let builder = opendal::services::Fs::default().root(parent.to_str().ok_or("bad COG dir")?);
    let operator = opendal::Operator::new(builder).map_err(|e| e.to_string())?.finish();
    let file_len = std::fs::metadata(abs_path).map_err(|e| e.to_string())?.len();
    let header_len = file_len.min(16384);
    let header_bytes = std::thread::spawn({
        let operator = operator.clone();
        let fname = fname.clone();
        move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async { operator.read_with(&fname).range(0..header_len).await })
                .map(|b| b.to_vec())
                .map_err(|e| e.to_string())
        }
    }).join().unwrap()?;
    let meta = crate::cog::parse_cog_metadata(&header_bytes)?;
    crate::virtual_store::VirtualCogStore::new(operator, fname, meta)
}
```
> VERIFY this against the existing local-COG branch: match how it constructs the `Fs` operator `root` and the `read_with(&fname)` filename, and confirm `VirtualCogStore`'s `filename` field is used as the read key (it is — `get()` calls `op.read_with(&self.filename)`). Adjust `root`/`fname` so the read resolves. If the existing branch already exposes a reusable helper, call it instead of duplicating.

In the **local-FS branch**, before the generic Zarr fallback and when the path is not `.zarr`/`.tif`, add:
```rust
// Local STAC Item JSON?
if let Ok(text) = std::fs::read_to_string(path) {
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) {
        if json.get("stac_version").is_some() {
            match json.get("type").and_then(|t| t.as_str()) {
                Some("FeatureCollection") => {
                    return Err("STAC ItemCollection / search results are not yet supported (single Items only)".into());
                }
                Some("Feature") => {
                    let base = std::path::Path::new(path).parent()
                        .ok_or("bad STAC path")?.to_path_buf();
                    let assets = json.get("assets").and_then(|a| a.as_object())
                        .ok_or("STAC Item has no assets")?;
                    let mut children = std::collections::HashMap::new();
                    for (name, asset) in assets {
                        let t = asset.get("type").and_then(|t| t.as_str()).unwrap_or("");
                        let href = asset.get("href").and_then(|h| h.as_str()).unwrap_or("");
                        let is_cog_asset = t.contains("tiff") || t.contains("cog")
                            || href.ends_with(".tif") || href.ends_with(".tiff");
                        if !is_cog_asset { continue; }
                        let abs = if std::path::Path::new(href).is_absolute() {
                            std::path::PathBuf::from(href)
                        } else {
                            base.join(href)
                        };
                        let child = build_local_cog_child(&abs)?;
                        children.insert(name.to_string(), child);
                    }
                    if children.is_empty() {
                        return Err("STAC Item has no COG assets".into());
                    }
                    let store = std::sync::Arc::new(
                        crate::virtual_stac_store::VirtualStacStore::new(children));
                    return Ok(ResolvedStore { store, is_remote: false });
                }
                _ => {}
            }
        }
    }
}
```
Also add the same `FeatureCollection` → not-supported error to the **HTTP** branch (right after the JSON is parsed, before/alongside the `type=="Feature"` check), so a `/search` URL returns the clear message rather than falling through.

> NOTE on the `GEOZARR_ALLOW_PATH` gate: the local-FS branch likely checks this env var before allowing a read. Ensure the STAC JSON read and the child COG reads are permitted under the same gate (the e2e tests set `GEOZARR_ALLOW_PATH`). If the gate is enforced in one place, the JSON `read_to_string` above may bypass it — keep behavior consistent (gate the JSON read too, or rely on the child COG reads being gated). Match the existing gate semantics.

- [ ] **Step 4: Run (pass)** — `cargo test -p geozarr_core --test stac_store` → PASS. Clippy `--lib --tests`, fmt clean.

- [ ] **Step 5: Commit**
```bash
git add geozarr_core/src/store.rs geozarr_core/tests/stac_store.rs
git commit --no-gpg-sign -m "feat(stac): resolve local STAC Item JSON; clear error for ItemCollections

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 4: `dataset.rs` — `open_with_asset` (asset selection by path)

**Files:** Modify `geozarr_core/src/dataset.rs`; Create `geozarr_core/tests/stac_e2e.rs`.

- [ ] **Step 1: Write the failing unit test for asset selection**

Add to a `tests` module in `dataset.rs` (pure selection helper, no store):
```rust
#[cfg(test)]
mod select_tests {
    use super::select_array_path;
    fn meta(names: &[&str]) -> String {
        let entries: Vec<String> = names.iter()
            .map(|n| format!("\"{n}/.zarray\":{{}}")).collect();
        format!(r#"{{"metadata":{{".zgroup":{{}},{}}},"zarr_consolidated_format":1}}"#, entries.join(","))
    }
    #[test]
    fn picks_named_asset() {
        assert_eq!(select_array_path(&meta(&["red","nir"]), Some("nir")).unwrap(), "/nir");
    }
    #[test]
    fn auto_selects_single_asset() {
        assert_eq!(select_array_path(&meta(&["only"]), None).unwrap(), "/only");
    }
    #[test]
    fn errors_on_multiple_without_asset() {
        let e = select_array_path(&meta(&["red","nir"]), None).unwrap_err();
        assert!(e.contains("red") && e.contains("nir") && e.contains("asset"));
    }
    #[test]
    fn errors_on_unknown_asset() {
        let e = select_array_path(&meta(&["red","nir"]), Some("green")).unwrap_err();
        assert!(e.contains("green") || e.contains("Available"));
    }
}
```

- [ ] **Step 2: Run (fail)** — `cargo test -p geozarr_core select_tests` → FAIL (no `select_array_path`).

- [ ] **Step 3: Implement `select_array_path` + `open_with_asset`**

Add the pure helper (module-level in `dataset.rs`):
```rust
/// Given a group's consolidated `.zmetadata` JSON and an optional asset name,
/// return the array path to open (e.g. "/red"). Errors list available assets.
pub(crate) fn select_array_path(zmetadata: &str, asset: Option<&str>) -> Result<String, String> {
    let v: serde_json::Value = serde_json::from_str(zmetadata).map_err(|e| e.to_string())?;
    let meta = v.get("metadata").and_then(|m| m.as_object())
        .ok_or("invalid group metadata")?;
    let mut names: Vec<String> = meta.keys()
        .filter_map(|k| k.strip_suffix("/.zarray").map(|s| s.to_string()))
        .collect();
    names.sort();
    match asset {
        Some(a) if names.iter().any(|n| n == a) => Ok(format!("/{a}")),
        Some(a) => Err(format!("asset '{a}' not found. Available: {}", names.join(", "))),
        None if names.len() == 1 => Ok(format!("/{}", names[0])),
        None if names.is_empty() => Err("STAC group has no assets".into()),
        None => Err(format!(
            "STAC Item has multiple assets; choose one with asset := '<name>'. Available: {}",
            names.join(", ")
        )),
    }
}
```
Refactor `open` into `open_with_asset`:
```rust
pub fn open(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
    Self::open_with_asset(path, None)
}

pub fn open_with_asset(path: &str, asset: Option<&str>) -> Result<Self, Box<dyn std::error::Error>> {
    let resolved_store = crate::store::resolve_sync_store(path)?;
    let is_remote = resolved_store.is_remote;
    let store_arc = resolved_store.store;

    // Root array (plain Zarr/COG) vs group (STAC): probe for a root `.zarray`.
    let has_root_array = store_arc
        .get(&zarrs::storage::StoreKey::new(".zarray").unwrap())
        .map(|o| o.is_some())
        .unwrap_or(false);
    let array_path = if has_root_array {
        "/".to_string()
    } else {
        let zmeta = store_arc
            .get(&zarrs::storage::StoreKey::new(".zmetadata").unwrap())
            .ok()
            .flatten()
            .ok_or_else(|| -> Box<dyn std::error::Error> {
                "source is neither a Zarr array nor a STAC group".into()
            })?;
        let zmeta = String::from_utf8(zmeta.to_vec()).map_err(|e| -> Box<dyn std::error::Error> { e.to_string().into() })?;
        select_array_path(&zmeta, asset).map_err(|e| -> Box<dyn std::error::Error> { e.into() })?
    };

    let array = Array::open(Arc::clone(&store_arc), &array_path).map_err(
        |e| -> Box<dyn std::error::Error> { format!("zarrs error (array): {}", e).into() },
    )?;
    // ... REST OF THE EXISTING `open` BODY UNCHANGED (shape, metadata, dim_names,
    //     CoordinateResolver::resolve(path, ...), chunk_shape, data_type, validate, fill value, Ok(Self{...})) ...
}
```
> Keep the remainder of the current `open` body verbatim after `let array = ...`. Only the store-resolution + `array_path` selection and the `Array::open(..., &array_path)` argument change. `CoordinateResolver::resolve` keeps receiving the original `path` (STAC assets have no `/lat` arrays → empty coords → affine path, exactly like a standalone COG).

- [ ] **Step 4: Run unit (pass)** — `cargo test -p geozarr_core select_tests` → PASS.

- [ ] **Step 5: Write the e2e test**

Create `geozarr_core/tests/stac_e2e.rs`:
```rust
use geozarr_core::dataset::ZarrDataset;

fn fixt(name: &str) -> String {
    format!("{}/tests/fixtures/{}", env!("CARGO_MANIFEST_DIR"), name)
}
fn allow() { std::env::set_var("GEOZARR_ALLOW_PATH", env!("CARGO_MANIFEST_DIR")); }

#[test]
fn stac_asset_is_georeferenced_like_the_cog() {
    allow();
    let ds = ZarrDataset::open_with_asset(&fixt("stac_item.json"), Some("band_uncompressed")).unwrap();
    assert_eq!(ds.dim_names, vec!["lat".to_string(), "lon".to_string()]);
    assert!(ds.spatial_transform.is_some());
    let cog = ZarrDataset::open(&fixt("cog_int16_uncompressed.tif")).unwrap();
    assert_eq!(ds.shape, cog.shape);
    let (_, vtype) = ds.schema().unwrap().pop().unwrap();
    assert_eq!(format!("{vtype:?}"), format!("{:?}", zarrs::array::DataType::Int16));
}

#[test]
fn stac_multiple_assets_without_selection_errors() {
    allow();
    let err = ZarrDataset::open(&fixt("stac_item.json")).unwrap_err();
    let msg = format!("{err}");
    assert!(msg.contains("band_uncompressed") && msg.contains("band_deflate"), "got: {msg}");
}

#[test]
fn stac_unknown_asset_errors() {
    allow();
    let err = ZarrDataset::open_with_asset(&fixt("stac_item.json"), Some("nope")).unwrap_err();
    assert!(format!("{err}").contains("nope") || format!("{err}").contains("Available"));
}
```

- [ ] **Step 6: Run e2e (pass)** — `cargo test -p geozarr_core --test stac_e2e`. Iterate on Tasks 1/3/4 if the asset doesn't open georeferenced (most likely fix point: ensure `Array::open(store, "/band_uncompressed")` finds `band_uncompressed/.zattrs` — covered by Task 1's metadata propagation + `get` routing). Then `cargo test -p geozarr_core`, clippy `--lib --tests`, fmt — all clean.

- [ ] **Step 7: Commit**
```bash
git add geozarr_core/src/dataset.rs geozarr_core/tests/stac_e2e.rs
git commit --no-gpg-sign -m "feat(stac): open a selected STAC asset by path (open_with_asset)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 5: Extension — drop short-circuit, add `asset` param

**Files:** Modify `extension/src/table_function.rs`; Create `extension/tests/test_stac_eval.rs`.

- [ ] **Step 1: Implement the dispatch + param changes**

In `ReadGeoVTab::bind` (and the matching `plan_read_geo` bind if it mirrors this):
- Register the `asset` named parameter where the other named params are registered (near `:83-86`):
  ```rust
  named_parameters.push(("asset".to_string(), LogicalTypeId::Varchar.into()));
  ```
  (match the exact registration idiom used for `lat_min`, e.g. `bind.add_named_parameter(...)` or the params vec — mirror the surrounding code.)
- Replace the short-circuit (`:101-106`) with a direct open that threads the asset param:
  ```rust
  let asset = bind.get_named_parameter("asset").map(|v| v.to_string());
  let dataset = geozarr_core::dataset::ZarrDataset::open_with_asset(&path, asset.as_deref())?;
  ```
  Remove the `path.contains("/search") || path.contains("items")` block entirely. (A `/search`/ItemCollection URL now yields the clear `geozarr_core` error.)

- [ ] **Step 2: Write the extension e2e test**

Create `extension/tests/test_stac_eval.rs`, mirroring the existing `extension/tests/test_cog_eval.rs` harness (read it first for the exact connection/extension-load boilerplate and the `GEOZARR_ALLOW_PATH` handling):
```rust
// Mirror test_cog_eval.rs setup (load extension, open connection).
// Use the committed STAC fixture via an absolute path built from CARGO_MANIFEST_DIR
// (the geozarr_core fixtures dir: ../geozarr_core/tests/fixtures/stac_item.json).
#[test]
fn read_geo_stac_asset_returns_rows() {
    // set GEOZARR_ALLOW_PATH appropriately (as test_cog_eval does)
    // SELECT count(*) FROM read_geo('<abs stac_item.json>', asset := 'band_uncompressed')
    // assert the query succeeds and row count > 0 (4x2 grid -> 8 rows)
}
```
Write the concrete body to match `test_cog_eval.rs`'s patterns (connection, `allow_unsigned_extensions`, `LOAD`, query, assert rows == 8 or > 0). If `test_cog_eval.rs` is `#[ignore]`/conditional on a fixture, follow the same convention but point at the committed STAC fixture so it runs.

- [ ] **Step 3: Run** — `cargo test -p eider_extension --test test_stac_eval` (and ensure the crate builds: `cargo build -p eider_extension`). Expected: PASS (8 rows). If the extension test harness can't easily run in this environment (matching whatever constraints `test_cog_eval.rs` already documents), keep the test but mark it with the same gating `test_cog_eval.rs` uses, and rely on the `geozarr_core` e2e (Task 4) as the primary proof — note this in the commit.

- [ ] **Step 4: Build + clippy + fmt** — `cargo build -p eider_extension`; `cargo clippy -p eider_extension --lib --tests -- -D warnings`; `cargo fmt --check`.

- [ ] **Step 5: Commit**
```bash
git add extension/src/table_function.rs extension/tests/test_stac_eval.rs
git commit --no-gpg-sign -m "feat(stac): wire read_geo asset param; drop the STAC short-circuit

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 6: Mark the live STAC test `#[ignore]`

**Files:** Modify `geozarr_core/tests/test_stac_fallback.rs`.

- [ ] **Step 1: Annotate**

Add `#[ignore = "hits live Sentinel-2 STAC endpoint; run with --ignored when online"]` above the `#[test]` (keep the test intact so it's runnable on demand).

- [ ] **Step 2: Verify it's skipped** — `cargo test -p geozarr_core --test test_stac_fallback` → reports `0 passed; … 1 ignored` (or the test is filtered). `cargo fmt --check` clean.

- [ ] **Step 3: Commit**
```bash
git add geozarr_core/tests/test_stac_fallback.rs
git commit --no-gpg-sign -m "test(stac): ignore the live-network STAC fallback test by default

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 7: Docs — STAC single-Item is first-class

**Files:** Modify `docs/docs/engineering/cog_virtualization.mdx`, `docs/docs/usage/sql_read_geo.md`.

- [ ] **Step 1: Update the engineering page**

In `cog_virtualization.mdx`, rewrite the STAC section from "planned / not wired to SQL" to:
```markdown
## STAC (single Item)

A single STAC **Item** is a first-class `read_geo` source. Eider fetches the
Item (local path or `http(s)://`), composes its COG assets as a virtual Zarr
group, and you select one asset:

```sql
SELECT * FROM read_geo('item.json', asset := 'red');
```

If the Item has exactly one COG asset it is selected automatically; with
multiple assets and none chosen, `read_geo` errors listing the available asset
names. Each selected asset is read with the full COG pipeline (real dtype,
GeoTIFF affine, CRS).

**Not yet supported:** STAC ItemCollections and `/search` results (multiple
Items / time-stacking), stacking multiple assets into one array, and STAC
Collection/Catalog traversal — these return a clear error.
```

- [ ] **Step 2: Update the SQL reference**

In `docs/docs/usage/sql_read_geo.md`: document the `asset` named parameter (VARCHAR — selects a COG asset from a STAC Item) and that a STAC Item (local or HTTP) is a supported source (single Item; pick an asset; auto-selected if only one). Keep the not-yet-supported list (ItemCollection/search/time) honest. Match the page's existing parameter-table / source-list structure.

- [ ] **Step 3: Build** — `cd docs && (test -d node_modules || npm ci) && npm run build 2>&1 | tail -5` → `[SUCCESS]`, no broken links.

- [ ] **Step 4: Commit**
```bash
git add docs/docs/engineering/cog_virtualization.mdx docs/docs/usage/sql_read_geo.md
git commit --no-gpg-sign -m "docs: STAC single-Item asset selection is first-class in read_geo

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 8: Full verification

**Files:** none.

- [ ] **Step 1: Workspace green**
```bash
cargo test
cargo fmt --check
cargo clippy -p geozarr_core --lib --tests -- -D warnings
cargo clippy -p eider_extension --lib --tests -- -D warnings
```
Expected: all pass (new STAC unit/e2e/store tests pass; the live STAC test is ignored; COG and all prior tests unaffected). Do NOT gate on `--all-targets` (pre-existing unrelated failures on `main`).

- [ ] **Step 2: Extension builds** — `cargo build -p eider_extension` succeeds against `open_with_asset`.

- [ ] **Step 3: Docs build** — `cd docs && npm run build 2>&1 | tail -3` → `[SUCCESS]`.

- [ ] **Step 4: Scope check** — `git diff --name-status main..HEAD` expected:
```
M docs/docs/engineering/cog_virtualization.mdx
M docs/docs/usage/sql_read_geo.md
M extension/src/table_function.rs
A extension/tests/test_stac_eval.rs
M geozarr_core/src/dataset.rs
M geozarr_core/src/store.rs
M geozarr_core/src/virtual_stac_store.rs
A geozarr_core/tests/fixtures/stac_item.json
A geozarr_core/tests/fixtures/stac_itemcollection.json
A geozarr_core/tests/stac_e2e.rs
A geozarr_core/tests/stac_store.rs
M geozarr_core/tests/test_stac_fallback.rs
A docs/superpowers/plans/2026-06-07-stac-first-class.md
A docs/superpowers/specs/2026-06-07-stac-first-class-design.md
```
Confirm `virtual_store.rs` and `cog.rs` are NOT modified (the COG layer is reused untouched), and no new COG binary fixtures were added.

---

## Self-review notes (author checklist, already applied)

- **Spec coverage:** asset selection (`open_with_asset` + `select_array_path`, Task 4) ✓; `asset` param + short-circuit removal (Task 5) ✓; local STAC Item JSON + ItemCollection error (Task 3) ✓; `.zattrs` propagation + listing (Task 1) ✓; fixtures reusing COGs (Task 2) ✓; offline e2e + unit + extension tests (Tasks 1,3,4,5) ✓; live test `#[ignore]` (Task 6) ✓; docs (Task 7) ✓; CI/scope gate (Task 8) ✓; single-Item-only + multi-band-fails-loudly (inherited) honored.
- **Type consistency:** `open_with_asset(path, Option<&str>)` and `open` delegate; `select_array_path(&str, Option<&str>) -> Result<String,String>` used in Task 4 tests and impl; `VirtualStacStore` keys `{name}/.zarray|.zattrs` consistent across Tasks 1/3/4; `VirtualCogStore::new -> Result` (from #126) honored in `build_local_cog_child`.
- **Placeholders:** none. Explicit verification points (justified, not vague): (a) Task 3 `build_local_cog_child` must match the existing local-COG `Fs` operator/filename/`GEOZARR_ALLOW_PATH` idiom — the implementer reads that branch and mirrors it; (b) Task 5 extension test mirrors `test_cog_eval.rs`'s harness/gating; (c) `StoreKeysPrefixes` constructor verified against the installed zarrs. Each names exactly what to confirm and where.
- **Non-goals honored:** no ItemCollection/search/time-stacking, no asset-band stacking, no reprojection, no `virtual_store.rs`/`cog.rs` changes, no dispatch logic beyond removing the short-circuit + adding the param.
