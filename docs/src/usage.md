# Usage

The extension exposes a single table function: `read_zarr`.

## Basic Querying

To read a Zarr array from a local file path:

```sql
SELECT * FROM read_zarr('/path/to/my_data.zarr');
```

The extension automatically inspects the Zarr metadata to determine the schema. For an array with dimensions `[time, lat, lon]`, it will yield four columns: `time`, `lat`, `lon`, and `value`.

## Cloud Storage (S3 / HTTP)

You can read directly from AWS S3 or HTTP endpoints. The extension uses Apache OpenDAL under the hood.

```sql
-- Read from an S3 bucket
SELECT SUM(value) FROM read_zarr('s3://my-bucket/climate-data.zarr');

-- Read from a public HTTP endpoint
SELECT AVG(value) FROM read_zarr('https://public-data.com/dataset.zarr');
```

### AWS Credentials
When using `s3://`, the extension automatically looks for standard AWS environment variables. Ensure these are set in the terminal before starting DuckDB:
- `AWS_ACCESS_KEY_ID`
- `AWS_SECRET_ACCESS_KEY`
- `AWS_REGION`

## Spatial Pruning (Filtering)

Due to limitations in current DuckDB extension bindings, standard `WHERE` clauses cannot be pushed down to the network layer. To prevent the extension from downloading chunks outside of your region of interest, use **Named Parameters**.

You can pass `_min` and `_max` parameters for any coordinate dimension. The extension will binary-search the coordinate arrays and strictly prune out-of-bounds chunks before performing network I/O.

```sql
SELECT count(*) 
FROM read_zarr(
    's3://my-bucket/climate.zarr', 
    lat_min := 45.0, 
    lat_max := 55.0,
    time_min := 1609459200 -- Epoch timestamp
);
```

## Projection Pushdown

The extension intelligently detects which columns are actually requested by your query. If you run an aggregation that only requires the `value` column:

```sql
SELECT SUM(value) FROM read_zarr('s3://bucket/data.zarr');
```

The extension will **skip** calculating and allocating the coordinate vectors (like `lat` and `lon`), saving significant CPU overhead and memory bandwidth.
