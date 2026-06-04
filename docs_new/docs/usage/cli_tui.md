# CLI & TUI Guide

The `eider` CLI is an interactive data engine for spatial workflows.

## STAC Discovery (TUI)
Run `eider search` without arguments to launch the multi-level interactive catalog explorer.

```bash
# Filter geographically before launching the TUI
eider search --bbox -122.27,37.77,-122.22,37.81
```

## Extracting Data
Download intersecting chunks and materialize them into a local DuckDB file.
```bash
eider extract s3://bucket/data.zarr ./my_region.geojson --out analysis.duckdb
```

## Analytics & Shell
```bash
# Resample time-series data
eider resample analysis.duckdb monthly.duckdb --freq month --agg avg

# Interactive SQL shell with pre-loaded extensions
eider shell monthly.duckdb
```
