# Design: First-Class STAC (single Item) in `read_geo`

- **Date:** 2026-06-07
- **Status:** Approved (design); implementation pending
- **Scope:** Make a single **STAC Item** a first-class `read_geo` SQL source: open the Item, select one COG asset, and return it as a georeferenced 2D array (reusing the just-merged first-class COG path). Works for a local STAC Item JSON (offline-testable) and over HTTP. Rust workstream (`geozarr_core` + `extension`) + docs. Second of the two sequenced source-wiring sub-projects (COG merged in #126; this builds on it).

## Context

The `read_geo` STAC/COG gap was documented honestly during the docs expansion and is now half-closed (COG is first-class). STAC plumbing is partly present but has one hard design gap and several breakages (verified against source):

- **Short-circuit (over-broad):** `extension/src/table_function.rs:101-106` errors when `path.contains("/search") || path.contains("items")` — which also blocks legitimate single-Item URLs (they contain `/items/`).
- **Group-vs-single-array gap (core):** `ZarrDataset::open` does `Array::open(store, "/")` (`geozarr_core/src/dataset.rs:26`), but `VirtualStacStore` is a Zarr **group** of N child arrays (one per asset) with no root `.zarray`. Opening a multi-asset Item crashes today.
- **`.zattrs` dropped at group level:** `virtual_stac_store.rs` copies child `.zarray` into the group `.zmetadata` but not `.zattrs`, hiding the CRS/affine the COG work added.
- **`ListableStorageTraits` unimplemented:** all methods return `Err(...)` in `virtual_stac_store.rs`.
- **Not testable offline:** the STAC doc is fetched via a hardcoded `reqwest::blocking::get` (`store.rs:182`); the only test hits live Sentinel-2 (`geozarr_core/tests/test_stac_fallback.rs`).
- **Input types:** only a single Item (`type=="Feature"`) is recognized; ItemCollection / `/search` are not.

What already works and is reused: COG asset filtering by media-type/href, concurrent COG-header fetch, child-key routing (`red/0.0` → child `VirtualCogStore`), and the now-correct COG metadata synthesis (real dtype/affine/CRS; multi-band fails loudly at open).

## Decisions (from brainstorming)

| Decision | Choice |
|---|---|
| Output model | **Asset selection.** New `read_geo` named param `asset` (VARCHAR). Exactly one COG asset → auto-selected; multiple + none → error listing available assets; named asset absent → error listing available. Returns one georeferenced 2D array. |
| Input types | **Single STAC Item only** (`type=="Feature"`). ItemCollection/`FeatureCollection`/`/search` → clear "not yet supported" error. Time-stacking deferred. |
| Offline testability | **Support a local STAC Item JSON path** (relative asset hrefs resolved against the JSON's directory, pointing at committed COG fixtures). Also a real user-facing capability. Keep the live Sentinel-2 test as `#[ignore]`. |

## Architecture

The keystone is **opening the chosen asset by path** to sidestep the group-vs-single-array gap:

- `resolve_sync_store(path)` returns the type-erased store. For STAC it's a `VirtualStacStore` (a group).
- `ZarrDataset::open_with_asset(path, Option<&str>)` detects a group root (a `.zgroup` and no root `.zarray`), reads the available asset names from the group's `.zmetadata`, applies the selection rules, and calls `Array::open(store, "/{asset}")`.
- Because the child is opened **by path**, `zarrs` requests `{asset}/.zarray` and `{asset}/.zattrs`, which `VirtualStacStore::get()` routes to the child `VirtualCogStore` — and that child already serves the real dtype + `geozarr` affine/CRS/`_ARRAY_DIMENSIONS` from the COG work. So the selected asset returns fully georeferenced with no extra wiring, and the existing schema/bounds/coordinate machinery applies unchanged.

`ZarrDataset::open(path)` becomes a thin delegate: `open_with_asset(path, None)`. For a non-group (plain Zarr/COG) store, `asset` is ignored if `None`; a stray `asset` on a non-STAC source is tolerated (ignored) to keep `read_geo`'s fixed param set simple.

## Components

### 1. `extension/src/table_function.rs`
- **Remove** the STAC short-circuit (lines 101-106); let `geozarr_core` classify the source and emit precise errors (a `/search`/ItemCollection URL now yields a clear "not yet supported" error from `geozarr_core`, not a blanket block).
- Register a new fixed named parameter `asset` (VARCHAR) alongside `lat_min`/… in the bind schema.
- Read the `asset` named parameter (Option<String>) and call `ZarrDataset::open_with_asset(&path, asset.as_deref())` instead of `::open(&path)`.
- The `{name}_min`/`{name}_max` bounds loop is unchanged — the selected asset's dims (`lat`/`lon` for EPSG:4326) drive pushdown exactly as for a standalone COG.

### 2. `geozarr_core/src/dataset.rs`
- Add `pub fn open_with_asset(path: &str, asset: Option<&str>) -> Result<Self, Box<dyn std::error::Error>>`.
- `open` delegates: `Self::open_with_asset(path, None)`.
- In `open_with_asset`: after `resolve_sync_store`, decide the array path:
  - Probe the store for a root array vs group: attempt to read `.zarray` (root) — if present, root path `"/"` (normal Zarr/COG; ignore `asset`).
  - Else read `.zmetadata`/`.zgroup`; collect asset names from metadata keys of the form `"{name}/.zarray"`.
    - `asset` given and present → open `"/{asset}"`.
    - `asset` absent and exactly one asset → open that one.
    - `asset` absent and multiple → `Err` listing names: `"STAC Item has multiple assets; choose one with asset := '<name>'. Available: a, b, c"`.
    - `asset` given but missing → `Err` listing available names.
  - `Array::open(store, &array_path)` and continue exactly as today (schema, dims, transform).

### 3. `geozarr_core/src/store.rs`
- **Local STAC Item JSON:** in the local-file branch (non-`.tif`/`.zarr`), attempt to parse the file contents as JSON; if `stac_version` present and `type=="Feature"`, build a `VirtualStacStore` whose children are local COGs whose hrefs are resolved **relative to the JSON file's parent directory** (absolute hrefs used as-is), via the Fs operator (reusing the COG local-header clamp from #126).
- **Clear errors for unsupported STAC:** if `type=="FeatureCollection"` (ItemCollection / `/search`), return `Err("STAC ItemCollection / search results are not yet supported (single Items only)")` — in both the local and HTTP branches.
- HTTP single-Item path retained (no behavior change beyond the FeatureCollection error).
- Relative-href resolution is shared between local and HTTP where practical; the local case resolves against the filesystem directory.

### 4. `geozarr_core/src/virtual_stac_store.rs`
- When synthesizing the group `.zmetadata`, also copy each child's `.zattrs` under `"{name}/.zattrs"` (belt-and-suspenders so child CRS/affine are visible via consolidated metadata, not only via direct `get`).
- Implement `ListableStorageTraits`: `list()` returns the group key set (`.zgroup`, `.zmetadata`, and per-child `.zarray`/`.zattrs`); `list_dir`/`list_prefix` return the appropriate child entries; `size_prefix` returns 0. No method should return `Err` for normal discovery.

### 5. Fixtures
- Commit `geozarr_core/tests/fixtures/stac_item.json`: a minimal valid STAC Item (`type:"Feature"`, `stac_version`, `geometry`/`bbox`, `properties.datetime`) with an `assets` map referencing the two existing COG fixtures via **relative** hrefs (e.g. `band_uncompressed` → `./cog_int16_uncompressed.tif`, `band_deflate` → `./cog_int16_deflate.tif`), each with a COG media type.
- No new binary fixtures (reuse the committed COGs). A short comment in the JSON documents its purpose.

### 6. Tests
- **`geozarr_core` offline e2e** (new, e.g. `tests/stac_e2e.rs`), using the local fixture:
  - `open_with_asset(stac_item.json, Some("band_uncompressed"))` → georeferenced array equal to opening that COG directly (dims `lat`/`lon`, `spatial_transform` present, Int16 dtype).
  - multiple assets + `None` → error mentioning both asset names.
  - unknown asset name → error listing available names.
  - a `FeatureCollection` fixture (or inline JSON) → "not yet supported" error.
- **Unit tests** in `virtual_stac_store.rs`: child-key routing (`band/.zarray`, `band/.zattrs`, `band/0.0`), `list()` returns child keys (not `Err`), and `.zattrs` appears in group `.zmetadata`.
- **Extension-level test** (mirrors the COG e2e style): `read_geo('<local stac_item.json>', asset := 'band_uncompressed')` returns rows with the georeferenced coordinates; verify the `asset` param is wired and the dispatch no longer short-circuits. Set `GEOZARR_ALLOW_PATH` as the COG e2e does.
- The existing live-network `test_stac_fallback` → annotate `#[ignore]` with a comment (network-dependent), keeping it runnable on demand.

### 7. Docs
- `docs/docs/engineering/cog_virtualization.mdx`: rewrite the STAC section from "planned / not wired to SQL" to first-class for **single Items** with **asset selection**, supporting local and HTTP, and explicitly list ItemCollection/`/search`/time-stacking and asset-band-stacking as not yet supported.
- `docs/docs/usage/sql_read_geo.md`: document the `asset` named parameter and that a STAC Item URL/path is a supported source (single Item; pick an asset), with the same honest limits.
- Docs build stays green (`onBrokenLinks: 'throw'`).

## Accuracy & verification

- Offline e2e proves a local STAC Item resolves, the chosen asset is georeferenced (dims/CRS/affine/dtype match the underlying COG), and the selection error paths fire. No network in CI.
- Multi-band assets continue to fail loudly (inherited from #126: child `VirtualCogStore::new` is fallible) — composing them in a STAC group propagates the error, never silently misreads.
- `cargo test` (workspace), `cargo clippy` (touched files), and `cargo fmt --check` pass; the extension builds against the new `open_with_asset`. Docs build green. Path-aware CI runs the full Rust matrix (code change) + Docs Build.
- Honest scoping: only single-Item asset-selection is claimed; everything deferred is documented as not-yet-supported with a clear runtime error rather than a silent wrong result.

## Non-goals (deferred)

- ItemCollection / `FeatureCollection` / `/search` results and any **time-stacking** across items.
- Stacking multiple assets into a single multi-band/3D array (assets differ in dtype/resolution/grid).
- CRS reprojection (inherited COG limit: projected assets read in native CRS, `y`/`x` dims, no geographic pushdown).
- STAC Collection/Catalog traversal, STAC API pagination/auth.
- Refetch/caching optimizations and the per-chunk tokio-runtime efficiency wart (pre-existing, out of scope).
