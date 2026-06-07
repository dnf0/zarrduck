# Design: Docs Workstream E — Engineering / Concepts Deep-Dives

- **Date:** 2026-06-07
- **Status:** Approved (design); implementation pending
- **Scope:** The four "Concepts & Engineering" deep-dive pages under `docs/docs/engineering/`. Fifth and final of the sequenced documentation workstreams (A, B, C, D done; order A→B→C→D→E). Docs-only — no production (Rust) code changes. The existing benchmark suite is *run* to capture real numbers, not modified.

## Context

The four engineering pages are thin stubs (~580–1073 bytes) that mix accurate description with imprecise and outright fabricated claims. A source-grounded audit (cited inline below) established ground truth:

| Page | Audit verdict |
|---|---|
| `architecture.mdx` | Data flow correct; **threading claim wrong** ("lock-free worker pool"). Stale `read_zarr`. |
| `spatial_pruning.mdx` | **Accurate** — affine transform + binary-search bbox pruning is real. |
| `cog_virtualization.mdx` | COG virtualization **real at the library level**; "**~2.4ms for a 10,000-tile COG**" is fabricated. |
| `benchmarks.mdx` | Plot numbers **hardcoded**; the "Eider vs xarray/zarr-python" head-to-head has **no source**; "32,830 rows" unverifiable; only the coordinate microbench is real (~10.2µs measured vs 9.5µs claimed). |
| STAC via SQL | **Absent** — `read_geo` errors on STAC paths. |

E is therefore primarily an **honesty/accuracy pass**: keep what's true, correct what's imprecise, delete what's fabricated, and label maturity honestly.

## Ground-truth source references (for the implementer)

- **Entry points / data flow:** `extension/src/lib.rs:8-14` registers `read_geo`, `plan_read_geo`, `read_zarr_metadata`. Flow: `ReadGeoVTab::bind()` (`extension/src/table_function.rs:93`) → `geozarr_core::dataset::ZarrDataset::open` (`geozarr_core/src/dataset.rs:21`) → `geozarr_core::store::resolve_sync_store` (`geozarr_core/src/store.rs:141-178`) → `zarrs` `Array::open` → codecs → OpenDAL operator → object store.
- **Threading (real model):** `extension/src/table_function.rs:179` `set_max_threads(num_chunks)`; per-thread `LocalState` keyed by `std::thread::current().id()` (`:208`); shared `GlobalState.grid_iterator` behind a `Mutex` (`:67`, `:163-168`, `:239-241`) hands out the next chunk coordinate. No tokio/rayon in `extension/Cargo.toml`. (So: DuckDB-managed worker threads consuming a shared chunk work-queue under a mutex — **not** lock-free.)
- **Codecs:** `extension/Cargo.toml:35` — `zarrs` features `blosc, gzip, crc32c, zstd, sharding, transpose, ndarray`; `geozarr_core/Cargo.toml:7` adds `opendal, async`.
- **Spatial pruning:** bbox params bound at `extension/src/table_function.rs:82-90,117-129`; `compute_bounds` at `:145`; binary search (`partition_point`) in `geozarr_core/src/query_planner.rs:17-112`; inverse affine `(value − translation)/scale` in `geozarr_core/src/dataset.rs:196-228`; `apply_transform` (`translation + grid_index*scale`) in `geozarr_core/src/coordinates.rs:4-8`; chunk iteration over intersecting chunks at `extension/src/table_function.rs:168-179`.
- **COG:** TIFF/IFD parse (tags 256/257/322/323/324/325) in `geozarr_core/src/cog.rs:1-162`; `VirtualCogStore` synthesizes `.zmetadata`/`.zarray` (`geozarr_core/src/virtual_store.rs:1-152`); chunk key `"y.x"` → tile offset+length (`virtual_store.rs:70-99`); `.tif/.tiff` detection in `resolve_sync_store` (`store.rs:144,153-170`); conditional e2e test `extension/tests/test_cog_eval.rs` calls `read_geo('test.tif')`.
- **STAC:** `read_geo` short-circuits with an error on STAC paths (`extension/src/table_function.rs:100-106`); `VirtualStacStore` exists but is unreachable from SQL (`store.rs:179-315`).
- **Benchmarks (real harness):** `extension/benches/coordinate_bench.rs` (`populate_lat_batch_2048`, measured ~10.2µs); `geozarr_core/benches/scanner_bench.rs` (`scanner_read_chunk_subset_remote`). Fabricated plot data lives in `docs/src/components/BenchmarkPlots.tsx:18-24,52-59`.

## Decisions (from brainstorming)

