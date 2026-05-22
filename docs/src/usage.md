# Usage Guide

The extension exposes a single, powerful table function: `read_zarr`.

## Basic Querying

To read a Zarr array from a local file path:

```sql
SELECT * FROM read_zarr('/path/to/my_data.zarr');
```

The extension automatically inspects the Zarr `_ARRAY_DIMENSIONS` metadata to determine the schema. For an array with dimensions `[time, lat, lon]`, it will automatically look for corresponding 1D coordinate arrays (`/time`, `/lat`, `/lon`) in the same store and yield four columns:
`time`, `lat`, `lon`, and `value`.

If a coordinate array does not exist, the extension gracefully falls back to yielding the raw integer index for that dimension (e.g., `0, 1, 2...`).

## GeoZarr Metadata & Spatial Projections

Zarrduck aligns with the official GeoZarr specification.

### Global Metadata

You can query dataset-level properties such as the coordinate reference system (CRS), array shape, and data types using the `read_zarr_metadata` function:

```sql
SELECT * FROM read_zarr_metadata('s3://my-bucket/climate.zarr');
```
This returns a single row containing columns like `array_shape`, `chunk_shape`, `data_type`, and `crs`.

### Spatial Affine Transforms

When scanning a table, the extension parses GeoZarr `spatial` metadata (affine transformations). For dimensions mapped to a spatial transform, the extension automatically performs the math (`translation + index * scale`) on the fly.

This means you can query `lon` and `lat` directly as projected geographic coordinates rather than dealing with raw integer grid indices.

## Cloud Storage (S3 / HTTP)

You can read directly from AWS S3 or public HTTP endpoints. The extension dynamically resolves the backend using Apache OpenDAL.

```sql
-- Read from an S3 bucket
SELECT count(value) FROM read_zarr('s3://my-bucket/climate-data.zarr');

-- Read from a public HTTP endpoint
SELECT AVG(value) FROM read_zarr('https://public-data.com/dataset.zarr');
```

### AWS Credentials
When querying `s3://` URIs, the extension automatically looks for standard AWS environment variables. Ensure these are set in the terminal environment where DuckDB is launched:
- `AWS_ACCESS_KEY_ID`
- `AWS_SECRET_ACCESS_KEY`
- `AWS_REGION`
- `AWS_SESSION_TOKEN` (optional)

*Note: The extension manages its own cloud connections via OpenDAL, so DuckDB's internal `CREATE SECRET` or `httpfs` configurations do not apply here.*

## Spatial Pruning (Network Optimization)

Due to limitations in current DuckDB extension bindings, standard `WHERE` clauses (e.g., `WHERE lat > 45.0`) cannot be "pushed down" to the network layer. If you use a `WHERE` clause, DuckDB will download the *entire* Zarr array from S3 and filter the rows in memory—which is disastrous for performance.

To solve this, use **Named Parameters**.

You can pass `_min` and `_max` parameters for any coordinate dimension. The extension will perform a binary search on the cached coordinate arrays and **strictly prune out-of-bounds chunks before performing network I/O**.

```sql
SELECT
    time,
    AVG(value) as regional_average
FROM read_zarr(
    's3://my-bucket/climate.zarr',
    lat_min := 45.0,
    lat_max := 55.0,
    lon_min := -10.0,
    lon_max := 5.0,
    time_min := 1609459200 -- Epoch timestamp
)
GROUP BY time;
```
In this query, only the chunks that intersect the bounding box of `lat`, `lon`, and `time` are downloaded. All other S3 requests are bypassed.

## Projection Pushdown

DuckDB is a columnar engine. If your query does not explicitly request all columns, the extension optimizes its workload by skipping the calculation and memory allocation for the ignored columns.

```sql
-- The coordinate columns (time, lat, lon) are completely ignored.
-- The extension will only yield the 'value' column.
SELECT SUM(value) FROM read_zarr('s3://bucket/data.zarr');
```
This saves significant CPU overhead and memory bandwidth during massive table scans.

## Missing Data and SQL NULLs

In Zarr, missing data is often represented by a `fill_value` metadata field (e.g., `-9999.0`). The extension reads this metadata and natively maps matching values to true SQL `NULL`s using DuckDB's `ValidityMask`.

This guarantees that standard SQL aggregations work correctly out of the box:
```sql
-- NULLs (fill values) are correctly ignored in the average calculation
SELECT AVG(value) FROM read_zarr('s3://bucket/data.zarr');
```
