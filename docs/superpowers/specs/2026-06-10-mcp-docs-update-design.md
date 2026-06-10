# Eider MCP Documentation Updates Design

## Context
Recent commits introduced two significant features to `eider-mcp`:
1. **Live query progress streaming**: The server now emits standard MCP progress notifications reflecting native DuckDB query progress.
2. **`extract_point_timeseries` tool**: A new tool for extracting a timeseries for a specific geographic point using `nearest` or `bilinear` interpolation.

The existing documentation at `docs/docs/usage/mcp.md` needs to be updated to reflect these capabilities.

## Updates

### 1. Introduction Addition
Add a new bullet point to the introduction's list of core features to highlight progress streaming as a major UX benefit.

**Content:**
- **Live query progress.** The server streams native DuckDB query progress over the standard MCP notification wire, ensuring long spatial reads don't look like hanging processes to the user.

### 2. New Section: Progress Streaming
Add a new `## Progress streaming` H2 section after the "Client configuration" section to instruct client authors on supporting this feature.

**Content:**
The server emits standard [MCP progress notifications](https://modelcontextprotocol.io/docs/tools#tool-progress) for all long-running spatial queries (like `zonal_stats` or large `read_region` calls). The percentage reflects native DuckDB query progress. To ensure a good user experience when queries take tens of seconds, your MCP client must support and render these progress notifications.

### 3. Tool Reference Addition
Add documentation for the new tool under `## Tool reference`.

**Content:**
### `extract_point_timeseries(uri, lat, lon, method?="nearest", value_col?="value")`

Returns the same `{ table_handle, row_count, columns, rows, truncated }` shape, plus the `method` used. Extracts a timeseries for a specific geographic point across all time steps. The bounding box pruning is highly optimized to fetch only the 1 or 4 closest cells.

- `method` ∈ `{ nearest, bilinear }`:
  - **`nearest`** — selects the single closest grid cell.
  - **`bilinear`** — calculates an inverse-distance weighted average of the 4 closest cells.

**When to use:** to extract a timeseries for a single latitude/longitude coordinate without needing to construct a geometry or run a full zonal aggregate.
