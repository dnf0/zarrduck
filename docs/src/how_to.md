# How-To Guides

This section provides practical, step-by-step examples for common data science and engineering workflows using the DuckDB GeoZarr extension.

## How to extract a specific geographic region (Bounding Box)

Often, climate datasets cover the entire globe, but your analysis is restricted to a specific country or region. Downloading the global dataset is incredibly wasteful.

By passing named parameters for coordinate limits, the extension will *only* download the Zarr chunks that intersect your bounding box.

```sql
SELECT 
    time,
    lat,
    lon,
    value as temperature
FROM read_zarr(
    's3://climate-data/global_temperature.zarr',
    -- Define the bounding box for Europe
    lat_min := 35.0,
    lat_max := 70.0,
    lon_min := -10.0,
    lon_max := 30.0
)
WHERE time >= 1609459200; -- Further filter by time (e.g., year 2021)
```
*Note: Because DuckDB cannot push standard `WHERE` clauses down into table functions yet, the coordinate pruning MUST be done using the `:=` named parameters to achieve network savings.*

## How to aggregate data over time (e.g., Monthly Averages)

If you are calculating time-series aggregations, DuckDB is incredibly fast. To maximize performance, ensure you only select the columns you need.

```sql
SELECT 
    time,
    AVG(value) as monthly_average
FROM read_zarr('s3://climate-data/precipitation.zarr')
GROUP BY time
ORDER BY time;
```
Because the `lat` and `lon` columns are completely absent from this query (Projection Pushdown), the extension skips calculating them and skips allocating memory for them, leading to a massive speedup.

## How to handle categorical data (Strings/Varchar)

Geospatial data isn't just numbers. Land cover datasets, for example, often use strings to denote classifications. The extension natively supports Zarr string arrays and automatically bridges them to DuckDB `VARCHAR` columns.

```sql
SELECT 
    value as land_cover_type,
    count(*) as pixel_count
FROM read_zarr('s3://climate-data/land_cover.zarr')
GROUP BY value
ORDER BY pixel_count DESC;
```

## How to connect to private S3 buckets

The extension uses the standard AWS SDK credential chain. You do not configure secrets inside DuckDB's SQL interface. Instead, export them in your terminal environment before launching your script or DuckDB CLI:

```bash
export AWS_ACCESS_KEY_ID="AKIAIOSFODNN7EXAMPLE"
export AWS_SECRET_ACCESS_KEY="wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY"
export AWS_REGION="us-east-1"

# Launch DuckDB
duckdb
```
Then simply query the bucket:
```sql
SELECT * FROM read_zarr('s3://my-private-bucket/data.zarr');
```
