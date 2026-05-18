# Zarrduck CLI

The `zarrduck` CLI is an Agentic Spatial Data Engine. It allows users and LLM agents to easily discover, extract, and manipulate GeoZarr data directly from the terminal without writing complex spatial SQL.

## Commands

### Discovery: `info`
Quickly inspect the shape, chunking, and Coordinate Reference System (CRS) of a remote Zarr array.

```bash
zarrduck info s3://my-bucket/climate.zarr
```

**Agent Mode:** Use `--output=json` to get a clean, parseable JSON response.
```bash
zarrduck info s3://my-bucket/climate.zarr --output=json
```

### Extraction: `extract`
Perform a Vector-Raster join (zonal extraction). This command downloads only the Zarr chunks that intersect with your vector boundaries, masks the pixels exactly to the polygons, and saves the data to a local DuckDB file.

```bash
zarrduck extract s3://my-bucket/climate.zarr ./my_region.geojson --out analysis.duckdb
```

### Analytics: `shell`
Drop into an interactive DuckDB REPL pre-loaded with the `spatial` and `duckdb_geozarr` extensions.

```bash
zarrduck shell analysis.duckdb
```
