# Docs Workstream A (IA + Getting Started) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Establish the documentation site's five-category information architecture and build a Getting Started section (docs landing/overview, two equal quickstart tracks, fixed installation page).

**Architecture:** Edit the Docusaurus site under `docs/`. Add three new Markdown pages and fix one under `docs/docs/usage/`, regroup all existing pages into five sidebar categories in `docs/sidebars.ts` (no file moves → no broken links), and retarget the homepage "Quick Start" button. Verify with `npm run build` (Docusaurus is configured `onBrokenLinks: 'throw'`).

**Tech Stack:** Docusaurus (TypeScript sidebar/config, Markdown content), Node/npm.

---

## Conventions & verified facts

Work from the repo root `/Users/danielfisher/repos/zarrduck` on branch `docs/workstream-a-getting-started` (do NOT commit to `main`). Conventional Commits, `--no-gpg-sign`, end messages with the `Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>` trailer.

Verified live this session (use verbatim):
- `docs/docusaurus.config.ts` sets `onBrokenLinks: 'throw'` — the build fails on broken internal links.
- The SQL beats produce this real output against the repo's `climate_data.zarr` with the built extension:
  - `SELECT array_shape, chunk_shape, data_type FROM read_zarr_metadata('climate_data.zarr/air_temperature')` → `[938, 73, 144]` | `Some(ChunkShape([12, 73, 144]))` | `Float32`.
  - `SELECT lat, AVG(value) AS mean_temp FROM read_geo('climate_data.zarr/air_temperature', lat_min := 30, lat_max := 50) GROUP BY lat ORDER BY lat LIMIT 5` → `30.0|19.097122000246408`, `32.5|16.925729751506257`, `35.0|14.822018816108105`, `37.5|13.73074441969959`, `40.0|12.425807312232353`.
- `LOAD` requires an absolute path; `SET allow_unsigned_extensions` errors at runtime (launch `duckdb -unsigned`). `read_geo`/`read_zarr_metadata` are the current function names (`read_zarr` is gone).
- The CLI subcommands `info`, `extract`, `shell`, `plot` exist and accept `--output=json`. The repo ships `scripts/demo_region.geojson` and `scripts/generate_demo_data.py` (writes `climate_data.zarr`).
- Homepage Quick Start link: `docs/src/pages/index.tsx` line ~22, `to="/docs/usage/installation"`.

## File structure

- Create: `docs/docs/usage/overview.md` — docs landing.
- Create: `docs/docs/usage/quickstart-sql.md` — SQL extension track.
- Create: `docs/docs/usage/quickstart-cli.md` — CLI track.
- Modify: `docs/docs/usage/installation.md` — fix stale load instructions.
- Modify: `docs/sidebars.ts` — five-category IA.
- Modify: `docs/src/pages/index.tsx` — retarget Quick Start button.

New pages are created BEFORE the sidebar references them, so the build never sees a dangling reference.

---

## Task 1: Fix `installation.md`

**Files:** Modify `docs/docs/usage/installation.md`

- [ ] **Step 1: Replace the file contents**

Overwrite `docs/docs/usage/installation.md` with:

```markdown
# Installation

Eider ships as two pieces that work together: a loadable **DuckDB extension** (queried from SQL) and the **`eider` CLI**.

## DuckDB extension

### From a release

Download the `eider-<platform>.duckdb_extension` for your platform from the
[Releases page](https://github.com/dnf0/eider/releases) and rename it to
`eider.duckdb_extension` — DuckDB derives the load entry point from the filename.

Launch DuckDB allowing unsigned extensions. The flag must be set **at startup**
(`SET allow_unsigned_extensions` cannot be changed at runtime), and `LOAD`
requires an **absolute** path:

```bash
duckdb -unsigned
```

```sql
LOAD '/absolute/path/to/eider.duckdb_extension';
```

### From source

Requires the Rust toolchain and `cargo-duckdb-ext-tools`
(`cargo install cargo-duckdb-ext-tools`):

```bash
git clone https://github.com/dnf0/eider.git
cd eider
cargo duckdb-ext build -o target/debug/eider.duckdb_extension \
  -d v1.5.2 -- --no-default-features --features loadable-extension
