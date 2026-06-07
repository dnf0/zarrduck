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
of prompting on overwrite. Success in JSON mode:

```json
{"status": "success", "db": "/tmp/c_demo.duckdb"}
```

## Examples

```bash
eider extract climate_data.zarr/air_temperature scripts/demo_region.geojson --out analysis.duckdb --yes
```

## See also
- [resample](./cli_resample.md), [plot](./cli_plot.md), [shell](./cli_shell.md) — work with the extracted data.
