# SQL Query Reference

The `read_zarr` table function is the core of Eider.

## Basic Syntax
```sql
SELECT time, lat, lon, value 
FROM read_zarr('s3://bucket/data.zarr');
```

## Spatial & Temporal Pushdown
Eider skips fetching entire Zarr chunks if they fall outside named bounding box parameters.

```sql
SELECT AVG(value)
FROM read_zarr(
    's3://bucket/data.zarr',
    lat_min := 45.0,
    lat_max := 55.0,
    time_min := '2020-01-01',
    time_max := '2020-12-31'
);
```

## Metadata Discovery
Before reading heavy data, inspect the Zarr metadata natively:
```sql
SELECT * FROM read_zarr_metadata('s3://bucket/data.zarr');
```
