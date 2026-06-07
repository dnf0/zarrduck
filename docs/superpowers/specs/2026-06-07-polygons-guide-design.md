# Design: Polygons Workflow Guide (multi-polygon extract + per-polygon max)

- **Date:** 2026-06-07
- **Status:** Approved (design); implementation pending
- **Scope:** A new Guides docs page demonstrating multi-polygon spatial subsetting with `eider extract`, a per-polygon `MAX` aggregation (one row per polygon), and the masked heatmap — built from real captured output. Plus a small CLI regression test locking the multi-polygon behavior. Docs + a test (no production code change expected).

## Context

`eider extract <zarr> <geojson>` extracts the grid cells that fall inside a vector boundary. Verified mechanism (`cli/src/commands/extract.rs`): it joins `read_geo(...)` against DuckDB's `ST_Read(geojson)` (the `spatial` extension) and keeps cells via `WHERE ST_Contains(v.geom, ST_Point(z.lon, z.lat))`, selecting `z.*, v.* EXCLUDE (geom)`. Consequences this guide relies on (all confirmed in source):

- **Multiple polygons work as-is:** a `FeatureCollection` with N polygon features yields N rows in `v`; a cell is kept if it is inside **any** feature (union semantics).
- **Feature properties carry through as columns** (`v.* EXCLUDE (geom)`): if each polygon has a `name` property, `extracted_data` has a `name` column → `GROUP BY name` gives **one row per polygon**.
- **Cells outside all polygons are absent** from `extracted_data` (filtered by the `WHERE`), not present-as-NULL.
- A cell inside two overlapping polygons appears once per matching polygon (one `(cell, feature)` row each) — correct for per-polygon aggregation.

Gap found: there is **no test** exercising a multi-feature/multi-polygon extract (existing fixtures `scripts/demo_region.geojson` and `cli/tests/fixtures/polygon.geojson` are single-feature rectangles). This guide's central claim (per-polygon aggregation over multiple polygons) is therefore locked with a new regression test.

## Decisions (from brainstorming)

| Decision | Choice |
|---|---|
| Focus | Spatial masking with **multiple, genuinely non-rectangular polygons** (the masking visual must be obvious vs the existing rectangle). |
| Per-polygon aggregation | `MAX(value)` grouped by polygon → **one row per polygon**. |
| Placement | **New** Guides page `docs/docs/usage/guide_polygons.md`, wired into the Guides sidebar. |
| Medium | **Written guide with real captured output** (no GIF): commands + the actual per-polygon result table + the ASCII heatmap tracing the polygons. |
| Test | **Add** a CLI multi-polygon extract regression test. |

## Components

### 1. Fixture: `scripts/demo_polygons.geojson`
A `FeatureCollection` of **2–3 non-rectangular, named polygons** over distinct regions of the global `climate_data.zarr` sample (2.5° grid), each with a `"name"` property (e.g. `"west"`, `"east"`). Shapes are triangles/diamonds (not axis-aligned rectangles) and each is large enough (~15–30° span) to contain several grid cells. Exact coordinates are finalized at implementation time against the real sample grid so each polygon demonstrably contains cells (verified by the live extract run). Longitudes use the −180..180 convention (as `demo_region.geojson` does; `read_geo` handles normalization).

### 2. Guide: `docs/docs/usage/guide_polygons.md`
Shape: Goal → Prerequisites (link to Installation; generate the sample) → numbered steps with **real** commands/output → next steps/cross-links. Steps:
1. **The polygons** — show the multi-feature GeoJSON (named polygons); explain union semantics and that properties become columns.
2. **Extract** — `eider extract climate_data.zarr/air_temperature scripts/demo_polygons.geojson --out analysis.duckdb --yes` (captured success output; mention `extracted_data` carries a `name` column).
3. **Per-polygon max** — in `eider shell analysis.duckdb`:
   ```sql
   SELECT name, MAX(value) AS max_temp
   FROM extracted_data
   GROUP BY name
   ORDER BY name;
   ```
   Captured output showing **one row per polygon** with its max.
4. **Masking visual** — `eider plot analysis.duckdb --plot-type heatmap`; captured ASCII heatmap in which the extracted cells trace the polygon shapes (outside cells absent).
5. **Next steps** — cross-link `cli_extract.md`, `cli_shell.md`, `cli_plot.md`, and `guide_workflow.md`.

If the real captured output differs from any drafted wording, the guide is corrected to match reality (document as-found; no invented output).

### 3. Sidebar: `docs/sidebars.ts`
Add `usage/guide_polygons` to the Guides category (after `guide_workflow`, before `guide_cloud`, or adjacent — order chosen for narrative; exact slot decided in the plan).

### 4. Regression test: `cli/tests/`
A test (e.g. `extract_multipolygon_test.rs`) using a committed 2-named-polygon fixture (`cli/tests/fixtures/multi_polygon.geojson`, or reuse `scripts/demo_polygons.geojson` if suitable) over the test Zarr the existing extract test uses. Asserts:
- extract succeeds and `extracted_data` exists with a `name` column;
- `SELECT name, COUNT(*) ... GROUP BY name` returns **exactly the number of polygons** (each region contributes cells);
- a per-polygon `MAX(value)` query returns one row per polygon with the expected maxima (values derived from the known test data / asserted as `>=` sanity bounds rather than brittle exact floats where the grid makes exact values fragile).
Mirror the harness of the existing `cli/tests/extract_test.rs` (fixture Zarr generation / sample, `assert_cmd`, temp dirs).

## Accuracy & verification

- Every command in the guide is run against the freshly built `eider` + generated `climate_data.zarr`; the per-polygon table and the heatmap are **captured from the real run** and pasted verbatim. Polygon coordinates are confirmed to contain cells.
- The regression test passes in `cargo test` and locks the union + per-polygon-aggregation behavior (currently untested).
- `cd docs && npm run build` succeeds with no broken links (`onBrokenLinks: 'throw'`); the new page appears under Guides.
- This is a code-touching change (test + fixtures), so path-aware CI runs the full Rust matrix + Docs Build.

## Non-goals (deferred)

- A recorded GIF (explicitly chosen against; written guide with captured output instead).
- STAC/time-stacking (separate paused sub-project).
- Polygon-with-holes / MultiPolygon-geometry specific documentation (the union-of-features path is the demo; `ST_Contains` handles holes/MultiPolygon but those aren't the focus and aren't separately tested here).
- Any change to extract's behavior — this documents and tests existing behavior only.
