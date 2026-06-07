# Polygons Workflow Guide Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A new Guides page demonstrating multi-polygon spatial subsetting with `eider extract`, a per-polygon `MAX` aggregation (one row per polygon), and the masked heatmap — from real captured output — plus a CLI regression test locking the multi-polygon behavior.

**Architecture:** Two committed multi-polygon GeoJSON fixtures (one for the guide under `scripts/`, one for the test under `cli/tests/fixtures/`), a new `cli/tests` regression test characterizing union + per-polygon aggregation, and a new `docs/docs/usage/guide_polygons.md` built from output captured by running the real CLI over the generated sample. Sidebar wiring. No production code change — extract already supports multiple polygons (union via `ST_Contains` over `ST_Read` rows) and carries feature properties through as columns.

**Tech Stack:** Rust CLI (`eider`), DuckDB `spatial` (`ST_Read`/`ST_Contains`), `assert_cmd`/`duckdb` test harness, Docusaurus.

---

## Conventions & prerequisites

Repo root `/Users/danielfisher/repos/zarrduck`, branch `docs/polygons-guide` (off `main`). Conventional Commits; every commit `--no-gpg-sign` ending with:
`Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>`
Pre-commit runs fmt/clippy/whitespace. Never `git add -A`; stage only named files.

Build + sample for live capture / tests:
```bash
cargo build -p eider && export PATH="$PWD/target/debug:$PATH"
cargo duckdb-ext build -o target/debug/eider.duckdb_extension -d v1.5.2 -- --no-default-features --features loadable-extension
python3 scripts/generate_demo_data.py   # writes ./climate_data.zarr if missing
```

### Established source facts (don't re-derive)
- `eider extract <zarr> <geojson> --out <db> --yes` builds `extracted_data` via (`cli/src/commands/extract.rs`):
  ```sql
  CREATE OR REPLACE TABLE extracted_data AS
  SELECT z.*, v.* EXCLUDE (geom)
  FROM read_geo(?, lon_min=?, lat_min=?, lon_max=?, lat_max=?) z, ST_Read(?) v
  WHERE ST_Contains(v.geom, ST_Point(z.lon, z.lat))
  ```
  → multiple features = union; each feature's `name` property becomes a `name` column; cells outside all polygons are absent. `air_temperature` is `(time, lat, lon)` so `extracted_data` has many rows per cell (one per time) plus `name`.
