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
