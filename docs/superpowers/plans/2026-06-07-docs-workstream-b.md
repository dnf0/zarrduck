# Docs Workstream B (SQL Reference) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a complete, accurate SQL/extension reference (landing + one page per table function) and fix the two `read_zarr_metadata` output warts the reference depends on (`chunk_shape` debug formatting; nested-`crs` parsing).

**Architecture:** Two small, test-driven Rust fixes (`geozarr_core` metadata parsing + `extension` chunk_shape rendering), then four Docusaurus Markdown pages under `docs/docs/usage/` regrouped in the "SQL Reference" sidebar category. Examples are verified against the freshly-built post-fix extension over the local `climate_data.zarr` sample.

**Tech Stack:** Rust (geozarr_core, eider_extension), DuckDB v1.5.2 + the loadable extension, Docusaurus.

---

## Conventions & verified facts

Work from repo root `/Users/danielfisher/repos/zarrduck` on branch `docs/workstream-b-sql-reference` (do NOT commit to `main`). Conventional Commits, `--no-gpg-sign`, end messages with `Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>`.

Verified live this session:
- The extension registers `read_geo`, `plan_read_geo`, `read_zarr_metadata` (`extension/src/lib.rs`).
- `read_geo`/`plan_read_geo` named params (all `Double` except pins): `lat_min`, `lat_max`, `lon_min`, `lon_max`, `time_min`, `time_max`, and `pins` (`Varchar`). One positional arg: the URI. `plan_read_geo` outputs `total_chunks`, `total_bytes` (`Bigint`).
- `read_zarr_metadata` outputs `array_shape`, `chunk_shape`, `data_type`, `crs` (all `Varchar`). Current (pre-fix) output for `climate_data.zarr/air_temperature`: `[938, 73, 144]` | `Some(ChunkShape([12, 73, 144]))` | `Float32` | `UNKNOWN`.
- The sample fixture's GeoZarr attrs nest CRS at `geozarr.spatial_reference.crs = "EPSG:4326"`; `geozarr_core::metadata::GeoZarrMetadata` only has a flat `crs` field → parses to `None` → `crs` shows `UNKNOWN`.
- `zarrs::array::ChunkShape` derives `Deref`/`From` (it wraps `Vec<NonZeroU64>`): iterate with `.iter().map(|n| n.get())`; construct in tests with `ChunkShape::from(vec![NonZeroU64::new(12).unwrap(), ...])`.
- Build the loadable extension: `cargo duckdb-ext build -o target/debug/eider.duckdb_extension -d v1.5.2 -- --no-default-features --features loadable-extension`. Run SQL: `duckdb -unsigned -cmd "LOAD '$PWD/target/debug/eider.duckdb_extension'" -c "<sql>"`.
- Docs build: `cd docs && (test -d node_modules || npm ci) && npm run build` (Docusaurus `onBrokenLinks: 'throw'`).

## File structure

- Modify: `geozarr_core/src/metadata.rs` — add nested `spatial_reference.crs` support + `resolved_crs()`.
- Modify: `extension/src/metadata_vtab.rs` — use `resolved_crs()`; render `chunk_shape` cleanly via a tested helper.
- Modify: `docs/docs/usage/sql_reference.md` — rewrite as the SQL Reference landing.
- Create: `docs/docs/usage/sql_read_geo.md`, `sql_read_zarr_metadata.md`, `sql_plan_read_geo.md`.
- Modify: `docs/sidebars.ts` — "SQL Reference" category lists the four pages.

---

## Task 1: Fix nested `crs` parsing (geozarr_core)

**Files:** Modify `geozarr_core/src/metadata.rs`, then `extension/src/metadata_vtab.rs`.

- [ ] **Step 1: Add a failing test for nested CRS**

In `geozarr_core/src/metadata.rs`, inside `mod tests`, add:

