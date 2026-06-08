# Design: Documenting DuckDB Extension Usage for Polygons

## Context
Currently, the `guide_polygons.md` documentation shows how to extract Zarr cells intersecting with vector polygons using the `eider extract` CLI command. We want to demonstrate that the `eider` DuckDB extension enables this exact same capability natively in SQL, which provides transparency ("under the hood") and offers users an alternative pure-SQL workflow.

## Design
We will present the CLI and SQL workflows side-by-side using Docusaurus `<Tabs>`.

### 1. Document Structure Changes
- **Imports:** Add Docusaurus MDX tab imports at the top of `docs/docs/usage/guide_polygons.md`:
  ```mdx
  import Tabs from '@theme/Tabs';
  import TabItem from '@theme/TabItem';
  ```

### 2. Tab Implementation
In Section "2. Extract the cells inside the polygons", replace the standalone bash block with a `<Tabs>` component containing two tabs:

- **CLI Tab:** Retain the current `eider extract` command and its output verbatim.
- **SQL Tab:** Introduce an interactive DuckDB shell session block.
  - Formatted to show the `D ` prompt.
  - Commands include loading required extensions (`INSTALL spatial; LOAD spatial; INSTALL eider; LOAD eider;`).
  - Displays the core spatial join query that replicates the CLI behavior:
    ```sql
    CREATE TABLE extracted_data AS
    SELECT z.*, v.* EXCLUDE (geom)
    FROM read_geo('climate_data.zarr/air_temperature') z,
         ST_Read('scripts/demo_polygons.geojson') v
    WHERE ST_Contains(v.geom, ST_Point(z.lon, z.lat));
    ```

### 3. Text Adjustments
- Slightly modify the introductory sentences in Section 2 to explain that this operation can be performed via the CLI or directly in DuckDB using the Eider extension.
