# Design: First-Class COG Support in `read_geo`

- **Date:** 2026-06-07
- **Status:** Approved (design); implementation pending
- **Scope:** Make Cloud Optimized GeoTIFFs (`.tif`/`.tiff`) a first-class, georeferenced, type-correct source for the `read_geo` and `read_zarr_metadata` SQL functions. Rust workstream (`geozarr_core` + a docs follow-on). First of two sequenced sub-projects (COG now; **STAC** as a later spec built on this foundation).
- **Dependency / sequencing:** Decided in brainstorming — **COG first, then STAC**, because `VirtualStacStore` composes `VirtualCogStore` children, so a correct COG store is the foundation STAC will reuse.

## Context

COGs already reach the COG code path from SQL today: the `read_geo` short-circuit only blocks STAC substrings (`"/search"`/`"items"`) at `extension/src/table_function.rs:101-103`, and a `.tif` path is **not** matched, so it flows through `ZarrDataset::open` → `resolve_sync_store` (`.tif` detection at `geozarr_core/src/store.rs:144`) → `VirtualCogStore`. **No dispatch/short-circuit change is needed.** The STAC short-circuit stays in place (next workstream).

The problem is that `VirtualCogStore` synthesizes **incorrect/placeholder metadata**, so the reads it produces are wrong or meaningless (verified against source):

- **dtype hardcoded `<f4>` (Float32)** — `geozarr_core/src/virtual_store.rs:24,56`. An Int16 DEM or UInt8 RGB COG is read as garbage. *Data-corruption bug.*
- **fill_value hardcoded `NaN`** — same file.
- **No `_ARRAY_DIMENSIONS`** → dimensions fall back to `dim_0`/`dim_1` (`geozarr_core/src/dataset.rs:261`).
- **No CRS, no affine transform** → no coordinate columns; `compute_bounds` treats `lat_*`/`lon_*` as pixel-index bounds, so spatial pushdown silently does nothing.
- **No internal-compression handling** — tile bytes are handed to `zarrs` raw; a compressed COG would not decode.

This follows the just-completed docs expansion, which documented COG as "experimental / library-level." This workstream makes the docs' eventual claim true and flips that label.

## Decisions (from brainstorming)

| Decision | Choice |
|---|---|
| Sequencing | COG first (this spec); STAC later (separate spec). |
| Georeferencing depth | **Georeferenced in the file's native CRS** — correct dtype + fill_value, GeoTIFF affine → coordinate columns, CRS reported, bbox pushdown via the existing affine path. **No reprojection.** |
| Test fixture | **Commit a tiny generated GeoTIFF** (+ a rasterio generator script); tests assert exact coords/dtype/CRS/values offline. |
| Internal compression | **Uncompressed + Deflate**, decoded in the store layer; **typed error** on LZW/JPEG/WebP/other. |
| Multi-band | **Single-band only** (`SamplesPerPixel == 1`); typed error otherwise (bands belong to the STAC/bands work). |

## Architecture & integration

The key insight: rather than special-casing COG downstream, `VirtualCogStore` synthesizes the **same `geozarr` metadata a real GeoZarr array would carry**, so the existing, already-correct machinery lights up with no changes:

- The **affine-dimension path** in `compute_bounds` / coordinate generation (`geozarr_core/src/dataset.rs:190-229`, `coordinates.rs:4-8` — `value = translation + index × scale`) produces coordinate columns and prunes chunks.
- **CRS reporting** via `resolved_crs()` (added in docs workstream B) reads `geozarr.crs` (flat) or `geozarr.spatial_reference.crs`.

So the synthesized `.zattrs` for the virtual array carries `_ARRAY_DIMENSIONS`, a `geozarr` spatial transform (scale + translation), and a `crs` string — and `read_geo`/`read_zarr_metadata` work unchanged.

## Components

### 1. `geozarr_core/src/cog.rs` — extend `CogMetadata` + the IFD parser
Parse the tags currently ignored, populating a richer `CogMetadata`:
- **dtype:** `BitsPerSample`(258) + `SampleFormat`(339) → a dtype enum/Zarr dtype string. Supported: unsigned int 8/16/32, signed int 8/16/32, float 32/64. Unsupported bit-depths/sample-formats → typed error.
- **fill_value:** `GDAL_NODATA`(42113) ASCII tag parsed to a number when present; else no fill.
- **compression:** `Compression`(259) → `CogCompression` enum `{ None, Deflate, Unsupported(code) }`.
- **bands:** `SamplesPerPixel`(277); a value `> 1` → typed error ("multi-band COGs not yet supported").
- **georef:** `ModelPixelScale`(33550) + `ModelTiepoint`(33922), or `ModelTransformation`(34264) → an affine (scale + translation per axis, with the conventional negative y-scale for north-up). `GeoKeyDirectory`(34735) → EPSG code from `ProjectedCSTypeGeoKey`(3072) or `GeographicTypeGeoKey`(2048); absent/complex → CRS unknown (affine still applied).
- Keep existing tile-geometry parsing (256/257/322/323/324/325).