```rust
    #[test]
    fn test_parse_crs_from_spatial_reference() {
        // GeoZarr spec / our fixtures nest CRS under spatial_reference.
        let attrs = json!({
            "geozarr": {
                "spatial_reference": { "crs": "EPSG:4326" },
                "spatial_transform": {
                    "scale": [1.0, -2.5, 2.5],
                    "translation": [0.0, 90.0, -180.0]
                }
            }
        });
        let meta = parse_geozarr_metadata(&attrs).unwrap();
        assert_eq!(meta.resolved_crs(), Some("EPSG:4326".to_string()));
    }

    #[test]
    fn test_resolved_crs_prefers_flat_then_nested() {
        let flat = json!({ "geozarr": { "crs": "EPSG:3857" } });
        assert_eq!(
            parse_geozarr_metadata(&flat).unwrap().resolved_crs(),
            Some("EPSG:3857".to_string())
        );
        let none = json!({ "geozarr": {} });
        assert_eq!(parse_geozarr_metadata(&none).unwrap().resolved_crs(), None);
    }
```

- [ ] **Step 2: Run to confirm failure**

Run: `cargo test -p geozarr_core metadata::tests::test_parse_crs_from_spatial_reference metadata::tests::test_resolved_crs_prefers_flat_then_nested`
Expected: FAIL — `no method named resolved_crs` (and `spatial_reference` not captured).

- [ ] **Step 3: Implement the nested field + resolver**

