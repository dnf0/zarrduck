---
sidebar_position: 4
---

# CLI Quickstart

Go from a Zarr array to analysis with the `eider` CLI in about five minutes. See
[Installation](./installation.md) to get the CLI (and the extension it loads).

Using the repo's sample data:

```bash
python3 scripts/generate_demo_data.py   # writes ./climate_data.zarr
```

## 1. Inspect a dataset

```bash
eider info climate_data.zarr/air_temperature
```

## 2. Extract data intersecting a region

`extract` downloads only the intersecting chunks and joins them with your vector
polygons into a local DuckDB file:

```bash
eider extract climate_data.zarr/air_temperature scripts/demo_region.geojson \
  --out analysis.duckdb --yes
```

## 3. Explore the result

```bash
# interactive SQL shell over the extracted data
eider shell analysis.duckdb

# or render an ASCII chart
eider plot analysis.duckdb
```

## Agent mode

Every command accepts `--output=json` for machine-readable output, so the CLI is
drop-in for LLM agents and scripts:

```bash
eider info climate_data.zarr/air_temperature --output=json
```

## Next steps

- **CLI Reference** — every subcommand and flag.
- **Guides** — temporal resampling, exporting.