```

## CLI

Download the `eider` binary from the [Releases page](https://github.com/dnf0/eider/releases),
or build from source:

```bash
cargo build --release -p eider   # binary at target/release/eider
```

## Authentication & access

Eider streams data through [Apache OpenDAL](https://opendal.apache.org/).
Configure access with standard environment variables:

- `AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY`, `AWS_REGION` — for `s3://`
- `GEOZARR_ALLOW_PATH` — permit local filesystem reads, e.g. `export GEOZARR_ALLOW_PATH=/`
```

- [ ] **Step 2: Verify no stale syntax remains**

Run: `grep -nE "SET allow_unsigned|read_zarr\(|eider_extension.duckdb_extension" docs/docs/usage/installation.md || echo "clean"`
Expected: `clean`.

- [ ] **Step 3: Commit**

```bash
git add docs/docs/usage/installation.md
git commit --no-gpg-sign -m "docs: fix installation load instructions

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 2: Create `overview.md` (docs landing)

**Files:** Create `docs/docs/usage/overview.md`

- [ ] **Step 1: Write the file**

Create `docs/docs/usage/overview.md` with:

```markdown
---
sidebar_position: 1
---

# Eider

Eider is a native DuckDB extension (with a companion CLI) that reads N-dimensional
[Zarr](https://zarr.dev/) / GeoZarr arrays and Cloud-Optimized GeoTIFFs **directly as
flat relational tables** — zero-copy, straight into DuckDB's vectorized engine, with
chunk-level spatial pruning so out-of-range data is never fetched.

## Two ways to use Eider

Eider has two front doors. Pick the one that fits how you work:

| | **DuckDB SQL extension** | **`eider` CLI** |
|---|---|---|
| Best for | Querying arrays as SQL tables | Discovery, extraction, agentic workflows |
| Start here | [SQL quickstart →](./quickstart-sql.md) | [CLI quickstart →](./quickstart-cli.md) |

## Where to go next

- **[Installation](./installation.md)** — get the extension and the CLI.
- **SQL Reference** — every table function and its parameters.
- **CLI Reference** — every `eider` subcommand and flag.
- **Guides** — task-oriented how-tos (extraction, exporting, cloud access).
- **Concepts & Engineering** — architecture, spatial pruning, COG virtualization, benchmarks.
```

- [ ] **Step 2: Commit**

```bash
git add docs/docs/usage/overview.md
git commit --no-gpg-sign -m "docs: add getting-started overview landing page

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 3: Create `quickstart-sql.md`

**Files:** Create `docs/docs/usage/quickstart-sql.md`

- [ ] **Step 1: Verify the example queries still produce the documented output**

Run (from repo root, with the extension built at `target/debug/eider.duckdb_extension`):
```bash
duckdb -unsigned -cmd "LOAD '$PWD/target/debug/eider.duckdb_extension'" -c "SELECT array_shape, chunk_shape, data_type FROM read_zarr_metadata('climate_data.zarr/air_temperature'); SELECT lat, AVG(value) AS mean_temp FROM read_geo('climate_data.zarr/air_temperature', lat_min := 30, lat_max := 50) GROUP BY lat ORDER BY lat LIMIT 5;"
```
Expected: matches the output embedded in Step 2. If it differs, update the page's result blocks to the real output before committing. (If the extension isn't built: `cargo duckdb-ext build -o target/debug/eider.duckdb_extension -d v1.5.2 -- --no-default-features --features loadable-extension`; if the data is missing: `python3 scripts/generate_demo_data.py`.)

- [ ] **Step 2: Write the file**

Create `docs/docs/usage/quickstart-sql.md` with:

```markdown
---
sidebar_position: 3
---

# SQL Quickstart

Query a Zarr array as a SQL table in about five minutes. See
[Installation](./installation.md) to get the extension first.

This quickstart uses the sample dataset from the repo. From a clone:

```bash
python3 scripts/generate_demo_data.py   # writes ./climate_data.zarr
```

The same queries work against remote `s3://` / `https://` Zarr — just swap the path.

## 1. Launch DuckDB and load Eider

```bash
duckdb -unsigned
```

```sql
LOAD '/absolute/path/to/eider.duckdb_extension';
```

## 2. Inspect the array

```sql
SELECT array_shape, chunk_shape, data_type
FROM read_zarr_metadata('climate_data.zarr/air_temperature');
```

