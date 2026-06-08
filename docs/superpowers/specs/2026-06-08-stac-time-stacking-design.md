# Design: STAC ItemCollection Time-Stacking in `read_geo`

- **Date:** 2026-06-08
- **Status:** Approved (design); implementation pending
- **Scope:** `read_geo` on a STAC **ItemCollection / FeatureCollection** (local JSON or a single HTTP response) stacks the selected COG asset across its Items into a **3D array `[time, lat, lon]`**, with `time` = epoch seconds parsed from each Item's `properties.datetime`. Third and final STAC source-wiring sub-project (single-Item STAC #127 and first-class COG #126 merged). Rust workstream (`geozarr_core`) + docs; the extension needs no change.

## Context

`resolve_sync_store` currently returns a clear "ItemCollection / search results are not yet supported (single Items only)" error for `type=="FeatureCollection"` in both the local and HTTP branches. This workstream replaces that error with real time-stacking, reusing everything single-Item STAC + first-class COG established:

- `VirtualCogStore` synthesizes a correct 2D COG array (real dtype, GeoTIFF affine, CRS; multi-band fails loudly at open).
- `ResolvedStore { store, is_remote, stac_assets: Option<Vec<String>> }` signals a STAC group and its asset names; `ZarrDataset::open_with_asset(path, Option<&str>)` selects an asset and opens it **by path** (`Array::open(store, "/{asset}")`).
- `CoordinateResolver` opens sibling `/time`, `/lat`, `/lon` coordinate arrays from the store root; `compute_bounds` (`dataset.rs`) does binary-search pushdown for any dimension that has a coordinate array (the same path the real `climate_data.zarr` 3D array uses), and `time_min`/`time_max`/`lat_*`/`lon_*` are the registered `read_geo` params.
- Local STAC Item JSON resolution + the `GEOZARR_ALLOW_PATH`-sandboxed local COG reader exist and are offline-testable.

## Decisions (from brainstorming)

| Decision | Choice |
|---|---|
| `time` coordinate values | **Epoch seconds** parsed from `properties.datetime` (RFC3339). Synthesize a `/time` coordinate array so the existing `/time` + `time_min`/`time_max` pushdown works for real temporal filtering. Add `chrono`. |
| Per-item heterogeneity | **Strict.** All items' selected-asset COGs must share identical `(shape, affine, CRS, dtype)`; any mismatch → clear error naming the item index + field. No regridding/reprojection. |
| Input scope | **Single response only**, no pagination. Stack exactly the `features` in the given ItemCollection/FeatureCollection (local file or one HTTP response). `rel:next` is ignored (documented). |

## Architecture: the 3D stack as a coordinate-complete group

A net-new `VirtualStacTimeStack` store (Zarr group) makes the selected asset come back fully coordinate-resolved so all existing downstream logic applies unchanged.

For each asset name, the store holds a **time-sorted `Vec<VirtualCogStore>`** (one per Item) and synthesizes:

- `/{asset}/.zarray` — shape `[N, H, W]`, chunks `[1, tileH, tileW]` (N = item count; H/W/tile from the uniform COG); dtype + fill from the COGs.
- `/{asset}/.zattrs` — `_ARRAY_DIMENSIONS = ["time","lat","lon"]` (geographic CRS) and the `geozarr` block (CRS) carried from the COG.
- **Group-level coordinate arrays** (length-1-chunk 1D float64 arrays):
  - `/time` — the N epoch-second values (sorted ascending).
  - `/lat` — H values from the uniform COG affine (`ty + i·sy`).
  - `/lon` — W values from the uniform COG affine (`tx + j·sx`).
