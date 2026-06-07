# Docs Workstream C (CLI Reference) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a complete `eider` CLI reference — a landing page (invocation + global `--output`/JSON agent mode + command index) plus one page per subcommand — replacing the thin `cli_tui` stub.

**Architecture:** Rewrite `docs/docs/usage/cli_tui.md` as the CLI Reference landing and add nine `cli_<command>.md` pages under `docs/docs/usage/`, then list them in the "CLI Reference" sidebar category. Docs-only: no CLI code changes. Every flag is reconciled against the real `eider <cmd> --help` and JSON examples are captured from live runs.

**Tech Stack:** Docusaurus (Markdown + TS sidebar), the `eider` CLI + extension, DuckDB.

---

## Conventions & verified facts

Work from repo root `/Users/danielfisher/repos/zarrduck` on branch `docs/workstream-c-cli-reference` (do NOT commit to `main`). Conventional Commits, `--no-gpg-sign`, end commit messages with `Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>`.

Command surface (from `cli/src/main.rs` clap defs — the canonical source is `eider <cmd> --help`, which Task 1 captures):

| Command | Positional | Options |
|---|---|---|
| `info` | `uri` | `--pin DIM=INDEX` (repeatable) |
| `extract` | `zarr_uri`, `vector_path` | `--out`, `-y/--yes`, `--pin` |
| `shell` | `db_path` | — |
| `export` | — | `--db`, `--query`, `--dest`, `--value-column`, `--chunks` |
| `completions` | `shell` | — |
| `search` | — | `--api`, `--collection`, `--bbox`, `--datetime` |
| `resample` | `input_db`, `output_db` | `--freq`, `--agg` |
| `plot` | `db_path` | `--plot-type`, `--table` (default `extracted_data`), `--value`, `--group-by`, `--pin` |
| `ingest` | `input_file`, `output_zarr_uri` | `--chunks`, `--value-column` |

Global: `--output table|json`. The CLI is dual-mode: interactive TUI (inquire prompts) for humans, `--output=json` for agents/scripts. JSON envelopes: success `{"status": "success", …}` (varies by command), error `{"status":"error","message":"…"}`.

Build the binary + extension once for live capture/verification:
```bash
cargo build -p eider
cargo duckdb-ext build -o target/debug/eider.duckdb_extension -d v1.5.2 -- --no-default-features --features loadable-extension
python3 scripts/generate_demo_data.py   # if climate_data.zarr is missing
export PATH="$PWD/target/debug:$PATH"     # so `eider` resolves to the built binary
```
Docs build: `cd docs && (test -d node_modules || npm ci) && npm run build` (`onBrokenLinks: 'throw'`).

## File structure

- Modify: `docs/docs/usage/cli_tui.md` — rewrite as the CLI Reference landing.
- Create: `docs/docs/usage/cli_info.md`, `cli_search.md`, `cli_extract.md`, `cli_ingest.md`, `cli_export.md`, `cli_resample.md`, `cli_plot.md`, `cli_shell.md`, `cli_completions.md`.
- Modify: `docs/sidebars.ts` — "CLI Reference" category lists the landing + nine pages.

---

## Task 1: Capture the real CLI surface (no commit)

**Files:** none (produces verified help text + JSON envelopes used by all page tasks).

- [ ] **Step 1: Build and capture every command's help**

```bash
cargo build -p eider
export PATH="$PWD/target/debug:$PATH"
for c in info extract shell export completions search resample plot ingest; do echo "===== $c ====="; eider "$c" --help; done
eider --help
```
Record each command's exact args/flags/descriptions. If any flag differs from the table in "Conventions" above, the real `--help` wins — note the differences for the page tasks.

- [ ] **Step 2: Capture key JSON envelopes**

```bash
cargo duckdb-ext build -o target/debug/eider.duckdb_extension -d v1.5.2 -- --no-default-features --features loadable-extension >/dev/null 2>&1
python3 scripts/generate_demo_data.py >/dev/null 2>&1 || true
# info (success envelope)
eider info climate_data.zarr/air_temperature --output=json
# resample error envelope (missing input)
eider resample missing.duckdb out.duckdb --freq year --agg avg --output=json
# extract success envelope (into a temp db)
rm -f /tmp/c_demo.duckdb; eider extract climate_data.zarr/air_temperature scripts/demo_region.geojson --out /tmp/c_demo.duckdb --yes --output=json
```
Record the exact JSON for `info` (success), `resample` (error envelope shape), and `extract` (success). These feed the page examples.

