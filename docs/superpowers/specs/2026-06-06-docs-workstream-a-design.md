# Design: Docs Workstream A — Information Architecture + Landing + Getting Started

- **Date:** 2026-06-06
- **Status:** Approved (design); implementation pending
- **Scope:** The Docusaurus documentation site under `docs/`. First of five sequenced documentation workstreams (A→B→C→D→E). Workstream A establishes the site information architecture and builds the Getting Started section. No production (Rust) code changes.

## Context

The docs site (`docs/docs/`) currently has 8 thin stub pages (~10–29 lines each) in two flat categories ("Using Eider", "Engineering Deep-Dive"). There is no docs landing/overview page and no guided getting-started. Several pages are stale relative to the current product (e.g. `sql_reference.md` uses `read_zarr` rather than `read_geo`; `installation.md` shows `SET allow_unsigned_extensions = true;`, which errors at DuckDB runtime — the flag must be set at launch via `duckdb -unsigned`).

This is the first of five workstreams agreed for a serious docs expansion, in order:
- **A. Information architecture + landing + getting-started** (this spec)
- B. SQL / extension reference
- C. CLI reference
- D. Guides / tutorials
- E. Engineering deep-dives

Each workstream gets its own spec → plan → implementation.

## Decisions (from brainstorming)

| Decision | Choice |
|---|---|
| Primary program goal | All five workstreams, done in sequence (A→B→C→D→E) |
| Getting-started entry points | Two equal side-by-side tracks: SQL extension and `eider` CLI |
| A scope boundary | Docs site only; no marketing-homepage redesign (only the "Quick Start" link target changes) |
| Existing pages | Reslotted into the new IA by regrouping in the sidebar; keep file paths stable to avoid broken links |

## Target information architecture

Restructure `docs/sidebars.ts` into five top-level categories. A builds out **Getting Started** and reslots existing pages into their future homes (deepened by later workstreams). No empty placeholder pages.

- **Getting Started** *(A builds)*
  - `usage/overview` — new docs landing/root
  - `usage/installation` — fixed (existing file, corrected content)
  - `usage/quickstart-sql` — new (SQL extension track)
  - `usage/quickstart-cli` — new (CLI track)
- **Guides** *(D later)* — `usage/exporting` (existing)
- **SQL Reference** *(B later)* — `usage/sql_reference` (existing)
- **CLI Reference** *(C later)* — `usage/cli_tui` (existing)
- **Concepts & Engineering** *(E later)* — `engineering/architecture`, `engineering/spatial_pruning`, `engineering/cog_virtualization`, `engineering/benchmarks` (existing)

New Getting Started pages live under `docs/docs/usage/` alongside the existing usage pages (keeps doc IDs/paths simple; no file moves of existing pages, only sidebar regrouping). The sidebar's first item becomes `usage/overview`.

## Content to build

### `usage/overview.md` (docs landing)
- One-paragraph "what is Eider" framing (Zarr/GeoZarr/COG → DuckDB SQL, zero-copy, cloud-native).
- The two entry points explained, with a clear "choose your path" signpost linking to the two quickstarts.
- A short "where to go next" map of the site (Guides, SQL Reference, CLI Reference, Concepts).
- Set as the docs root / first sidebar item.

### `usage/quickstart-sql.md` (SQL extension track)
A ~5-minute path, every block verified against the real extension:
1. Install / obtain the extension (link to installation).
2. Launch `duckdb -unsigned` (note: `SET allow_unsigned_extensions` cannot be changed at runtime).
3. `LOAD '<absolute path>/eider.duckdb_extension';` (note `LOAD` requires an absolute path).
4. Inspect: `SELECT array_shape, chunk_shape, data_type FROM read_zarr_metadata('<zarr>');`
5. Query: a `read_geo('<zarr>', lat_min := …, lat_max := …)` aggregation with `GROUP BY`.
6. "Next steps" → SQL Reference, Guides.

### `usage/quickstart-cli.md` (CLI track)
A ~5-minute path, every command verified against the real CLI:
1. Install / build the `eider` CLI (link to installation).
2. A minimal flow — e.g. `eider info <zarr>` → `eider extract <zarr> <vector> --out out.duckdb` → `eider shell out.duckdb` (or `eider plot`).
3. Note the `--output=json` agent-friendly mode.
4. "Next steps" → CLI Reference, Guides.

### `usage/installation.md` (fix existing)
- Correct the load instructions: launch `duckdb -unsigned`; `LOAD '<absolute path>';` (absolute path required); the released artifact name.
- Cover both the extension and the CLI (binary releases + build-from-source).
- Keep the OpenDAL auth/env section (`AWS_*`, `GEOZARR_ALLOW_PATH`).

## Link / config updates

- `docs/sidebars.ts` — the new five-category structure above.
- The homepage "Quick Start" button (`docs/src/pages/index.tsx`) and any navbar link currently targeting `/docs/usage/installation` → retarget to the new `usage/overview`. This is the single allowed homepage touch.
- No other `index.tsx` changes.

## Example dataset (content decision for the plan)

The SQL quickstart needs a runnable dataset. The plan resolves this with a verification step, preferring **repo-local determinism**: clone the repo and generate the sample (`python3 scripts/generate_demo_data.py` → `climate_data.zarr/air_temperature`), with a note that the same queries work against remote `s3://`/`https://` Zarr. If a stable public Zarr URL can be verified at implementation time, it may be used additionally. The chosen example's exact output must be confirmed against the real extension before publishing.

## Accuracy & verification

- Every command/SQL/`eider` invocation in A is run against the real binary/extension and its output confirmed before the page ships (same discipline used for the demo GIF). Stale or aspirational syntax (e.g. unverified `time_min`/`time_max` parameters) must not be carried over un-verified.
- `cd docs && npm run build` succeeds with **no broken links** (Docusaurus fails the build on broken internal links by default).
- Manual: the new pages render in the sidebar under the correct categories; the overview is the docs root; the two quickstart tracks are reachable and signposted from the overview.

## Non-goals (deferred)

- Deepening the SQL Reference, CLI Reference, or Engineering pages (workstreams B/C/E).
- New guides/tutorials beyond the two quickstarts (workstream D).
- Marketing-homepage redesign (only the Quick Start link target changes).
- Any Rust/source changes.
