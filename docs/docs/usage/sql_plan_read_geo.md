---
sidebar_position: 4
---

# plan_read_geo

Estimate the cost of a `read_geo` query — how many chunks and bytes it would
fetch — without reading any data. Useful as a dry-run before a large read.

## Signature

```sql
plan_read_geo(uri VARCHAR, [named parameters])
```

Accepts the same named parameters as [read_geo](./sql_read_geo.md), so the
estimate reflects spatial/temporal pruning.

## Output

| Column | Type | Description |
|---|---|---|
| `total_chunks` | `BIGINT` | Number of chunks the query would fetch. |
| `total_bytes` | `BIGINT` | Estimated bytes those chunks occupy. |

## Example

```sql
SELECT total_chunks, total_bytes
FROM plan_read_geo('climate_data.zarr/air_temperature', lat_min := 30, lat_max := 50);
```

```
┌──────────────┬─────────────┐
│ total_chunks │ total_bytes │
│    int64     │    int64    │
├──────────────┼─────────────┤
│           79 │    39861504 │
└──────────────┴─────────────┘
```

## See also

- [read_geo](./sql_read_geo.md) — run the actual read.