---

## Task 2: Rewrite the landing (`cli_tui.md`)

**Files:** Modify `docs/docs/usage/cli_tui.md`.

- [ ] **Step 1: Write the landing**

Overwrite `docs/docs/usage/cli_tui.md`:

```markdown
---
sidebar_position: 1
---

# CLI Reference

The `eider` CLI is a spatial data engine for GeoZarr and DuckDB. It works two ways:

- **Interactive (TUI):** run a command with missing inputs and it prompts you
  with menus (provider/collection pickers, resampling options, plot types, …).
- **Agent / scripting:** pass `--output=json` for machine-readable output and
  fully non-interactive behavior.

See [Installation](./installation.md) to get the CLI.

## Global options

| Option | Description |
|---|---|
| `--output table` | Human-readable output (default). |
| `--output json` | Machine-readable JSON; suppresses interactive prompts (required inputs must be passed as flags). |

In JSON mode, every command emits a status envelope. On failure:

```json
{ "status": "error", "message": "<what went wrong>" }
```

On success the shape depends on the command (documented per page).

## Commands

| Command | Purpose |
|---|---|
| [`info`](./cli_info.md) | Inspect a Zarr array's metadata. |
| [`search`](./cli_search.md) | Discover GeoZarr/COG assets via a STAC API. |
| [`extract`](./cli_extract.md) | Extract array data intersecting vector polygons into DuckDB. |
| [`ingest`](./cli_ingest.md) | Convert a legacy file (NetCDF/GeoTIFF/CSV) to GeoZarr. |
| [`export`](./cli_export.md) | Write a DuckDB query result out to a Zarr array. |
| [`resample`](./cli_resample.md) | Temporally resample extracted data. |
| [`plot`](./cli_plot.md) | Render an ASCII plot from a DuckDB file. |
| [`shell`](./cli_shell.md) | Open a DuckDB shell preloaded with the extension. |
| [`completions`](./cli_completions.md) | Generate shell completion scripts. |

For end-to-end workflows that chain these commands, see the Guides section.
```

(The "Guides section" reference is intentionally plain text — Guides are workstream D and not yet linkable.)

- [ ] **Step 2: Commit**

```bash
git add docs/docs/usage/cli_tui.md
git commit --no-gpg-sign -m "docs: rewrite CLI reference landing page

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 3: Discovery pages (`cli_info.md`, `cli_search.md`)

**Files:** Create `docs/docs/usage/cli_info.md`, `docs/docs/usage/cli_search.md`.

- [ ] **Step 1: Write `cli_info.md`**

```markdown
---
sidebar_position: 2
---

# eider info

Inspect a Zarr array's metadata (shape, chunking, type, CRS) without reading data.

## Synopsis

```
eider info <uri> [--pin DIM=INDEX]... [--output table|json]
```

## Arguments

- `uri` — the Zarr array URI (local path, `s3://`, or `http(s)://`).

## Options

| Option | Description |
|---|---|
| `--pin DIM=INDEX` | Pin a dimension to a fixed index (repeatable), e.g. `--pin time=0`. |

## Examples

```bash
eider info climate_data.zarr/air_temperature
```

JSON mode returns the array metadata:

```bash
eider info climate_data.zarr/air_temperature --output=json
```

<!-- VERIFY: paste the real JSON from Task 1 Step 2 (info success envelope) -->

## See also
- [read_zarr_metadata](./sql_read_zarr_metadata.md) — the same metadata from SQL.
```

- [ ] **Step 2: Write `cli_search.md`**

```markdown
---
sidebar_position: 3
---

# eider search

Discover GeoZarr / COG assets from a STAC API. Run interactively to pick a
provider and collection, or pass flags to script it.

## Synopsis

```
eider search [--api URL] [--collection ID] [--bbox MIN_LON,MIN_LAT,MAX_LON,MAX_LAT] [--datetime RANGE] [--output table|json]
```

## Options

| Option | Description |
|---|---|
| `--api URL` | STAC API root, e.g. `https://planetarycomputer.microsoft.com/api/stac/v1`. Prompted if omitted (TUI). |
| `--collection ID` | Collection to search, e.g. `era5-pds`. Prompted if omitted (TUI). |
| `--bbox` | Bounding box `min_lon,min_lat,max_lon,max_lat`. |
| `--datetime` | Datetime range, e.g. `2020-01-01T00:00:00Z/2020-12-31T23:59:59Z`. |

