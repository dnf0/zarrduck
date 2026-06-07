# Design: Docs Workstream B — SQL / Extension Reference

- **Date:** 2026-06-07
- **Status:** Approved (design); implementation pending
- **Scope:** The DuckDB-extension SQL reference section of the docs site, plus two small extension code fixes that the reference depends on for accuracy. Second of five sequenced documentation workstreams (A done; order A→B→C→D→E).

## Context

Workstream A established the docs information architecture, including a "SQL Reference" sidebar category currently holding a single thin, stale `sql_reference.md` (it documents `read_zarr` — renamed to `read_geo` — and a string `time_min`/`time_max` example, when those parameters are actually `Double`). The extension registers three table functions:

- `read_geo(uri, …)` — positional URI; named params `lat_min`, `lat_max`, `lon_min`, `lon_max`, `time_min`, `time_max` (all `Double`) and `pins` (`Varchar`); dynamic output schema (coordinate columns + a value column). Unified over Zarr / STAC / COG sources (post-#114).
- `plan_read_geo(uri, …)` — same parameters; outputs `total_chunks`, `total_bytes` (`Bigint`).
- `read_zarr_metadata(uri)` — outputs `array_shape`, `chunk_shape`, `data_type`, `crs`.

Two output warts currently undermine a clean reference (both verified live):
- `chunk_shape` renders as Rust debug `Some(ChunkShape([12, 73, 144]))` rather than `[12, 73, 144]`.
- `crs` returns `UNKNOWN` even when the array has a CRS, because the metadata parser expects a flat `geozarr.crs` while our own fixture (and the GeoZarr spec) nests it at `geozarr.spatial_reference.crs`.

## Decisions (from brainstorming)

| Decision | Choice |
|---|---|
| Structure | SQL Reference landing page + one page per function |
| Output warts | Fix both in this workstream (docs + small extension code changes) |
| Examples | Local `climate_data.zarr` sample; output verified against the post-fix extension |

## Page structure

All pages flat under `docs/docs/usage/` (no file moves; regroup in the existing "SQL Reference" sidebar category). Sidebar order:

1. `usage/sql_reference` — **landing** (existing file, rewritten)
2. `usage/sql_read_geo` — new
3. `usage/sql_read_zarr_metadata` — new
4. `usage/sql_plan_read_geo` — new

The `sidebars.ts` "SQL Reference" category is updated to list these four in order.

## Page content

### `sql_reference.md` (landing / shared concepts)
- Loading the extension (cross-link to Installation; `duckdb -unsigned` + absolute `LOAD`).
- Function catalog: a table of the three functions with a one-line purpose and a link to each page.
- **Parameters** shared by `read_geo`/`plan_read_geo`: `lat_min/lat_max/lon_min/lon_max/time_min/time_max` (`DOUBLE`) and `pins` (`VARCHAR`), with the `name := value` named-argument syntax.
- **Spatial & temporal pushdown**: how bounding-box / time bounds prune whole chunks before fetching.
- **`pins`**: pinning non-spatial dimensions to fixed indices (syntax + example).
- **Supported data types**: the Zarr primitive → DuckDB type mapping (verified from `geozarr_core::types`), and `fill_value` → SQL `NULL`.
- **Source URIs**: local paths, `s3://`, `http(s)://`; and `read_geo`'s accepted source kinds (Zarr array, COG, STAC) — only those verified working are documented as supported; anything not working is omitted or explicitly marked experimental.

### `sql_read_geo.md`
- Signature and the positional URI argument.
- Full named-parameter table (name, type, meaning, default/optional).
- Output schema: coordinate columns + the value column (document the actual value-column naming, verified), and `fill_value` → `NULL`.
- Worked examples (each verified): basic select; bounding-box pushdown; temporal bounds; `pins`. Source-type examples (Zarr; COG and/or STAC only if verified working).

### `sql_read_zarr_metadata.md`
- Signature; the four output columns (`array_shape`, `chunk_shape`, `data_type`, `crs`) with descriptions.
- Example showing the cleaned-up output (`chunk_shape` = `[12, 73, 144]`, `crs` = `EPSG:4326`).

### `sql_plan_read_geo.md`
- Signature; outputs `total_chunks`, `total_bytes` (`BIGINT`).
- Purpose: estimate the cost / dry-run a query before a heavy read; takes the same pushdown params so the estimate reflects pruning.
- Example.

## Extension fixes (required for an accurate reference)

1. **`chunk_shape` formatting** — `extension/src/metadata_vtab.rs`: emit `[12, 73, 144]` rather than the `Some(ChunkShape([...]))` debug form. Add a co-located test asserting the rendered string format.
2. **`crs` parsing** — `geozarr_core` metadata deserialization: also read the nested `geozarr.spatial_reference.crs` so a CRS present under the GeoZarr spec's `spatial_reference` is surfaced (returns `EPSG:4326` for the sample) instead of `UNKNOWN`; keep the flat `geozarr.crs` path working if present. Add a test against the fixture metadata.

Both fixes are behavior-preserving elsewhere, ship with tests, and pass the full Rust CI (`cargo test`, `cargo clippy -- -D warnings`, `cargo fmt --check`) in addition to the docs build.

## Verification

- The two extension fixes are unit-tested and the workspace test suite stays green.
- Every SQL block in the reference is run against the freshly-built **post-fix** extension over the local `climate_data.zarr` sample; output blocks reflect the corrected output. The `read_geo` output schema, exact value-column name, and which `read_geo` source kinds (Zarr/COG/STAC) actually work are confirmed at implementation time and documented as-found — no unverified/aspirational syntax (e.g. the old string-typed `time_min`).
- `cd docs && npm run build` succeeds with no broken links (Docusaurus `onBrokenLinks: 'throw'`). New pages appear under "SQL Reference"; intra-section links resolve; links to not-yet-written sections (CLI Reference, Guides) remain plain text.

## Non-goals (deferred)

- CLI reference (C); guides/tutorials including `COPY … TO ZARR` export (D); engineering deep-dives (E).
- Any new read_geo / STAC / COG *capability* — B documents existing behavior and fixes output rendering only.
- File moves/renames of existing pages.
