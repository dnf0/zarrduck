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
