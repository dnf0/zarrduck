# Exporting Data

You can write materialized views back out to cloud storage as Zarr arrays using DuckDB's `COPY` command.

```sql
COPY (
    SELECT time, lat, lon, (temp_k - 273.15) AS temp_c 
    FROM read_zarr('s3://in/data.zarr')
) TO 's3://out/data.zarr' (FORMAT ZARR);
```
