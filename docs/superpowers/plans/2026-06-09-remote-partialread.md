# Remote Partial-Read Benchmark — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development. Steps use `- [ ]`.

**Goal:** A reproducible benchmark of eider's chunk-pruning over a network store (bytes fetched + requests, time secondary) vs a naive whole-array read and a competent chunk-aware baseline (xarray/zarr `.sel`, rasterio windowed `/vsicurl`), written into `engineering/benchmarks`. Plus a bundled follow-up fix to `bench_zonal_headtohead.py`.

**Architecture:** `scripts/bench_remote_partialread.py` — generate a lat/lon-chunked Zarr + a tiled COG, serve from a **byte-logging, Range-capable** local HTTP server, and per (format × window × contender) run a correctness gate then record bytes/requests/time. Controller runs the final capture; doc written from captured JSON.

**Tech Stack:** Python; duckdb==1.5.2 + eider extension (opendal `services-http`); xarray, zarr, fsspec, aiohttp, rioxarray, rasterio; numpy.

---

## Conventions
Repo root `/Users/danielfisher/repos/zarrduck`, branch `bench/remote-partial-read` (based on the COG centre-coords fix; rebase onto main once that merges). Venv `/tmp/bench_venv` (`source` it). Conventional Commits, `--no-gpg-sign`, trailer `Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>`. Never `git add -A` (note unrelated dirty `scanner.rs`/`mcts_state_scanner.json` — never stage them). Extension at `target/debug/eider.duckdb_extension` (rebuilt with the centre fix).

**Confirmed (spike):** eider reads `read_geo('http://127.0.0.1:PORT/store.zarr/var', lon_min:=, …)` via opendal http; a logging server showed a time-pruned read fetched 1/79 chunk files (0.39 vs 30.3 MB). A Zarr chunk = one full-file GET; COG uses `Range` GETs → the server MUST support Range.

---

## Task 1: deps + Range-capable byte-logging server + data generators

**Files:** `scripts/bench_requirements.txt` (append), `scripts/bench_remote_partialread.py` (new), `tests_bench/test_remote_gen.py`.

- [ ] **Step 1:** Append to `scripts/bench_requirements.txt` (pin to installed versions — check with `pip show`): `xarray`, `zarr`, `fsspec`, `aiohttp`, `requests`, `rioxarray`. Install into the venv.

- [ ] **Step 2: Range-capable byte-logging HTTP server.** A `ThreadingHTTPServer` handler that serves files from a root dir, supports `Range` (`206 Partial Content` with `Content-Range`), and records per-request `{path, bytes_sent, is_range}` into a shared accumulator with a `reset()`/`snapshot()` API. (Bare `SimpleHTTPRequestHandler` does NOT do Range — implement it, or use a tested Range handler.) Bind to port 0 (ephemeral); run in a daemon thread.

- [ ] **Step 3: Generators.** `generate_zarr(dir, shape=(4000,4000), chunks=(256,256), seed=…)` → a single 2D float32 variable with lat/lon coordinate arrays (so `.sel` works) written via `xarray.to_zarr` (zarr v2, consolidated metadata). `generate_cog(path, shape, blocksize=256)` → tiled GeoTIFF (EPSG:4326 so eider COG bbox pushdown binds — recall pushdown params are lat/lon and only bind for 4326 COGs). Windows helper: given a fraction (0.001/0.01/0.1) return a centered bbox in the grid's CRS.

- [ ] **Step 4: Tests** (`test_remote_gen.py`): server serves a file and a Range request returns 206 + correct bytes and the accumulator records them; generated Zarr reopens with xarray (right shape/chunks) and the COG reopens tiled EPSG:4326. Run `pytest tests_bench/test_remote_gen.py -q` → PASS. Commit `feat(bench): range-logging http server + remote store generators`.

---

## Task 2: contender runners + correctness gate

**Files:** `scripts/bench_remote_partialread.py`, `tests_bench/test_remote_runners.py`.

Each runner reads the window and returns `(values_summary, bytes_fetched, n_requests)` using the server accumulator (reset before, snapshot after).

- [ ] **Step 1: eider runner** — `read_geo('http://127.0.0.1:{port}/{store}', lon_min:=…, lat_min:=…, lon_max:=…, lat_max:=…)`; for COG same with the 4326 URL. Return the window's value array/stats (e.g. sorted values or count+sum+max for the gate) + bytes/requests from the accumulator.

- [ ] **Step 2: chunk-aware baseline** — Zarr: `xarray.open_zarr(fsspec.get_mapper('http://…'))` then `.sel(lat=slice(...), lon=slice(...)).load()`. COG: `rioxarray.open_rasterio('/vsicurl/http://…')` windowed read (or `rasterio` `Window`). Confirm via the accumulator it fetched a subset (not all chunks). Return same value summary + bytes/requests.