### 2. `geozarr_core/src/virtual_store.rs` — synthesize honest metadata + decode tiles
- `.zarray`: real `dtype` and `fill_value` from `CogMetadata` (no more hardcoded `<f4>`/`NaN`); existing shape/chunks.
- `.zattrs` (and the `.zmetadata` aggregate): `_ARRAY_DIMENSIONS` (see §4 for the exact names), plus a `geozarr` block with the affine `spatial_transform` (scale/translation) and `crs` (`"EPSG:n"`) when known.
- `get()` for a tile chunk: if `Deflate`, inflate the tile bytes (zlib) before returning so `zarrs` receives the decoded array bytes; if `None`, return as today; if `Unsupported`, return a typed error surfaced to SQL.

### 3. Test fixture + generator
- `scripts/generate_cog_fixture.py` — rasterio script producing tiny georeferenced COGs with **known** CRS (e.g. EPSG:4326), affine, and values:
  - an **uncompressed Float32** COG,
  - a **Deflate-compressed** COG,
  - a **non-float dtype** (e.g. Int16) COG to prove the dtype fix.
- Committed fixtures under `geozarr_core/tests/fixtures/` (each a few KB). A short README/docstring documents how to regenerate.

### 4. Dimension naming & bbox binding (resolved at plan time)
The exact mapping of `read_geo`'s `lat_*`/`lon_*`/`time_*` params to the COG's two spatial dimensions (by dimension **name** vs **axis order**) will be verified against `compute_bounds` during implementation, and the synthesized `_ARRAY_DIMENSIONS` named accordingly (geographic `lat`/`lon` when the CRS is geographic; otherwise `y`/`x`). The fixture test pins the resulting behavior.

## Tests

- **`geozarr_core` unit tests:** tag parsing (dtype from BitsPerSample/SampleFormat; nodata; compression enum; SamplesPerPixel guard; affine from ModelPixelScale+ModelTiepoint and from ModelTransformation; EPSG from GeoKeyDirectory); Deflate tile decode; multi-band and unsupported-compression error paths.
- **End-to-end (extension or `geozarr_core` integration):** against the committed fixtures —
  - `read_zarr_metadata(fixture)` reports the real `data_type`, chunk shape, and `crs` (`EPSG:4326`).
  - `read_geo(fixture)` returns coordinate columns with the **georeferenced** values implied by the affine (not pixel indices), the correct value dtype, and the right sample value at a known cell.
  - A bbox-constrained `read_geo(fixture, lat_min:=…, lat_max:=…, lon_min:=…, lon_max:=…)` returns only the in-box cells (proves pushdown prunes).
  - The Deflate fixture yields identical values to the uncompressed one.
- All tests run **offline** (no live endpoints); the existing live-network Sentinel-2 STAC test is untouched (STAC is the next workstream). Full workspace CI stays green (`cargo test`, `cargo clippy -- -D warnings`, `cargo fmt --check`).

## Docs follow-on (in this workstream)

- `docs/docs/engineering/cog_virtualization.mdx`: replace the "experimental" admonition with first-class status; document the honest limits (single-band; uncompressed/Deflate only; native-CRS axes; no reprojection). Keep the STAC section as "planned / not wired to SQL."
- `docs/docs/usage/sql_read_geo.md`: mark COG (`.tif`/`.tiff`) as a supported `read_geo` source, with the same limits noted.
- Docs build stays green (`onBrokenLinks: 'throw'`); this triggers both the Rust matrix (code change) and the Docs Build in path-aware CI.

## Non-goals (deferred)

- **STAC via SQL** — the next sub-project; the STAC short-circuit and the live STAC test are untouched here.
- **Multi-band COGs**, **LZW/JPEG/WebP** internal compression, and **CRS reprojection** to lat/lon (projected COGs are queried in native-CRS axis units).
- Async-runtime efficiency (a new tokio runtime is created per chunk read, `virtual_store.rs:85-91`): a known wart; an optional low-risk improvement may be folded in by reusing a shared runtime, but correctness is the priority and a refactor is not required by this spec.
- Any change to the `read_geo` dispatch / short-circuit, or to Zarr/COG-unrelated code.