- Sample grid: lat −90..90 (73 pts, 2.5°), lon −180..177.5 (144 pts, 2.5°), `air_temperature` float32.
- Test harness (`cli/tests/common/mod.rs`): `climate_zarr()`, `repo_root()`, `fixture_path(name)` (→ `cli/tests/fixtures/`), `find_geozarr_ext()` (returns None when the extension isn't built — tests must skip-guard), `eider(&dir)` (Command builder). Existing pattern: `extract` tests set `.env("GEOZARR_ALLOW_PATH", repo_root())` and use `air_temp_uri()` = `climate_zarr().join("air_temperature")`.

## File structure
- Create: `scripts/demo_polygons.geojson` (guide fixture), `cli/tests/fixtures/multi_polygon.geojson` (test fixture) — identical 2 named non-rectangular polygons (Task 1)
- Create: `cli/tests/extract_multipolygon_test.rs` (Task 2)
- Create: `docs/docs/usage/guide_polygons.md` (Task 3)
- Modify: `docs/sidebars.ts` (Task 4)
- Verify: Task 5

The two polygons (triangles, clearly non-rectangular, each containing several 2.5° cells), used by BOTH fixtures:
- **`west`** — triangle: `[[-130,30],[-100,30],[-115,55],[-130,30]]`
- **`east`** — triangle: `[[60,10],[110,10],[85,40],[60,10]]`

---

## Task 1: Committed multi-polygon fixtures

**Files:** Create `scripts/demo_polygons.geojson`, `cli/tests/fixtures/multi_polygon.geojson` (identical content).

- [ ] **Step 1: Write the GeoJSON (both files, same content)**

```json
{
  "type": "FeatureCollection",
  "features": [
    {
      "type": "Feature",
      "properties": { "name": "west" },
      "geometry": {
        "type": "Polygon",
        "coordinates": [[[-130.0, 30.0], [-100.0, 30.0], [-115.0, 55.0], [-130.0, 30.0]]]
      }
    },
    {
      "type": "Feature",
      "properties": { "name": "east" },
      "geometry": {
        "type": "Polygon",
        "coordinates": [[[60.0, 10.0], [110.0, 10.0], [85.0, 40.0], [60.0, 10.0]]]
      }
    }
  ]
}
```

- [ ] **Step 2: Sanity-check each polygon contains cells**

Build (per prerequisites), then:
```bash
eider extract climate_data.zarr/air_temperature scripts/demo_polygons.geojson --out /tmp/poly_check.duckdb --yes
duckdb /tmp/poly_check.duckdb -c "SELECT name, COUNT(*) FROM extracted_data GROUP BY name ORDER BY name;"
```
Expected: two rows, `east` and `west`, each count > 0. If a polygon is empty (no cells), widen/move that triangle (still non-rectangular) until both contain cells, and update BOTH fixture files identically.

- [ ] **Step 3: Commit**

```bash
git add scripts/demo_polygons.geojson cli/tests/fixtures/multi_polygon.geojson
git commit --no-gpg-sign -m "test: add multi-polygon (named, non-rectangular) GeoJSON fixtures

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 2: Multi-polygon extract regression test

**Files:** Create `cli/tests/extract_multipolygon_test.rs`.

- [ ] **Step 1: Write the test (characterizes existing behavior)**

```rust
mod common;
use common::*;

fn air_temp_uri() -> String {
    climate_zarr()
        .join("air_temperature")
        .to_string_lossy()
        .into_owned()
}

#[test]
fn extract_unions_named_polygons_and_groups_per_polygon() {
    if find_geozarr_ext().is_none() {
        eprintln!("skipping: eider.duckdb_extension not built (expected on Windows)");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("multi.duckdb");

    eider(&dir)
        .env("GEOZARR_ALLOW_PATH", repo_root())
        .arg("extract")
        .arg(air_temp_uri())
        .arg(fixture_path("multi_polygon.geojson"))
        .args(["--out", out.to_str().unwrap()])
        .arg("--yes")
        .arg("--output=json")
        .assert()
        .success();

    let conn = duckdb::Connection::open(&out).unwrap();
    // One row per polygon: union extract carries each feature's `name` through.
    let mut stmt = conn
        .prepare(
            "SELECT name, COUNT(*) AS n, MAX(value)::DOUBLE AS mx \
             FROM extracted_data GROUP BY name ORDER BY name",
        )
        .unwrap();
    let rows: Vec<(String, i64, f64)> = stmt
        .query_map([], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?, r.get::<_, f64>(2)?))
        })
        .unwrap()
        .map(|r| r.unwrap())
        .collect();

    assert_eq!(
        rows.iter().map(|(n, _, _)| n.as_str()).collect::<Vec<_>>(),
        vec!["east", "west"],
        "expected exactly one row per polygon (east, west), got {rows:?}"
    );
    for (name, n, mx) in &rows {
        assert!(*n > 0, "polygon {name} should contain extracted cells");
        assert!(mx.is_finite(), "polygon {name} max should be finite");
    }
}
```

- [ ] **Step 2: Run the test**

Run: `cargo test -p eider --test extract_multipolygon_test`
Expected: PASS (locks the union + per-polygon-aggregation behavior). If it FAILS with fewer/more than 2 rows or a missing `name` column, multi-polygon union or property-passthrough is not working as the spec assumed — STOP and report (it would mean a real product gap, not a test bug).

- [ ] **Step 3: Commit**

```bash
git add cli/tests/extract_multipolygon_test.rs
git commit --no-gpg-sign -m "test: lock multi-polygon union extract and per-polygon aggregation

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 3: The guide page (real captured output)

**Files:** Create `docs/docs/usage/guide_polygons.md`.

- [ ] **Step 1: Capture the real command output**

With the CLI built and the sample generated, run and SAVE the exact output of each:
```bash
rm -f /tmp/poly_analysis.duckdb
eider extract climate_data.zarr/air_temperature scripts/demo_polygons.geojson --out /tmp/poly_analysis.duckdb --yes
duckdb /tmp/poly_analysis.duckdb -c "SELECT name, MAX(value) AS max_temp FROM extracted_data GROUP BY name ORDER BY name;"
eider plot /tmp/poly_analysis.duckdb --plot-type heatmap
```
Keep the verbatim outputs to paste into the page (the per-polygon table and the ASCII heatmap). Use the exact `eider shell` form in the page (the reader-facing flow), but you may capture the aggregation via `duckdb` for convenience — the SQL is identical.

- [ ] **Step 2: Write the page (paste the captured output into the marked blocks)**

Create `docs/docs/usage/guide_polygons.md`:

````markdown
---
sidebar_position: 2
---

# Extracting with polygons

`eider extract` pulls the grid cells that fall inside a vector boundary. The
boundary can hold **several polygons at once** — a cell is kept if it lies in
**any** of them (union) — and each polygon's properties travel through to the
output, so you can aggregate **per polygon**.

This guide uses the sample dataset (see [Installation](./installation.md)):

```bash
python3 scripts/generate_demo_data.py   # writes ./climate_data.zarr
```

## 1. Define multiple polygons

`scripts/demo_polygons.geojson` is a `FeatureCollection` with two named,
non-rectangular regions:

```json
{
  "type": "FeatureCollection",
  "features": [
    { "type": "Feature", "properties": { "name": "west" },
      "geometry": { "type": "Polygon",
        "coordinates": [[[-130,30],[-100,30],[-115,55],[-130,30]]] } },
    { "type": "Feature", "properties": { "name": "east" },
      "geometry": { "type": "Polygon",
        "coordinates": [[[60,10],[110,10],[85,40],[60,10]]] } }
  ]
}
```

The `name` property identifies each polygon in the results.