- [ ] **Step 3: naive baseline** — Zarr: open and `.load()` the whole array, then subset in memory. COG: read the full raster then subset. Return value summary + bytes/requests (≈ full store).

- [ ] **Step 4: correctness gate** — reuse/port `assert_agree`-style: the three contenders' window value summaries must match within tol (same cells, same values). Test (`test_remote_runners.py`, small 512×512 store, one 10% window): all three agree; eider & chunk-aware fetch FEWER bytes than naive. Run → PASS (eider genuinely reads over http; build extension first, set any needed env). Commit `feat(bench): eider/chunk-aware/naive remote read runners + gate`.

> CRS/coord note: eider COG bbox pushdown binds only for **EPSG:4326** (dims lat/lon); use 4326 for the COG so pushdown works. For Zarr, pushdown uses the coordinate arrays. Verify eider actually prunes (bytes ≪ naive) in the test — if a cell doesn't prune, that's a finding to report, not hide.

---

## Task 3: matrix + emit + CLI

**Files:** `scripts/bench_remote_partialread.py`.

- [ ] **Step 1:** `run_matrix` over format (zarr, cog) × window (0.001, 0.01, 0.1) × contender (eider, chunk_aware, naive): gate once per (format,window), then record bytes/requests/time (time = median of a few reps; bytes/requests deterministic so 1 measure). Emit a stdout table (cols: bytes, requests, time, + a "× vs naive" byte ratio) and `--json`. Include env block (versions) + the localhost caveat.
- [ ] **Step 2: CLI** `argparse`: `--out-dir`, `--windows`, `--shape`, `--reps`, `--json`, `--quick`. `--quick` runs a tiny store end-to-end (gate + table). Commit `feat(bench): remote partial-read matrix + emit`.

---

## Task 4: Controller captures (NOT a subagent)
- [ ] Controller runs the full harness, captures `/tmp/remote_results.json` with real bytes/requests/time for both formats × windows × contenders; confirms the gate passed. Keeps JSON for the doc.

---

## Task 5: Docs + the bundled follow-up fix

**Files:** `docs/docs/engineering/benchmarks.mdx`, `scripts/bench_zonal_headtohead.py`.

- [ ] **Step 1: Follow-up fix.** In `scripts/bench_zonal_headtohead.py`, the COG cell coords from `read_geo` are now CENTRES (extension fixed), so remove the `+dx/2 / -dy/2` shift in `_materialize_field` (use `x`, `y` directly; keep `dx/dy` for the cell box `±step/2`). Re-run its correctness gate at small scale (`--quick` or the correctness test) → must still PASS (the gate self-catches a wrong shift).
- [ ] **Step 2: Docs section** "Remote partial reads" in `benchmarks.mdx`, written from `/tmp/remote_results.json`: bytes/requests table per window; the honest framing (eider ≈ chunk-aware bytes, both ≪ naive; eider's edge = automatic spatial pushdown in SQL + uniform Zarr/COG/STAC); localhost/synthetic caveats; reproduction; cross-link spatial_pruning. `cd docs && npm run build` → `[SUCCESS]`.
- [ ] **Step 3: Commit** `docs: remote partial-read benchmark + fix zonal bench double-shift`.

---

## Task 6: Verification
- [ ] `python scripts/bench_remote_partialread.py --quick` end-to-end clean (gate passes).
- [ ] `python -m pytest tests_bench/ -q` (incl. the existing zonal tests + new remote tests) all pass — confirms the zonal bench fix didn't break its gate.
- [ ] `cd docs && npm run build` → `[SUCCESS]`.
- [ ] Scope: `git diff --name-status` (vs the COG-fix base / main) shows only `scripts/bench_remote_partialread.py`, `scripts/bench_requirements.txt`, `scripts/bench_zonal_headtohead.py` (the shift fix), `tests_bench/test_remote_*.py`, `docs/docs/engineering/benchmarks.mdx`, spec/plan. No production Rust changes.

---

## Self-review
- Honest baseline (naive ceiling + chunk-aware competitor) per the approved design — no strawman.
- Bytes/requests are the deterministic headline; localhost time is a labeled footnote.
- Correctness gate precedes any bytes/time; controller captures real numbers.
- The bundled follow-up (zonal bench double-shift) is verified by that script's own gate.
- 4326-COG note ensures eider pushdown actually binds; Zarr prunes via coord arrays. "Verify it prunes; if not, report it."
