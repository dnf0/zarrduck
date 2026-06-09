# Design: remote partial-read benchmark (eider chunk pruning over HTTP)

- **Date:** 2026-06-09
- **Status:** Approved (design); implementation pending
- **Scope:** A reproducible benchmark of eider's **cloud-native partial read** — fetching only the Zarr/COG chunks that intersect a query window over a network store — measured against a naive whole-array read and a *competent chunk-aware* baseline (xarray/zarr `.sel`, rasterio windowed `/vsicurl`). Headline metric is **bytes fetched + HTTP request count** (deterministic), wall-time secondary. Results go into `engineering/benchmarks`. This PR also fixes a follow-up from the COG centre-coords change.

## Why

The [zonal head-to-head](./2026-06-08-zonal-headtohead-design.md) deliberately gave every tool a **warm local raster** to isolate the compute kernel — which excluded eider's *other* advantage: never downloading the cells you don't need. This benchmark measures exactly that axis. The honest question: against a **competent** chunk-aware reader (not a strawman), does eider's pushdown actually cut bytes, and by how much?

Feasibility confirmed (spike, 2026-06-09): eider reads `read_geo('http://…/store.zarr/var', …)` via opendal `services-http`; a byte-logging local HTTP server shows a pruned read fetched **1 of 79 chunk files (0.39 MB vs 30.3 MB)**.

## Data (synthetic, seeded, generated)

- **Zarr**, single 2D variable, **chunked in lat/lon** (e.g. 4000×4000, chunks 256×256 → ~256 chunks) so a spatial bbox prunes to a handful of chunks. Plus a 3D `[time,lat,lon]` variant to also show time pruning (optional, if cheap).
- **COG**, tiled GeoTIFF (e.g. 4000×4000, 256-tile) for the rasterio `/vsicurl` comparison.
- Query windows at a few sizes (e.g. 0.1%, 1%, 10% of the grid area) so the bytes-vs-window-size curve is visible.

## Transport & measurement

- A **local HTTP server** serves the generated store from a temp dir, **logging per-request path + bytes + whether it was a `Range` request** (Zarr chunk = one full-file GET; COG = `Range` GETs within the `.tif`). The server must support **HTTP Range** (for COG/`/vsicurl` and any range-using reader) — use a Range-capable handler, not bare `SimpleHTTPRequestHandler`.
- **Primary metric:** total bytes fetched + number of HTTP requests (range or full), read straight from the server's accounting. Deterministic.
- **Secondary:** wall-clock (localhost, so it understates real-network latency wins — state this).

## Contenders (per window, per format)

| Tool | Zarr | COG | Reads only intersecting chunks? |
|---|---|---|---|
| **eider** | `read_geo(http URL, bbox)` | `read_geo(http URL, bbox)` | yes (pushdown) |
| **chunk-aware baseline** | `xarray.open_zarr(fsspec http) .sel(window)` | `rioxarray`/`rasterio` windowed read over `/vsicurl` | yes |
| **naive** | open + read whole array, then subset in memory | read full raster, then subset | no (fetches everything) |

**Correctness gate (before any bytes/time are reported):** all three return the **same values** for the window (max abs diff within tol). A mismatch is a finding, not hidden — mirrors the zonal benchmark's gate.

## Expected (honest) shape of the result

eider and the chunk-aware baseline should fetch **comparable bytes** (both pull the intersecting chunks); both **far below** naive. eider's distinguishing value is doing the **bbox→chunk pruning automatically from a spatial query in SQL** (the xarray path needs hand-computed index ranges or coordinate `.sel`; rasterio needs an explicit window), and uniform handling across Zarr/COG/STAC. The writeup must say so plainly — if eider merely *matches* chunk-aware bytes, that is the honest finding, with the ergonomic/integration win called out separately, not dressed up as a byte-savings win.

## Components

1. **`scripts/bench_remote_partialread.py`** — generate store(s) → start byte-logging Range-capable HTTP server → per (format, window, contender): correctness gate, then bytes/requests/time → emit table (stdout) + `--json`. Reuses `scripts/bench_requirements.txt` (+ `xarray`, `zarr`, `fsspec`, `aiohttp`/`requests`, `rioxarray` — pin them).
2. **Docs:** a "Remote partial reads" section in `docs/docs/engineering/benchmarks.mdx` — bytes/request table per window, the naive-vs-chunk-aware-vs-eider story, the honest "eider matches chunk-aware bytes; its edge is automatic spatial pushdown" framing, localhost/synthetic caveats, reproduction. Cross-link [Spatial Pruning](../engineering/spatial_pruning.mdx).
3. **Follow-up fix (bundled per request):** `scripts/bench_zonal_headtohead.py` currently shifts `read_geo` COG coords by `+dx/2` to centres; now that the extension returns centres (COG centre-coords fix), drop that shift so it doesn't double-correct. Re-run that script's correctness gate at small scale to confirm it still agrees (the gate self-catches a wrong shift).

## Honesty guardrails

- Bytes/time reported only after the correctness gate passes per cell.
- The controller runs the final capture and writes the doc from captured JSON (not estimates).
- "naive" is labeled as a reference ceiling, NOT eider's real competitor — the chunk-aware baseline is.
- localhost removes network latency, so the **bytes** number is the real story; time is contextual. Stated explicitly.
- If a tool can't run a cell, mark skipped + why.

## Non-goals

- Real S3 / public buckets (local HTTP only — reproducible, deterministic bytes).
- Modeling network latency/throughput; we report bytes + requests, with localhost time as a footnote.
- The compute kernel (covered by the zonal head-to-head).
- STAC partial reads (Zarr + COG only here).