```
┌────────────────┬─────────────────────────────────┬───────────┐
│  array_shape   │           chunk_shape           │ data_type │
├────────────────┼─────────────────────────────────┼───────────┤
│ [938, 73, 144] │ Some(ChunkShape([12, 73, 144])) │ Float32   │
└────────────────┴─────────────────────────────────┴───────────┘
```

## 3. Query with a spatial bounding box

`read_geo` streams only the chunks that intersect your bounds:

```sql
SELECT lat, AVG(value) AS mean_temp
FROM read_geo('climate_data.zarr/air_temperature', lat_min := 30, lat_max := 50)
GROUP BY lat
ORDER BY lat
LIMIT 5;
```

```
┌────────┬────────────────────┐
│  lat   │     mean_temp      │
├────────┼────────────────────┤
│   30.0 │ 19.097122000246408 │
│   32.5 │ 16.925729751506257 │
│   35.0 │ 14.822018816108105 │
│   37.5 │  13.73074441969959 │
│   40.0 │ 12.425807312232353 │
└────────┴────────────────────┘
```

## Next steps

- **SQL Reference** — all table functions and parameters.
- **Guides** — exporting results, cloud access.
```

- [ ] **Step 3: Commit**

```bash
git add docs/docs/usage/quickstart-sql.md
git commit --no-gpg-sign -m "docs: add SQL extension quickstart

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 4: Create `quickstart-cli.md`

**Files:** Create `docs/docs/usage/quickstart-cli.md`

- [ ] **Step 1: Verify the CLI commands run**

Run (from repo root, with the CLI + extension built and `climate_data.zarr` present):
```bash
DUCKDB_EXTENSION_DIRECTORY=$PWD/.duckdb_ext_cache cargo run -q -p eider -- info climate_data.zarr/air_temperature --output=json
```
Expected: a JSON object with `"array_shape"`. This confirms `eider info` + the function names are current. (Extraction/shell/plot are covered by the integration test suite; this step just confirms the entry command works for the doc.)

- [ ] **Step 2: Write the file**

Create `docs/docs/usage/quickstart-cli.md` with:

```markdown
---
sidebar_position: 4
---

# CLI Quickstart

Go from a Zarr array to analysis with the `eider` CLI in about five minutes. See
[Installation](./installation.md) to get the CLI (and the extension it loads).

Using the repo's sample data:

```bash
python3 scripts/generate_demo_data.py   # writes ./climate_data.zarr
```

## 1. Inspect a dataset

```bash
eider info climate_data.zarr/air_temperature
```

## 2. Extract data intersecting a region

`extract` downloads only the intersecting chunks and joins them with your vector
polygons into a local DuckDB file:

```bash
eider extract climate_data.zarr/air_temperature scripts/demo_region.geojson \
  --out analysis.duckdb --yes
```

## 3. Explore the result

```bash
# interactive SQL shell over the extracted data
eider shell analysis.duckdb

# or render an ASCII chart
eider plot analysis.duckdb
```

## Agent mode

Every command accepts `--output=json` for machine-readable output, so the CLI is
drop-in for LLM agents and scripts:

```bash
eider info climate_data.zarr/air_temperature --output=json
```

## Next steps

- **CLI Reference** — every subcommand and flag.
- **Guides** — temporal resampling, exporting.
```

- [ ] **Step 3: Commit**

```bash
git add docs/docs/usage/quickstart-cli.md
git commit --no-gpg-sign -m "docs: add CLI quickstart

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 5: Restructure the sidebar (five-category IA)

**Files:** Modify `docs/sidebars.ts`

- [ ] **Step 1: Replace the sidebar definition**

Overwrite `docs/sidebars.ts` with:

```typescript
import type {SidebarsConfig} from '@docusaurus/plugin-content-docs';

const sidebars: SidebarsConfig = {
  docsSidebar: [
    {
      type: 'category',
      label: 'Getting Started',
      items: [
        'usage/overview',
        'usage/installation',
        'usage/quickstart-sql',
        'usage/quickstart-cli',
      ],
    },
    {
      type: 'category',
      label: 'Guides',
      items: ['usage/exporting'],
    },
    {
      type: 'category',
      label: 'SQL Reference',
      items: ['usage/sql_reference'],
    },
    {
      type: 'category',
      label: 'CLI Reference',
      items: ['usage/cli_tui'],
    },
    {
      type: 'category',
      label: 'Concepts & Engineering',
      items: [
        'engineering/architecture',
        'engineering/spatial_pruning',
        'engineering/cog_virtualization',
        'engineering/benchmarks',
      ],
    },
  ],
};