| Decision | Choice |
|---|---|
| Benchmarks page | **Real microbenches only.** Run the existing criterion benches, publish measured numbers as a results table + a "How to reproduce" section. Remove the head-to-head Python comparison and all fabricated figures. **Delete** the `BenchmarkPlots.tsx` component (table over hardcoded plot). |
| COG / STAC | **Document as-built, label maturity honestly.** COG page rewritten accurately with an "experimental / reachable via `ZarrDataset::open`, not a first-class `read_geo` source" callout; fabricated 2.4ms removed. STAC folded in as a short "planned / not wired to SQL yet" note. No fictional capability claimed. |
| Threading | Correct to the real DuckDB-managed-threads + shared-mutex chunk work-queue model; drop "lock-free." |
| Structure | Keep the four pages and the "Concepts & Engineering" sidebar category as-is. No new landing, no restructure. |
| Grounding | Every non-obvious claim cites real `file:line`; diagrams kept but corrected. |

## Page-by-page plan

### `architecture.mdx`
- Fix the mermaid graph: `read_zarr` → the three real table functions; show `geozarr_core` → `zarrs` codec pipeline → OpenDAL → object store (`s3://`/`http(s)://`/local).
- Replace the "Multi-threading Model" section with the accurate model: DuckDB allocates worker threads (`set_max_threads(num_chunks)`); each thread pulls the next chunk coordinate from a shared `grid_iterator` guarded by a `Mutex`, then reads/decodes that chunk independently. Honest framing: a parallel chunk **work-queue**, not lock-free; concurrency bounded by chunk count and DuckDB's thread pool. Note no tokio/rayon — OpenDAL drives the async network layer inside the store wrapper.
- Add the real codec list and Zarr V2/V3 support. Cite file:line.

### `spatial_pruning.mdx`
- Keep/refine the mermaid. Document the two pruning paths precisely:
  - Coordinate-array dims: binary search (`partition_point`) maps each bbox bound to an index range.
  - Affine-transform dims: inverse transform `(value − translation)/scale` computes the index range directly (no coordinate array fetched).
- Bounds → chunk-grid coordinates → only intersecting chunks iterated; non-matching chunks are never requested.
- Tie in `plan_read_geo` as the dry-run reporting post-pruning `total_chunks`/`total_bytes`. Cite file:line.

### `cog_virtualization.mdx`
- Keep the sequence diagram (corrected). Describe the real mechanism: range-GET the TIFF header → parse IFD tags (offsets/byte-counts/tile dims) → `VirtualCogStore` synthesizes `.zmetadata`/`.zarray` in memory → Zarr chunk key `"y.x"` maps to a tile byte-range → OpenDAL issues the range GET → `zarrs` decodes lazily.
- **Remove** the fabricated "~2.4ms for a 10,000-tile COG."
- Maturity callout: experimental; reachable via `ZarrDataset::open("*.tif")` and exercised by a conditional e2e test through `read_geo('*.tif')`; internals may change; not yet a first-class documented `read_geo` source.
- Short STAC subsection: a `VirtualStacStore` (STAC Item → COG assets) exists at the library level, but `read_geo` currently returns an explicit error for STAC paths — planned, not yet wired to SQL. Cite file:line.

### `benchmarks.mdx`
- Remove the `import { HeadToHeadPlot, ScalingPlot }` line, both component usages, the head-to-head Python comparison, the "32,830 rows" framing, and the COG/coordinate fabricated figures.
- Run `cargo bench --bench coordinate_bench` (extension) and `cargo bench --bench scanner_bench` (geozarr_core); record the **measured** numbers (criterion's median).
- Present a **results table**: benchmark name, what it measures, measured time, with a one-line interpretation (e.g. coordinate generation is dominated by arithmetic, not network). Be explicit these are microbenchmarks on one machine, not cross-tool comparisons.
- Add a "How to reproduce" section with the exact `cargo bench` commands and where the bench sources live.
- **Delete** `docs/src/components/BenchmarkPlots.tsx` (now unused). Confirm nothing else imports it (`grep -rn BenchmarkPlots docs/src docs/docs`).

## Accuracy & verification

- The two criterion benches are actually run at implementation time; only their measured numbers are published. If a bench cannot run in the environment, the page documents the bench and the reproduce command **without** inventing a number (states it as "run locally to measure"), rather than carrying a fabricated figure.
- Every retained quantitative or behavioral claim is grounded in the cited source; no figure appears that isn't either measured or cited.
- Experimental/absent capabilities (COG via SQL, STAC) are labeled honestly; no fictional capability is implied.
- `cd docs && npm run build` succeeds with **no broken links** (`onBrokenLinks: 'throw'`) and **no dangling component import** after `BenchmarkPlots.tsx` is deleted.
- Scope check: only `docs/docs/engineering/*.mdx` and the deleted `docs/src/components/BenchmarkPlots.tsx` change. No Rust source changes; no sidebar restructure.

## Non-goals (deferred)

- Any Rust/product change, including new benchmarks, COG-as-first-class-`read_geo`, or STAC-via-SQL.
- Restructuring the sidebar or adding a Concepts landing page.
- Re-documenting reference material owned by B (SQL) / C (CLI); engineering pages link to them where useful rather than duplicating.
