---
sidebar_position: 3
---

# read_zarr_metadata

Inspect a Zarr array's structure without reading any chunk data.

## Signature

```sql
read_zarr_metadata(uri VARCHAR)
```

## Output

| Column | Type | Description |
|---|---|---|
| `array_shape` | `VARCHAR` | Full array dimensions, e.g. `[938, 73, 144]`. |
| `chunk_shape` | `VARCHAR` | Chunk dimensions, e.g. `[12, 73, 144]`. |
| `data_type` | `VARCHAR` | Element type, e.g. `Float32`. |
| `crs` | `VARCHAR` | Coordinate reference system, e.g. `EPSG:4326` (`UNKNOWN` if none is declared). |

## Example

```sql
SELECT array_shape, chunk_shape, data_type, crs
FROM read_zarr_metadata('climate_data.zarr/air_temperature');
```

```
┌────────────────┬───────────────┬───────────┬───────────┐
│  array_shape   │  chunk_shape  │ data_type │    crs    │
│    varchar     │    varchar    │  varchar  │  varchar  │
├────────────────┼───────────────┼───────────┼───────────┤
│ [938, 73, 144] │ [12, 73, 144] │ Float32   │ EPSG:4326 │
└────────────────┴───────────────┴───────────┴───────────┘
```

## See also

- [read_geo](./sql_read_geo.md) — read the array's data.
