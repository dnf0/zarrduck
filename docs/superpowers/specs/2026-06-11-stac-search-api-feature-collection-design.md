# STAC Search API / FeatureCollection Integration Design

## Overview
This feature introduces the ability to seamlessly query STAC Search APIs and STAC `FeatureCollection`s using DuckDB, leveraging the existing `VirtualStacTimeStack` to map API results into virtual 3D Zarr arrays (`[time, lat, lon]`).

By safely pushing down bounding box (`bbox`) constraints to the API during the binding phase, we ensure that DuckDB query planning is accurate, parallelization is preserved, and unbounded data fetching is prevented.

## Architecture

### 1. Bounding Box Extraction & Validation
In the DuckDB extension (`table_function.rs`), the `bind()` phase currently opens the dataset *before* parsing the query parameters (e.g., `lat_min`, `lon_max`).
- **Change:** Parse the spatial bounds parameters first.
- **Safety Gate:** Calculate the area of the bounding box. If the area exceeds a predefined maximum threshold (or if no spatial bounds are provided), fail the `bind()` phase immediately with an explicit error.
- **Why:** This prevents a naive query from fetching the entire catalog of imagery before filtering.

### 2. Constraint Pushdown
The `ZarrDataset::open_with_asset` and `store::resolve_sync_store` functions will be updated to accept an optional set of constraints (e.g., `&QueryConstraints` or a direct bounding box).
- **Behavior:** When `resolve_sync_store` detects a STAC `FeatureCollection` URL (or detects that it should hit a search endpoint), it will format the bounds into a `&bbox=lon_min,lat_min,lon_max,lat_max` parameter and append it to the request URL.
- **Execution:** It then fetches the constrained JSON response, parses the returned STAC Items, and builds the `VirtualStacTimeStack` in memory.

### 3. Zero-Cost Parallel Scan
Once `bind()` completes, DuckDB knows the exact shape of the returned dataset (`[N, H, W]`, where `N` is the number of features intersecting the bounding box).
- **Benefit:** DuckDB allocates exactly the right number of threads to read the chunks in parallel.
- **Impact:** The `scan()` phase (`ReadGeoVTab::func`) requires **zero modifications**. It simply requests chunks from the `ZarrDataset`, and the `VirtualStacTimeStack` fetches the specific COG tiles concurrently.

## Error Handling
- Invalid, missing, or overly large bounding boxes yield a clear error in DuckDB during query compilation.
- Empty FeatureCollections (no STAC items intersecting the box) yield an empty array safely without panic.
- Existing STAC Item (single feature) logic remains untouched.

## Testing
- Add unit tests for `build_stac_url` (or equivalent URL formatting logic) to verify bbox parameter generation.
- Add unit tests for bounding box area validation.
- End-to-end test mocking a STAC Search API endpoint.
