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
