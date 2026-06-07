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