## Behavior

In interactive mode, `search` presents provider and collection pickers, then a
dataset selector. In `--output=json` mode it requires `--api` and `--collection`
and prints the matching STAC feature URIs as `{"status":"success","uris":[…]}`.

> **Note:** `search` currently emits each matching STAC feature's self link.
> Reading STAC items directly via `read_geo` is experimental (see the
> [SQL Reference](./sql_reference.md#source-uris)).

## Examples

```bash
eider search --bbox -122.27,37.77,-122.22,37.81
eider search --api https://example.com/stac --collection era5-pds --output=json
```
```

- [ ] **Step 3: Reconcile with Task 1 captures**

Replace the `<!-- VERIFY -->` in `cli_info.md` with the real `info --output=json` envelope from Task 1 Step 2. Reconcile both pages' options/synopsis against the captured `eider info --help` / `eider search --help`. No `<!-- VERIFY -->` may remain.

- [ ] **Step 4: Commit**

```bash
git add docs/docs/usage/cli_info.md docs/docs/usage/cli_search.md
git commit --no-gpg-sign -m "docs: add info and search CLI pages

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 4: Extraction & conversion pages (`cli_extract.md`, `cli_ingest.md`, `cli_export.md`)

**Files:** Create the three pages under `docs/docs/usage/`.

- [ ] **Step 1: Write `cli_extract.md`**

```markdown
---
sidebar_position: 4
---

# eider extract

Extract array cells intersecting vector polygons into a local DuckDB table
(`extracted_data`), fetching only the chunks the polygons touch.

## Synopsis

```
eider extract <zarr_uri> <vector_path> [--out FILE] [-y|--yes] [--pin DIM=INDEX]... [--output table|json]
```

## Arguments

- `zarr_uri` — the Zarr array URI.
- `vector_path` — path to vector boundaries (GeoJSON, Shapefile).

## Options

| Option | Description |
|---|---|
| `--out FILE` | Output DuckDB file. Falls back to `default_out` from config if omitted. |
| `-y`, `--yes` | Bypass the extraction-plan and overwrite confirmation prompts. |
| `--pin DIM=INDEX` | Pin a dimension to a fixed index (repeatable). |

## Behavior

In human mode, `extract` prints an extraction plan (chunk count, estimated data
volume) and asks for confirmation, and prompts before overwriting an existing
output. `--yes` skips both; `--output=json` is non-interactive and errors instead
of prompting on overwrite. Success in JSON mode: `{"status":"success","db":"<path>"}`.

## Examples

```bash
eider extract climate_data.zarr/air_temperature scripts/demo_region.geojson --out analysis.duckdb --yes
```

<!-- VERIFY: paste the real extract --output=json success envelope from Task 1 Step 2 -->

## See also
- [resample](./cli_resample.md), [plot](./cli_plot.md), [shell](./cli_shell.md) — work with the extracted data.
```

- [ ] **Step 2: Write `cli_ingest.md`**

```markdown
---
sidebar_position: 5
---

# eider ingest

Convert a legacy spatial file (NetCDF, GeoTIFF, CSV with geometry) to a GeoZarr array.

## Synopsis

```
eider ingest <input_file> <output_zarr_uri> [--chunks JSON] [--value-column NAME] [--output table|json]
```

## Arguments

- `input_file` — the local file to convert.
- `output_zarr_uri` — destination Zarr URI.

## Options

| Option | Description |
|---|---|
| `--chunks JSON` | Override auto chunk sizes, e.g. `'{"time": 30}'`. |
| `--value-column NAME` | Name of the value column (defaults to `value`). |

## Examples

```bash
eider ingest input.geojson out.zarr --value-column value
```

Success in JSON mode: `{"status":"success","uri":"<output_zarr_uri>"}`.
```

- [ ] **Step 3: Write `cli_export.md`**

```markdown
---
sidebar_position: 6
---

# eider export

Write the result of a DuckDB SQL query out to a Zarr array. Coordinate columns
must be 0-based integer dimension indices; all other-than-value columns are treated
as coordinates.

## Synopsis

```
eider export --query SQL --dest URI --value-column NAME [--db FILE] [--chunks JSON] [--output table|json]
```

## Options

| Option | Description |
|---|---|
| `--query SQL` | The SQL query to execute. **Required.** |
| `--dest URI` | Destination Zarr path, e.g. `s3://bucket/output.zarr`. **Required.** |
| `--value-column NAME` | The column holding the values; all others are coordinates. **Required.** |
| `--db FILE` | DuckDB database to query (in-memory if omitted). |
| `--chunks JSON` | Dimension→chunk-size map, e.g. `'{"time": 10}'`. |

## Example

```bash
eider export --db src.duckdb --query "SELECT * FROM src" --dest out.zarr --value-column value
```

> Note: `--dest` is the destination flag (it does not collide with the global `--output` format flag).
```

- [ ] **Step 4: Reconcile + commit**

Replace the `<!-- VERIFY -->` in `cli_extract.md` with the real extract JSON envelope from Task 1 Step 2; reconcile all three pages' synopsis/options against the captured `--help`. Then:

```bash
git add docs/docs/usage/cli_extract.md docs/docs/usage/cli_ingest.md docs/docs/usage/cli_export.md
git commit --no-gpg-sign -m "docs: add extract, ingest, export CLI pages

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 5: Analysis pages (`cli_resample.md`, `cli_plot.md`)

**Files:** Create the two pages under `docs/docs/usage/`.

- [ ] **Step 1: Write `cli_resample.md`**

```markdown
---
sidebar_position: 7
---

# eider resample

Temporally resample an extracted-data DuckDB file into a coarser frequency,
writing a `resampled_data` table.

## Synopsis

```
eider resample <input_db> <output_db> [--freq FREQ] [--agg AGG] [--output table|json]
```

## Arguments

- `input_db` — DuckDB file containing the `extracted_data` table.
- `output_db` — destination DuckDB file.

## Options

| Option | Description |
|---|---|
| `--freq FREQ` | Temporal frequency: `hour`, `day`, `week`, `month`, `year`. Prompted if omitted (TUI). |
| `--agg AGG` | Aggregate: `avg`, `min`, `max`, `sum`, `count`, `median`, `mode`, `stddev`, `variance`. Prompted if omitted (TUI). |

In `--output=json` mode, `--freq` and `--agg` are required (no prompts). Success: `{"status":"success","db":"<output_db>"}`.

## Example

```bash
eider resample analysis.duckdb monthly.duckdb --freq month --agg avg
```
```

- [ ] **Step 2: Write `cli_plot.md`**

```markdown
---
sidebar_position: 8
---

# eider plot

Render an ASCII plot from a DuckDB file in the terminal.

## Synopsis

```
eider plot <db_path> [--plot-type TYPE] [--table NAME] [--value COL] [--group-by COL] [--pin DIM=INDEX]...
```

## Arguments

- `db_path` — the DuckDB database file.

## Options

| Option | Description |
|---|---|
| `--plot-type TYPE` | `hist`, `heatmap`, or `line`. Prompted if omitted (TUI). |
| `--table NAME` | Table to query (default `extracted_data`). |
| `--value COL` | Value column to aggregate (auto-detected if omitted). |
| `--group-by COL` | Optional column to group by. |
| `--pin DIM=INDEX` | Pin a dimension to a fixed index (repeatable). |

## Example

```bash
eider plot analysis.duckdb --plot-type heatmap --value air_temperature
```
```

- [ ] **Step 3: Reconcile against `--help` (Task 1) and commit**

Confirm the `--freq`/`--agg` allowed values and the `plot` types/`--table` default match the captured help and the code. Then:

```bash
git add docs/docs/usage/cli_resample.md docs/docs/usage/cli_plot.md
git commit --no-gpg-sign -m "docs: add resample and plot CLI pages

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 6: Utility pages (`cli_shell.md`, `cli_completions.md`)

**Files:** Create the two pages under `docs/docs/usage/`.

- [ ] **Step 1: Write `cli_shell.md`**

```markdown
---
sidebar_position: 9
---

# eider shell

Open an interactive DuckDB shell against a database file, preloading the DuckDB
`spatial` extension and (when the local `duckdb` CLI version matches the bundled
build) the eider extension.

## Synopsis

```
eider shell <db_path>
```

## Arguments

- `db_path` — the DuckDB database file to open.

## Notes

Requires the `duckdb` CLI on your `PATH`. If the local `duckdb` version differs
from the version the eider extension was built against, the shell still opens but
the eider extension is not loaded (a notice is printed).

## Example

```bash
eider shell analysis.duckdb
```
```

- [ ] **Step 2: Write `cli_completions.md`**

```markdown
---
sidebar_position: 10
---

# eider completions

Generate a shell completion script and print it to stdout.

## Synopsis

```
eider completions <shell>
```

## Arguments

- `shell` — one of `bash`, `zsh`, `fish`, `powershell`, `elvish`.

## Example

```bash
# bash: load completions for the current session
source <(eider completions bash)

# or install persistently (bash)
eider completions bash > ~/.local/share/bash-completion/completions/eider
```
```

- [ ] **Step 3: Verify the completions shell list against `--help` and commit**

Confirm the accepted `shell` values from `eider completions --help` (clap_complete::Shell). Adjust the list if it differs. Then:

```bash
git add docs/docs/usage/cli_shell.md docs/docs/usage/cli_completions.md
git commit --no-gpg-sign -m "docs: add shell and completions CLI pages

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 7: Wire the sidebar

**Files:** Modify `docs/sidebars.ts`.

- [ ] **Step 1: Update the CLI Reference category**

In `docs/sidebars.ts`, replace the `CLI Reference` category's `items` with:

```typescript
      items: [
        'usage/cli_tui',
        'usage/cli_info',
        'usage/cli_search',
        'usage/cli_extract',
        'usage/cli_ingest',
        'usage/cli_export',
        'usage/cli_resample',
        'usage/cli_plot',
        'usage/cli_shell',
        'usage/cli_completions',
      ],
```

(Leave the other four categories unchanged.)

- [ ] **Step 2: Commit**

```bash
git add docs/sidebars.ts
git commit --no-gpg-sign -m "docs: wire CLI reference pages into the sidebar

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 8: Build verification

**Files:** none (verification only).

- [ ] **Step 1: Build the docs (gates broken links)**

Run: `cd docs && (test -d node_modules || npm ci) && npm run build 2>&1 | tail -6`
Expected: `[SUCCESS]`, no "Broken link" errors. CLI pages cross-link to each other, to `installation.md`, and to SQL Reference pages (`sql_read_zarr_metadata.md`, `sql_reference.md`) — all of which exist. The "Guides section" reference on the landing is plain text (not a link).

- [ ] **Step 2: No stale/placeholder content**

Run: `grep -rnE "<!-- VERIFY|--output <OUTPUT>|read_zarr\(" docs/docs/usage/cli_*.md || echo clean`
Expected: `clean` (no leftover VERIFY markers; `export` documents `--dest`, not `--output`).

- [ ] **Step 3: Confirm scope (no out-of-scope edits)**

Run: `git diff --name-status main..HEAD`
Expected: `M docs/docs/usage/cli_tui.md`, `M docs/sidebars.ts`, `A` for the nine new `cli_*` pages, plus the spec/plan docs — and nothing else (no Rust changes, no edits to Getting Started / SQL Reference / engineering pages).

---

## Self-review notes (author checklist, already applied)

- **Spec coverage:** landing with global `--output`/JSON envelope + command index (Task 2) ✓; per-command pages for all nine (Tasks 3–6) ✓; consistent synopsis/args/options/behavior/example template ✓; interactive-prompt behavior documented where applicable (search/extract/resample/plot) ✓; `--output=json` envelopes documented + the key ones captured live (Task 1 → Tasks 3,4) ✓; sidebar wiring (Task 7) ✓; accuracy via `--help` reconciliation + build gate (Tasks 1, 8) ✓; `cli_tui.md` rewritten in place, no file moves (Task 8 Step 3) ✓.
- **Accuracy specifics honored:** `export --dest` (Task 4), `extract --out/-y/--pin` (Task 4), `plot --table` default `extracted_data` (Task 5), `completions` positional shell (Task 6), `search` self-link behavior + experimental `read_geo` STAC cross-reference (Task 3).
- **Placeholders:** the only `<!-- VERIFY -->` markers are in `cli_info.md`/`cli_extract.md` JSON-example slots, with explicit reconcile steps (Task 3 Step 3, Task 4 Step 4) to replace them with captured real output; Task 8 Step 2 enforces none remain.
- **Link safety:** CLI cross-links and links to existing Getting Started / SQL Reference pages only; Guides reference is plain text (D not yet built).
- **Non-goals honored:** no workflow tutorials (D), no SQL/engineering edits, no CLI code changes, no file moves.