## 2. Extract the cells inside the polygons

```bash
eider extract climate_data.zarr/air_temperature scripts/demo_polygons.geojson \
  --out analysis.duckdb --yes
```

<!-- PASTE the real captured success output here -->

This writes an `extracted_data` table containing only the cells inside `west`
or `east`, with a `name` column tagging which polygon each cell came from. See
[`eider extract`](./cli_extract.md).

## 3. Aggregate per polygon

Group by `name` to get one row per polygon — here, the maximum value in each:

```sql
SELECT name, MAX(value) AS max_temp
FROM extracted_data
GROUP BY name
ORDER BY name;
```

Run it in the SQL shell (`eider shell analysis.duckdb`):

<!-- PASTE the real captured result table here (two rows: east, west) -->

See [`eider shell`](./cli_shell.md).

## 4. See the mask

Render the extracted cells as a heatmap — the populated cells trace the polygon
shapes; everything outside is absent:

```bash
eider plot analysis.duckdb --plot-type heatmap
```

<!-- PASTE the real captured ASCII heatmap here -->

See [`eider plot`](./cli_plot.md).

## Next steps

- [End-to-end analysis workflow](./guide_workflow.md)
- [SQL Reference](./sql_reference.md) — query `extracted_data` directly.
````

Replace each `<!-- PASTE ... -->` with the verbatim output captured in Step 1. If the real output reveals different behavior (e.g. a column named differently), correct the prose to match reality.

- [ ] **Step 3: Commit**

```bash
git add docs/docs/usage/guide_polygons.md
git commit --no-gpg-sign -m "docs: add polygons workflow guide (multi-polygon extract + per-polygon max)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 4: Wire the sidebar

**Files:** Modify `docs/sidebars.ts`.

- [ ] **Step 1: Add the page to the Guides category**

In the `Guides` category `items`, add `'usage/guide_polygons'` after `'usage/guide_workflow'`:
```typescript
      items: [
        'usage/guide_workflow',
        'usage/guide_polygons',
        'usage/guide_cloud',
        'usage/exporting',
      ],
```
(Confirm `guide_polygons`'s `sidebar_position: 2` is consistent with this slot; adjust the frontmatter number if the existing pages use explicit positions that would conflict — match the neighbors.)

- [ ] **Step 2: Commit**

```bash
git add docs/sidebars.ts
git commit --no-gpg-sign -m "docs: wire polygons guide into the Guides sidebar

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 5: Verification

**Files:** none.

- [ ] **Step 1: Test suite**

Run: `cargo test -p eider --test extract_multipolygon_test` → PASS. Then `cargo test` (whole workspace) → 0 failures (nothing else affected). `cargo fmt --check` clean.

- [ ] **Step 2: Docs build (broken links + new page)**

Run: `cd docs && (test -d node_modules || npm ci) && npm run build 2>&1 | tail -6`
Expected: `[SUCCESS]`, no broken links. The guide links to `installation.md`, `cli_extract.md`, `cli_shell.md`, `cli_plot.md`, `guide_workflow.md`, `sql_reference.md` — all exist on `main`.

- [ ] **Step 3: No placeholder output remains**

Run: `grep -n "PASTE" docs/docs/usage/guide_polygons.md || echo clean`
Expected: `clean` (all captured-output blocks filled with real output).

- [ ] **Step 4: Scope**

Run: `git diff --name-status main..HEAD | grep -v superpowers`
Expected exactly:
```
A	cli/tests/extract_multipolygon_test.rs
A	cli/tests/fixtures/multi_polygon.geojson
A	docs/docs/usage/guide_polygons.md
M	docs/sidebars.ts
A	scripts/demo_polygons.geojson
```
No production code changes (extract is documented/tested, not modified).

---

## Self-review notes (author checklist, already applied)

- **Spec coverage:** multiple non-rectangular named polygons fixture (Task 1) ✓; multi-polygon union + per-polygon `MAX` one-row-per-polygon test (Task 2) ✓; new Guides page from real captured output incl. per-polygon table + masked heatmap (Task 3) ✓; sidebar wiring (Task 4) ✓; build + no-placeholder + scope gate (Task 5) ✓; regression test added per decision ✓.
- **Grounding:** test mirrors the real `cli/tests/extract_test.rs` harness (`find_geozarr_ext` skip-guard, `GEOZARR_ALLOW_PATH`, `air_temp_uri`, `fixture_path`); `MAX(value)::DOUBLE` cast avoids a float32→f64 `get` mismatch; polygons sit inside the real −180..180 / −90..90 grid and are verified to contain cells (Task 1 Step 2).
- **Placeholders:** the guide's three output blocks are explicit live-capture steps (Task 3 Step 1 produces them; Task 5 Step 3 greps for any unfilled `PASTE`), the established capture pattern — not vague placeholders. Polygon coordinates are concrete; the only contingency is widening a triangle if it contains no cells (Task 1 Step 2).
- **Non-goals honored:** no GIF; no extract behavior change; no MultiPolygon/holes focus; no STAC/time-stacking.
