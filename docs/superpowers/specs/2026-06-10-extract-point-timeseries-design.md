# Point Timeseries Extraction Tool Design

## 1. Context & Purpose
The `eider-mcp` server currently exposes spatial operations via raw SQL (`run_sql`). To improve agent ergonomics, we are adding an `extract_point_timeseries` tool. This tool will allow agents to easily retrieve the timeseries for a specific geographic point `(lat, lon)` without needing to construct complex DuckDB spatial bounding queries.

## 2. Interface
The tool will accept the following parameters:
- `uri` (String): The Zarr dataset URI.
- `lat` (f64): The target latitude in EPSG:4326.
- `lon` (f64): The target longitude in EPSG:4326.
- `method` (String, optional): Interpolation method: `"nearest"` or `"bilinear"`. Defaults to `"nearest"`.
- `value_col` (String, optional): The column to extract. Defaults to `"value"`.

## 3. Architecture: Two-Phase Execution
To minimize network fetch overhead, we will use a two-phase query approach.

### Phase 1: Coordinate Discovery
We need to determine the exact grid cell(s) that correspond to the requested `(lat, lon)`.
Since datasets have varying resolutions, we cannot safely hardcode a tiny bounding box. Instead we will:
1. Query `read_geo` with a `LIMIT 1` and no spatial bounds to extract the grid's `lat` and `lon` arrays into a CTE.
2. Mathematically compute the distance from the target point to all `lat` and `lon` values in the grid to find the closest match (or 4 closest for bilinear).

### Phase 2: Timeseries Extraction
We will execute a second `read_geo` query, passing exactly the discovered cell coordinates as spatial filters (`lat_min=...`, `lat_max=...`, `lon_min=...`, `lon_max=...`).
- **`method="nearest"`:** The query will filter for the exact nearest cell and return its timeseries.
- **`method="bilinear"`:** The query will filter for the 4 surrounding cells. We will use SQL to aggregate them: `SUM(value * weight) / SUM(weight)`, where weight is the inverse distance (or exact bilinear area overlap) to the target point, grouped by `time`.

## 4. Output
The tool will materialize the result into a temporary DuckDB table (e.g., `mcp_result_...`).
It will return:
- The `table_handle` name.
- The `method` used.
- An array of the first 1,000 rows containing `time`, `lat`, `lon`, and `<value_col>`.

## 5. Constraints
- The user-provided `lat` and `lon` must be finite numbers.
- `method` must be exactly `"nearest"` or `"bilinear"`.
- Validations on table names and column names apply to prevent SQL injection.