export default sidebars;
```

- [ ] **Step 2: Commit**

```bash
git add docs/sidebars.ts
git commit --no-gpg-sign -m "docs: restructure sidebar into five categories

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 6: Retarget the homepage Quick Start button

**Files:** Modify `docs/src/pages/index.tsx`

- [ ] **Step 1: Update the link target**

In `docs/src/pages/index.tsx`, change the Quick Start button target:

```tsx
            to="/docs/usage/overview">
```
(was `to="/docs/usage/installation">`)

- [ ] **Step 2: Verify**

Run: `grep -n 'to="/docs/usage/' docs/src/pages/index.tsx`
Expected: shows `to="/docs/usage/overview"` (no remaining `/docs/usage/installation`).

- [ ] **Step 3: Commit**

```bash
git add docs/src/pages/index.tsx
git commit --no-gpg-sign -m "docs: point homepage Quick Start at getting-started overview

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 7: Build verification

**Files:** none (verification only)

- [ ] **Step 1: Install docs dependencies (if needed)**

Run: `cd docs && (test -d node_modules || npm ci)`
Expected: dependencies present (no error).

- [ ] **Step 2: Build the site (gates broken links)**

Run: `cd docs && npm run build`
Expected: build completes successfully. Because `onBrokenLinks: 'throw'`, any broken internal link (e.g. a relative link in the new pages pointing at a wrong path) fails the build — fix the offending link and re-run. The "SQL Reference / CLI Reference / Guides / Concepts" bullet items in `overview.md` are intentionally plain text (not links) since those sections are populated by later workstreams; confirm they are not Markdown links.

- [ ] **Step 3: Sanity-check the rendered structure (optional local preview)**

Run: `cd docs && npm run build 2>&1 | tail -5`
Expected: no "Broken link" warnings/errors; the four Getting Started pages and five categories are present in the generated sidebar.

- [ ] **Step 4: Confirm no existing pages were moved (paths stable)**

Run: `git status --short docs/docs/` and `git diff --stat main..HEAD -- docs/docs/`
Expected: only `usage/overview.md`, `usage/quickstart-sql.md`, `usage/quickstart-cli.md` added and `usage/installation.md` modified; no renames/deletions of `sql_reference.md`, `cli_tui.md`, `exporting.md`, or any `engineering/*` page.

- [ ] **Step 5: Final commit (only if Step 1 created/changed lockfiles)**

```bash
# Only if `npm ci` changed tracked files (it should not). Otherwise skip.
git status --short docs/
```
If nothing changed, no commit needed — Tasks 1–6 already committed all content.

---

## Self-review notes (author checklist, already applied)

- **Spec coverage:** five-category IA (Task 5) ✓; overview landing (Task 2) ✓; two equal quickstart tracks (Tasks 3, 4) ✓; installation fix (Task 1) ✓; homepage Quick Start retarget (Task 6) ✓; existing pages reslotted by sidebar regrouping with stable paths (Task 5 + Task 7 Step 4) ✓; accuracy via real-command verification (Task 3 Step 1, Task 4 Step 1) ✓; `npm run build` / no broken links (Task 7) ✓; repo-local example dataset with remote note (Tasks 3, 4) ✓.
- **Ordering:** pages are created (Tasks 2–4) before the sidebar references them (Task 5), so the build never sees a dangling doc ID.
- **Link safety:** `overview.md` links only to pages that exist in this workstream (`installation`, `quickstart-sql`, `quickstart-cli`); future sections (SQL Reference, CLI Reference, Guides, Concepts) are plain text, not links, to keep `onBrokenLinks: 'throw'` green (called out explicitly in Task 7 Step 2).
- **Placeholder scan:** none; the example-dataset decision is resolved (repo-local + remote note), result blocks are concrete verified output.
- **Non-goals honored:** no homepage redesign (only the one link), no deepening of B/C/E pages, no file moves, no Rust changes.
