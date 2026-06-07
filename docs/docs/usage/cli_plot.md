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
