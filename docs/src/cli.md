# Eider CLI

The `eider` CLI is an Agentic Spatial Data Engine. It allows users and LLM agents to easily discover, extract, and manipulate GeoZarr data directly from the terminal without writing complex spatial SQL.

## Commands

### Discovery: `search`
Search modern STAC APIs for cloud-native Zarr data.

```bash
eider search --bbox -122.27,37.77,-122.22,37.81
```
**Interactive TUI Explorer:** For human users, the CLI features a powerful multi-level interactive menu. If run without explicitly specifying an `--api` or `--collection`, it guides you through selecting a Provider, a Collection, and a Dataset URI. It parses STAC metadata to provide rich descriptions and includes a **smart multi-word filter**—just type keywords separated by spaces to instantly drill down large catalogs!

### Introspection: `info`
Quickly inspect the shape, chunking, and Coordinate Reference System (CRS) of a remote Zarr array or Group. If pointed at a Group, it will present an interactive menu to let you select which specific array to load.

```bash
eider info s3://my-bucket/climate.zarr
```

### Extraction: `extract`
Perform a Vector-Raster join (zonal extraction). This command downloads only the Zarr chunks that intersect with your vector boundaries, masks the pixels exactly to the polygons, and saves the data to a local DuckDB file.

```bash
eider extract climate_data.zarr/air_temperature ./my_region.geojson --out analysis.duckdb
```

### Analytics: `resample`
Automatically temporally aggregate high-frequency time-series data into coarser buckets (e.g., convert hourly data to monthly means). It intelligently detects numeric time columns and dynamically casts them for you.

```bash
eider resample analysis.duckdb monthly.duckdb --freq month --agg avg
```

### Analytics: `shell`
Drop into an interactive DuckDB REPL pre-loaded with the `spatial` and `eider` extensions.

```bash
eider shell monthly.duckdb
```

## Agent Mode
To bypass all interactive prompts, spinners, and menus when automating with an LLM agent, simply append `--output=json` to any command. The CLI will return clean, parseable JSON payloads and explicitly error out if a required parameter is missing instead of hanging on an interactive prompt.