- `.zgroup`, consolidated `.zmetadata` (listing every `{asset}/.zarray`, `{asset}/.zattrs`, and the three coordinate arrays' `.zarray`/data so consolidated reads see them).

**`get()` routing:**
- `.zgroup` / `.zmetadata` → synthesized group metadata.
- `time/.zarray`, `time/0`, and likewise `lat/*`, `lon/*` → the synthesized coordinate arrays.
- `{asset}/.zarray`, `{asset}/.zattrs` → the 3D asset metadata.
- `{asset}/{t}.{y}.{x}` → split off `t`, delegate to `children[asset][t].get("{y}.{x}")` (item *t*'s COG tile byte-range). The 2D spatial chunk math is the existing `VirtualCogStore` logic, unchanged.
- `ListableStorageTraits` returns the full key set (no `Err`), mirroring the single-Item `VirtualStacStore` fix.

**Why coordinate arrays for all three dims (not affine for lat/lon):** it avoids mixing the coord-array and affine branches of `compute_bounds` across dimension indices (which would require a 3-element affine with a non-affine time axis). With `/time`, `/lat`, `/lon` all present, `CoordinateResolver` fills `coords` for every dim and `compute_bounds` uses binary search uniformly — `time_min`/`time_max` (epoch bounds) and the `lat`/`lon` bbox all prune through the existing path with **no new bounds logic**. `lat`/`lon` values are computed once from the validated-uniform affine.

## Data flow

`resolve_sync_store(path)`:
1. Detect `type=="FeatureCollection"` (local + HTTP branches).
2. Read `features`; error if empty.
3. For each feature: require `properties.datetime` (non-null); parse RFC3339 → epoch seconds (error clearly if missing/unparseable). Sort features by epoch ascending.
4. Determine asset names (the COG assets, as single-Item does) from the features (intersection/union of asset keys — use the first item's COG-asset set; require every item to provide each of those assets, else error).
5. For each `(asset, item)`: build a `VirtualCogStore` (fetch the COG header — local via the sandboxed reader, HTTP/S3 concurrently, matching single-Item).
6. **Validate uniformity** per asset: every item's COG `(shape, affine, CRS, dtype)` equals item 0's; mismatch → `Err` naming the item index + differing field.
7. Build the `VirtualStacTimeStack` (synthesizing `/time` from the sorted epochs, `/lat`/`/lon` from the uniform affine). Return `ResolvedStore { store, is_remote, stac_assets: Some(sorted_asset_names) }`.

`ZarrDataset::open_with_asset` is **unchanged**: `stac_assets.is_some()` → `select_asset_path` → `Array::open(store, "/{asset}")` → the 3D array; `CoordinateResolver` finds `/time`,`/lat`,`/lon`; schema → `(time f64, lat f64, lon f64, value <dtype>)`.

Header-fetch cost: resolve eagerly fetches every item×asset COG header (16 KB range reads, concurrent) to build the group and validate — matching the single-Item pattern. Large collections therefore issue many header reads; documented as a known characteristic (pagination/lazy-asset fetch are non-goals).

## Components

- **Create `geozarr_core/src/virtual_stac_time_stack.rs`** — the `VirtualStacTimeStack` store: per-asset `Vec<VirtualCogStore>`, synthesized 3D `.zarray`/`.zattrs`, synthesized `/time`/`/lat`/`/lon` coordinate arrays, `get()`/`ListableStorageTraits` routing, and the uniformity validation + coordinate synthesis helpers.
- **Modify `geozarr_core/src/store.rs`** — replace the two `FeatureCollection` errors with the data-flow above (local + HTTP). Reuse the local sandboxed COG reader and the HTTP/S3 concurrent header fetch. Build epoch-sorted children and the time-stack store.
- **Modify `geozarr_core/Cargo.toml`** — add `chrono` (default features off as feasible; only RFC3339 parsing needed).
- **Modify `geozarr_core/src/metadata.rs` or a small datetime util** — RFC3339 → epoch-seconds helper (unit-tested).
- **Docs** — `docs/docs/engineering/cog_virtualization.mdx` (STAC section: single Item **and** ItemCollection time-stacking now supported, with the epoch-`time` and strict-uniformity semantics and the no-pagination/no-regridding limits) and `docs/docs/usage/sql_read_geo.md` (a STAC ItemCollection is a supported source; `asset` selects the stacked asset; `time_min`/`time_max` are epoch seconds).
- **No extension change** — the `asset` param + dispatch already route ItemCollections (they reach `open_with_asset` exactly like single Items).

## Tests (offline)

- **Fixture** `geozarr_core/tests/fixtures/stac_itemcollection.json` (replace the empty placeholder) — a `FeatureCollection` with **2 Items**, each a single COG asset (`band`) pointing at the two existing COG fixtures (`cog_int16_uncompressed.tif`, `cog_int16_deflate.tif` — identical 4×2 EPSG:4326 Int16 data, a naturally uniform stack), with distinct `properties.datetime` (e.g. `2026-01-01` and `2026-02-01`).
- **e2e** (`geozarr_core/tests/stac_timestack_e2e.rs`):
  - `open_with_asset(itemcollection.json, Some("band"))` → `dim_names == ["time","lat","lon"]`, shape `[2, 2, 4]`, and `/time` coords equal the two parsed epoch seconds (ascending).
  - schema → `(time, lat, lon, value)`; value dtype Int16.
  - `compute_bounds` with `time_min`/`time_max` bracketing only the first datetime prunes the time dim to index 0 (one slice).
  - the two slices' values match the underlying COG (uncompressed vs deflate decode identical).
- **Error tests:** a heterogeneous fixture (an item whose COG differs in shape/CRS) → clear error; an item missing `properties.datetime` → error; an empty `FeatureCollection` → error.
- **Unit:** RFC3339 → epoch-seconds parsing (incl. `Z` and offset forms; reject null/garbage).
- The existing live Sentinel-2 test stays `#[ignore]`.

## Accuracy & verification

- Offline e2e proves the 3D stack opens coordinate-resolved, `time` is real epoch seconds, temporal pushdown prunes, and the slices decode correctly — no network in CI.
- Strict uniformity means a stack is only ever formed from genuinely aligned slices; everything else errors clearly (no silent misalignment), matching the project's honesty bar. Multi-band assets still fail loudly (inherited from COG).
- `cargo test` (workspace) + touched-file clippy + `cargo fmt --check` pass; `npm run build` green. Path-aware CI runs the full Rust matrix (code change) + Docs Build.

## Non-goals (deferred)

- STAC API **pagination** (`rel:next`) — single response only.
- **Regridding / reprojection** — heterogeneous items error rather than being aligned.
- Items without `properties.datetime` (e.g. only `start_datetime`/`end_datetime`) — error for now.
- Stacking multiple assets into a band dimension; STAC Collection/Catalog traversal; non-COG assets.
- Any extension/dispatch change beyond what single-Item already provides.
