# Design: DuckDB Extension Demo GIF

- **Date:** 2026-06-06
- **Status:** Approved (design); implementation pending
- **Scope:** A new VHS-generated demo GIF showing the eider DuckDB extension used directly in SQL, to sit alongside the existing CLI demo GIF in the README/docs. No production code changes.

## Goal

The README currently has one demo GIF (`docs/static/img/demo-v2.gif`) showing the agentic `eider` **CLI** workflow. The extension's other audience — people who just want to `LOAD` it into DuckDB and run SQL over Zarr — has no visual. This adds a second GIF telling the **SQL-native Zarr analytics** story: load the extension into a plain DuckDB shell and query a Zarr array as if it were a table.

Success = a short, legible, deterministic GIF embedded in the README's "Quick Start (Reading)" section, visually cohesive with the existing CLI GIF, reproducible from a committed VHS tape.

## Decisions (from brainstorming)

| Decision | Choice |
|---|---|
| Core story | SQL-native Zarr analytics (load → inspect → analyze) |
| Data source | Local `climate_data.zarr` (offline, deterministic, matches the CLI demo's data) |
| Environment | Plain `duckdb` CLI session (not `eider shell`) — the authentic "extension in raw SQL" story |
| Storyboard | Tight 3-beat (~20–25s) |
| Placement | README "Quick Start (Reading)" section |

## Tooling, files & output

- **Tape:** `scripts/demo_extension.tape` (mirrors the structure of `scripts/demo.tape`).
- **Output:** `docs/static/img/demo-extension.gif` (alongside `demo-v2.gif`).
- **VHS settings** (match `demo.tape` for cohesion): `Theme "Dracula"`, `FontSize 16`, the same `FontFamily` stack, `Framerate 30`, `TypingSpeed 28ms`, `CursorBlink false`, `Width 1200`, `Height 900`, `Padding 18`, `LineHeight 1.12`.
- **Reuses** `scripts/header.sh` (the `header` function) for the cyan section titles, exactly as `demo.tape` does.

## Environment & hidden setup

The tape drives a **plain `duckdb` CLI** session. Hidden (pre-`Show`) setup frames:
1. Ensure the local data exists: run `scripts/generate_demo_data.py` if `climate_data.zarr` is absent.
2. Build the loadable extension: `cargo duckdb-ext build -o target/debug/eider.duckdb_extension -d v1.5.2 -- --no-default-features --features loadable-extension`.
3. Set a clean prompt (`export PS1='❯ '`), `source scripts/header.sh`, `clear`.
4. Appropriate `Sleep`s to let the build finish before recording the visible beats (mirrors `demo.tape`'s setup `Sleep 15s`).

### Prerequisites (documented; not enforced by the tape)

- **`duckdb` CLI v1.5.2** on `PATH` — must match the libduckdb version the `duckdb` crate bundles (`=1.10502.0` → runtime `v1.5.2`). A mismatched CLI cannot `LOAD` the version-locked extension (the same constraint `eider shell` has).
- **VHS** installed (`charmbracelet/vhs`).
- A POSIX shell environment with Python 3 (for data generation) and the Rust toolchain + `cargo-duckdb-ext-tools` (for the extension build).

## Storyboard (3 beats, ~20–25s)

Each beat uses `header '<n>. <title>'` for the section banner, matching `demo.tape`.

1. **Load the extension** — `header '1. Load the eider extension into DuckDB'`
   - Launch `duckdb -unsigned`.
   - `SET allow_unsigned_extensions = true;`
   - `LOAD 'target/debug/eider.duckdb_extension';`
   - (Matches the README "Quick Start (Reading)" snippet. If `-unsigned` already permits the load, the `SET` is shown for instructional parity with the README; verify the minimal working sequence during implementation.)

2. **Inspect the array as a SQL table** — `header '2. Inspect the array as a SQL table'`
   - `SELECT array_shape, chunk_shape, data_type, crs FROM read_zarr_metadata('climate_data.zarr/air_temperature');`
   - Expected to show the array shape (`[938, 73, 144]`), chunk shape, `Float32`, and CRS.

3. **Run analytical SQL over the Zarr array** — `header '3. Run analytical SQL directly over the Zarr array'`
   - A `read_geo(...)` query with a spatial bounding box + aggregation, ending on a clean result table, e.g.:
     ```sql
     SELECT lat, AVG(value) AS mean_temp
     FROM read_geo('climate_data.zarr/air_temperature', lat_min := 30, lat_max := 50)
     GROUP BY lat
     ORDER BY lat
     LIMIT 10;
     ```
   - Then `.exit`.

**Verification-required note:** the exact column names/types and parameter names in beats 2–3 (`read_zarr_metadata` output columns; `read_geo` value/coordinate column names; whether `time` needs `to_timestamp`) must be confirmed against real output from the freshly-built extension before recording. The query is illustrative; the implementer runs it manually first and adjusts so the recorded output is correct and clean. `read_geo` is the post-#114 name for the former `read_zarr`.

## Docs placement

- Embed `demo-extension.gif` in the README's **"Quick Start (Reading)"** section, immediately around the existing SQL snippet, with a caption distinguishing it from the top-of-README CLI GIF (e.g. *"Querying Zarr directly in DuckDB with the eider extension"*).
- Optionally surface the same GIF on the docs site's reading/extension page (low priority; README is the primary target).

## Verification

Not unit-testable. Verification steps:
1. Run each SQL beat manually against the local `climate_data.zarr` with the freshly-built extension; confirm output is correct and visually clean; adjust queries/column names as needed.
2. `vhs scripts/demo_extension.tape` renders without error and produces `docs/static/img/demo-extension.gif`.
3. Eyeball the GIF for timing, legibility, and that it loops cleanly.
4. Confirm the README embed renders.

Deterministic: data and extension are local; no network during recording.

## Non-goals

- No remote/cloud (s3://, http://) data in this GIF (deterministic local only).
- No demonstration of STAC/COG via `read_geo` (separate story; this GIF is Zarr-analytics focused).
- No automation of GIF generation in CI (generated manually/locally, like the existing demo).
- No changes to the existing CLI demo GIF.