In `geozarr_core/src/metadata.rs`, add a `SpatialReference` struct, a `spatial_reference` field, and a `resolved_crs()` method. Replace the `SpatialTransform`/`GeoZarrMetadata` block with:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpatialTransform {
    pub scale: Vec<f64>,
    pub translation: Vec<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpatialReference {
    pub crs: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeoZarrMetadata {
    pub crs: Option<String>,
    #[serde(rename = "spatial_reference")]
    pub spatial_reference: Option<SpatialReference>,
    #[serde(rename = "spatial_transform")]
    pub transform: Option<SpatialTransform>,
}

impl GeoZarrMetadata {
    /// CRS resolved from the flat `crs` field, falling back to the nested
    /// `spatial_reference.crs` (the layout used by the GeoZarr spec).
    pub fn resolved_crs(&self) -> Option<String> {
        self.crs
            .clone()
            .or_else(|| self.spatial_reference.as_ref().and_then(|s| s.crs.clone()))
    }
}
```

- [ ] **Step 4: Run the new + existing metadata tests**

Run: `cargo test -p geozarr_core metadata::`
Expected: all PASS (the new two plus the pre-existing `test_parse_spatial_metadata`, `test_parse_geozarr_missing_crs`, `test_parse_geozarr_invalid_scale`, `test_parse_geozarr_empty`).

- [ ] **Step 5: Use `resolved_crs()` in the extension**

In `extension/src/metadata_vtab.rs`, in both the `ArrayMetadata::V2` and `ArrayMetadata::V3` branches, replace `if let Some(c) = geozarr.crs { crs = c; }` with:

```rust
                if let Some(c) = geozarr.resolved_crs() {
                    crs = c;
                }
```

- [ ] **Step 6: Build the extension crate to confirm it compiles**

Run: `cargo build -p eider_extension`
Expected: builds (default `bundled` feature).

- [ ] **Step 7: Commit**

```bash
git add geozarr_core/src/metadata.rs extension/src/metadata_vtab.rs
git commit --no-gpg-sign -m "fix: read nested geozarr.spatial_reference.crs in metadata

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 2: Clean `chunk_shape` rendering (extension)

**Files:** Modify `extension/src/metadata_vtab.rs`.

- [ ] **Step 1: Add a failing unit test for the formatter**

In `extension/src/metadata_vtab.rs`, add (creating a `#[cfg(test)] mod tests` if absent):

```rust
#[cfg(test)]
mod tests {
    use super::render_chunk_shape;
    use std::num::NonZeroU64;
    use zarrs::array::ChunkShape;

    fn nz(v: u64) -> NonZeroU64 {
        NonZeroU64::new(v).unwrap()
    }

    #[test]
    fn renders_chunk_shape_as_plain_list() {
        let cs = ChunkShape::from(vec![nz(12), nz(73), nz(144)]);
        assert_eq!(render_chunk_shape(Some(cs)), "[12, 73, 144]");
    }

    #[test]
    fn renders_missing_chunk_shape() {
        assert_eq!(render_chunk_shape(None), "unknown");
    }
}
```

- [ ] **Step 2: Run to confirm failure**

Run: `cargo test -p eider_extension metadata_vtab::tests`
Expected: FAIL — `cannot find function render_chunk_shape`.

- [ ] **Step 3: Add the formatter and use it**

In `extension/src/metadata_vtab.rs`, add the helper (module level):

```rust
/// Render a chunk shape as a plain `[d0, d1, …]` list, or `"unknown"` when absent.
pub(crate) fn render_chunk_shape(chunk_shape: Option<zarrs::array::ChunkShape>) -> String {
    match chunk_shape {
        Some(cs) => {
            let dims: Vec<u64> = cs.iter().map(|n| n.get()).collect();
            format!("{:?}", dims)
        }
        None => "unknown".to_string(),
    }
}
```

Then replace the existing `chunk_shape` computation:

```rust
        let chunk_shape = render_chunk_shape(
            array
                .chunk_grid()
                .chunk_shape(&vec![0; array.shape().len()], array.shape())
                .unwrap_or(None),
        );
```

(If `cargo build` reports that `chunk_shape(...)` returns a non-`Option` or the `ChunkShape` import path differs in zarrs 0.16.4, adjust the import/`unwrap_or(None)` to match — the formatter signature stays the same.)

- [ ] **Step 4: Run the formatter tests**

Run: `cargo test -p eider_extension metadata_vtab::tests`
Expected: both PASS.

- [ ] **Step 5: Commit**

```bash
git add extension/src/metadata_vtab.rs
git commit --no-gpg-sign -m "fix: render chunk_shape as a plain dimension list

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 3: Rebuild the extension and capture verified output

**Files:** none (produces the verified output strings used by Tasks 4–6).

- [ ] **Step 1: Build the loadable extension with the fixes**

Run: `cargo duckdb-ext build -o target/debug/eider.duckdb_extension -d v1.5.2 -- --no-default-features --features loadable-extension`
Expected: `Finished DuckDB Extension`. (If `climate_data.zarr` is missing: `python3 scripts/generate_demo_data.py`.)

- [ ] **Step 2: Confirm the metadata output is now clean**

Run: `duckdb -unsigned -cmd "LOAD '$PWD/target/debug/eider.duckdb_extension'" -c "SELECT array_shape, chunk_shape, data_type, crs FROM read_zarr_metadata('climate_data.zarr/air_temperature');"`
Expected: `[938, 73, 144]` | `[12, 73, 144]` | `Float32` | `EPSG:4326` (no `Some(ChunkShape(...))`, no `UNKNOWN`). Record this exact table for the docs.

- [ ] **Step 3: Capture the `read_geo` output schema and a result sample**

Run:
```bash
duckdb -unsigned -cmd "LOAD '$PWD/target/debug/eider.duckdb_extension'" -c "DESCRIBE SELECT * FROM read_geo('climate_data.zarr/air_temperature', lat_min := 30, lat_max := 50); SELECT * FROM read_geo('climate_data.zarr/air_temperature', lat_min := 40, lat_max := 42.5, lon_min := -100, lon_max := -98) LIMIT 5;"
```
Expected: a column list (the coordinate columns + the value column — note the exact value-column name) and a small result table. Record the real column names/types and the sample rows; Task 4 uses them verbatim. If `lon_min/lon_max` or `time_min/time_max` change the row count as expected, note it.

- [ ] **Step 4: Capture a `plan_read_geo` sample**

Run: `duckdb -unsigned -cmd "LOAD '$PWD/target/debug/eider.duckdb_extension'" -c "SELECT total_chunks, total_bytes FROM plan_read_geo('climate_data.zarr/air_temperature', lat_min := 30, lat_max := 50);"`
Expected: one row with `total_chunks`, `total_bytes`. Record it for Task 6.

No commit (verification only).

---

## Task 4: Write `sql_read_geo.md`

**Files:** Create `docs/docs/usage/sql_read_geo.md`.

- [ ] **Step 1: Write the page using the schema/samples captured in Task 3**

Create `docs/docs/usage/sql_read_geo.md`:

```markdown
---
sidebar_position: 2
---

# read_geo

`read_geo` reads a geospatial array as a relational table — one row per cell —
streaming only the chunks that intersect the requested bounds.

## Signature

```sql
read_geo(uri VARCHAR, [named parameters])
```

- `uri` — positional. A Zarr array, COG, or STAC source (see [Source URIs](./sql_reference.md#source-uris)).

### Named parameters

| Parameter | Type | Description |
|---|---|---|
| `lat_min`, `lat_max` | `DOUBLE` | Latitude bounds; chunks outside are pruned before fetch. |
| `lon_min`, `lon_max` | `DOUBLE` | Longitude bounds. |
| `time_min`, `time_max` | `DOUBLE` | Time-index bounds (numeric). |
| `pins` | `VARCHAR` | Pin non-spatial dimensions to fixed indices (see [pins](./sql_reference.md#pins)). |

## Output

<!-- VERIFY: replace with the exact DESCRIBE output captured in Task 3 Step 3,
     including the real coordinate column names and the value column name. -->

Missing cells (Zarr `fill_value`) are returned as SQL `NULL`.

## Examples

### Basic
<!-- VERIFY against Task 3 Step 3 sample -->
```sql
SELECT * FROM read_geo('climate_data.zarr/air_temperature') LIMIT 5;
```

### Spatial bounding-box pushdown
```sql
SELECT lat, AVG(value) AS mean_temp
FROM read_geo('climate_data.zarr/air_temperature', lat_min := 30, lat_max := 50)
GROUP BY lat
ORDER BY lat;
```

### Pinning a dimension
```sql
SELECT * FROM read_geo('climate_data.zarr/air_temperature', pins := 'time=0') LIMIT 5;
```

## See also
- [read_zarr_metadata](./sql_read_zarr_metadata.md) — inspect shape/type/CRS first.
- [plan_read_geo](./sql_plan_read_geo.md) — estimate the read cost.
```

- [ ] **Step 2: Replace the VERIFY placeholders with real captured content**

Using the Task 3 Step 3 output, fill the Output section with the actual schema and the basic-example result block, and correct any column names (e.g. the value column) and example expressions (`value`, `lat`) to match reality. Confirm `pins := 'time=0'` actually runs (adjust if the pin syntax differs). No `<!-- VERIFY -->` comments may remain.

- [ ] **Step 3: Commit**

```bash
git add docs/docs/usage/sql_read_geo.md
git commit --no-gpg-sign -m "docs: add read_geo SQL reference page

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 5: Write `sql_read_zarr_metadata.md`

**Files:** Create `docs/docs/usage/sql_read_zarr_metadata.md`.

- [ ] **Step 1: Write the page (uses Task 3 Step 2 verified output)**

Create `docs/docs/usage/sql_read_zarr_metadata.md`:

```markdown
---
sidebar_position: 3
---

# read_zarr_metadata

Inspect a Zarr array's structure without reading any chunk data.

## Signature

```sql
read_zarr_metadata(uri VARCHAR)
```

## Output

| Column | Type | Description |
|---|---|---|
| `array_shape` | `VARCHAR` | Full array dimensions, e.g. `[938, 73, 144]`. |
| `chunk_shape` | `VARCHAR` | Chunk dimensions, e.g. `[12, 73, 144]`. |
| `data_type` | `VARCHAR` | Element type, e.g. `Float32`. |
| `crs` | `VARCHAR` | Coordinate reference system, e.g. `EPSG:4326` (`UNKNOWN` if none is declared). |

## Example

```sql
SELECT array_shape, chunk_shape, data_type, crs
FROM read_zarr_metadata('climate_data.zarr/air_temperature');
```

```
┌────────────────┬───────────────┬───────────┬───────────┐
│  array_shape   │  chunk_shape  │ data_type │    crs    │
├────────────────┼───────────────┼───────────┼───────────┤
│ [938, 73, 144] │ [12, 73, 144] │ Float32   │ EPSG:4326 │
└────────────────┴───────────────┴───────────┴───────────┘
```

## See also
- [read_geo](./sql_read_geo.md) — read the array's data.
```

- [ ] **Step 2: Reconcile with real output**

Confirm the example block matches the Task 3 Step 2 capture exactly (column widths may differ — match the values, not the exact box drawing). Adjust if needed.

- [ ] **Step 3: Commit**

```bash
git add docs/docs/usage/sql_read_zarr_metadata.md
git commit --no-gpg-sign -m "docs: add read_zarr_metadata SQL reference page

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 6: Write `sql_plan_read_geo.md`

**Files:** Create `docs/docs/usage/sql_plan_read_geo.md`.

- [ ] **Step 1: Write the page (uses Task 3 Step 4 output)**

Create `docs/docs/usage/sql_plan_read_geo.md`:

```markdown
---
sidebar_position: 4
---

# plan_read_geo

Estimate the cost of a `read_geo` query — how many chunks and bytes it would
fetch — without reading any data. Useful as a dry-run before a large read.

## Signature

```sql
plan_read_geo(uri VARCHAR, [named parameters])
```

Accepts the same named parameters as [read_geo](./sql_read_geo.md), so the
estimate reflects spatial/temporal pruning.

## Output

| Column | Type | Description |
|---|---|---|
| `total_chunks` | `BIGINT` | Number of chunks the query would fetch. |
| `total_bytes` | `BIGINT` | Estimated bytes those chunks occupy. |

## Example

```sql
SELECT total_chunks, total_bytes
FROM plan_read_geo('climate_data.zarr/air_temperature', lat_min := 30, lat_max := 50);
```

<!-- VERIFY: paste the real one-row result from Task 3 Step 4 -->

## See also
- [read_geo](./sql_read_geo.md) — run the actual read.
```

- [ ] **Step 2: Fill the result block from Task 3 Step 4 and remove the VERIFY comment**

- [ ] **Step 3: Commit**

```bash
git add docs/docs/usage/sql_plan_read_geo.md
git commit --no-gpg-sign -m "docs: add plan_read_geo SQL reference page

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 7: Rewrite the SQL Reference landing + update the sidebar

**Files:** Modify `docs/docs/usage/sql_reference.md`, `docs/sidebars.ts`.

- [ ] **Step 1: Rewrite `sql_reference.md` as the landing**

Overwrite `docs/docs/usage/sql_reference.md`:

```markdown
---
sidebar_position: 1
---

# SQL Reference

Eider exposes Zarr/GeoZarr arrays to DuckDB through three table functions. Load
the extension first (see [Installation](./installation.md)): launch
`duckdb -unsigned` and `LOAD '/absolute/path/to/eider.duckdb_extension';`.

## Functions

| Function | Purpose |
|---|---|
| [`read_geo`](./sql_read_geo.md) | Read an array as a relational table, with spatial/temporal pushdown. |
| [`read_zarr_metadata`](./sql_read_zarr_metadata.md) | Inspect an array's shape, chunking, type, and CRS. |
| [`plan_read_geo`](./sql_plan_read_geo.md) | Estimate a read's cost (chunks/bytes) before fetching. |

## Spatial & temporal pushdown

`read_geo` and `plan_read_geo` accept `lat_min`/`lat_max`, `lon_min`/`lon_max`,
and `time_min`/`time_max` (all `DOUBLE`) named parameters. Bounds are applied at
the **chunk** level: chunks lying entirely outside the requested range are never
fetched, so a tightly-scoped query touches only a fraction of the array.

```sql
SELECT lat, AVG(value)
FROM read_geo('s3://bucket/data.zarr', lat_min := 45.0, lat_max := 55.0)
GROUP BY lat;
```

## pins

Use the `pins` parameter (`VARCHAR`) to fix non-spatial dimensions to specific
indices, e.g. a single timestep:

```sql
SELECT * FROM read_geo('climate_data.zarr/air_temperature', pins := 'time=0');
```

## Supported types

Zarr element types map to DuckDB types as follows:

| Zarr type | DuckDB type |
|---|---|
| `bool` | `BOOLEAN` |
| `int8` / `int16` / `int32` / `int64` | `TINYINT` / `SMALLINT` / `INTEGER` / `BIGINT` |
| `uint8` / `uint16` / `uint32` / `uint64` | `UTINYINT` / `USMALLINT` / `UINTEGER` / `UBIGINT` |
| `float32` / `float64` | `FLOAT` / `DOUBLE` |
| `string` | `VARCHAR` |

A Zarr `fill_value` is surfaced as SQL `NULL`.

## Source URIs

The URI argument accepts local paths and remote `s3://` and `http(s)://`
locations (configured via OpenDAL environment variables — see
[Installation](./installation.md)). `read_geo` reads Zarr arrays directly; COG
and STAC sources are also supported where available.
```

- [ ] **Step 2: Verify the `pins := 'time=0'` example and the type table against reality**

Run the `pins` example from Step 1 against the built extension; if the pin syntax or a type mapping differs from what is documented, correct the page. The "COG and STAC sources are also supported where available" sentence must reflect what actually works — if a quick `read_geo` against a COG/STAC source does not work in this build, soften to "experimental" or remove that clause rather than overclaim.

- [ ] **Step 3: Update the sidebar**

In `docs/sidebars.ts`, replace the `SQL Reference` category's `items` with:

```typescript
      items: [
        'usage/sql_reference',
        'usage/sql_read_geo',
        'usage/sql_read_zarr_metadata',
        'usage/sql_plan_read_geo',
      ],
```

(Leave the other four categories unchanged.)

- [ ] **Step 4: Commit**

```bash
git add docs/docs/usage/sql_reference.md docs/sidebars.ts
git commit --no-gpg-sign -m "docs: rewrite SQL Reference landing and wire per-function pages

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 8: Full verification

**Files:** none (verification only).

- [ ] **Step 1: Rust suite + lint**

Run: `cargo test -p geozarr_core -p eider_extension 2>&1 | grep -E "test result:|FAILED|error\["`
Expected: all pass (the two new metadata tests, the two chunk_shape tests, and the existing extension tests).
Run: `cargo clippy -p geozarr_core -p eider_extension -- -D warnings 2>&1 | tail -3`
Expected: clean.
Run: `cargo fmt --all -- --check`
Expected: no diffs (run `cargo fmt` + amend the relevant commit if needed).

- [ ] **Step 2: Docs build (gates broken links)**

Run: `cd docs && (test -d node_modules || npm ci) && npm run build 2>&1 | tail -5`
Expected: `[SUCCESS]`, no "Broken link" errors. The four SQL Reference pages cross-link only to each other and to `installation.md` (all exist). Links to not-yet-written sections (CLI Reference, Guides) must remain plain text.

- [ ] **Step 3: Confirm no stale syntax slipped in**

Run: `grep -rnE "read_zarr\(|Some\(ChunkShape|allow_unsigned_extensions = true" docs/docs/usage/sql_*.md || echo "clean"`
Expected: `clean`.

- [ ] **Step 4: Confirm scope (no file moves, no out-of-scope page edits)**

Run: `git diff --name-status main..HEAD`
Expected: `M geozarr_core/src/metadata.rs`, `M extension/src/metadata_vtab.rs`, `M docs/docs/usage/sql_reference.md`, `M docs/sidebars.ts`, `A` for the three new `sql_*` pages, plus the spec/plan docs — and NOTHING else (no edits to cli_tui, exporting, engineering pages, or the homepage).

---

## Self-review notes (author checklist, already applied)

- **Spec coverage:** landing + 3 per-function pages (Tasks 4–7) ✓; shared concepts — params, pushdown, pins, supported types, source URIs (Task 7) ✓; chunk_shape fix (Task 2) ✓; crs nested-parse fix (Task 1) ✓; examples verified against post-fix extension (Task 3 feeds Tasks 4–6, with explicit reconcile steps) ✓; Rust CI + docs build (Task 8) ✓; sidebar wiring, no file moves (Tasks 7, 8 Step 4) ✓.
- **Verify-before-publish:** `read_geo`'s exact output schema/value-column name and the COG/STAC "supported where available" claim are explicitly verified at implementation (Task 3 Step 3, Task 4 Step 2, Task 7 Step 2) — flagged, not assumed.
- **Type consistency:** `resolved_crs()` is defined (Task 1 Step 3) and used (Task 1 Step 5); `render_chunk_shape` is defined and used (Task 2). Test references match.
- **Placeholders:** the only `<!-- VERIFY -->` markers are in draft page bodies with an explicit follow-up step to replace them with captured real output before commit; Task 8 Step 3-equivalent ensures no stale syntax, and no VERIFY comments may remain (Task 4 Step 2, Task 6 Step 2 state this).
- **Non-goals honored:** no CLI/guide/engineering page changes, no new STAC/COG capability, no file moves, fixes limited to rendering/parsing.
