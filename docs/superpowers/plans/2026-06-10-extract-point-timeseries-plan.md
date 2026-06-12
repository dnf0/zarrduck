# extract_point_timeseries Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [x]`) syntax for tracking.

**Goal:** Build the `extract_point_timeseries` MCP tool to allow agents to easily extract a timeseries for a specific geographic point using a fast two-phase DuckDB query approach.

**Architecture:** We will implement the pure logic in `mcp/src/tools.rs` and the rmcp wrapper in `mcp/src/server.rs`. The two-phase query discovers the exact grid cells for a coordinate (handling nearest and bilinear interpolation), then extracts the timeseries across all time steps for just those cells.

**Tech Stack:** Rust, DuckDB, rmcp

---

### Task 1: Add pure logic to `tools.rs`

**Files:**
- Modify: `mcp/src/tools.rs`

- [x] **Step 1: Write the failing tests**

Append to `mod tests` inside `mcp/src/tools.rs`:
```rust
    #[test]
    fn extract_point_timeseries_nearest() {
        let Some(c) = session() else { return };
        // Valid lat/lon inside the dataset bounds
        let v = extract_point_timeseries(&c, &zarr(), 50.0, -10.0, "nearest", None).unwrap();
        assert_eq!(v["method"], json!("nearest"));
        assert!(v["row_count"].as_i64().unwrap() >= 1);
        let rows = v["rows"].as_array().unwrap();
        assert!(rows[0].get("time").is_some());
        assert!(rows[0].get("value").is_some());
        assert!(v["table_handle"].as_str().unwrap().starts_with("mcp_result_"));
    }

    #[test]
    fn extract_point_timeseries_bilinear() {
        let Some(c) = session() else { return };
        let v = extract_point_timeseries(&c, &zarr(), 50.0, -10.0, "bilinear", None).unwrap();
        assert_eq!(v["method"], json!("bilinear"));
        assert!(v["row_count"].as_i64().unwrap() >= 1);
        let rows = v["rows"].as_array().unwrap();
        assert!(rows[0].get("time").is_some());
        assert!(rows[0].get("value").is_some());
    }

    #[test]
    fn extract_point_timeseries_invalid_method() {
        let Some(c) = session() else { return };
        let res = extract_point_timeseries(&c, &zarr(), 50.0, -10.0, "bicubic", None);
        assert!(res.is_err());
    }
```

- [x] **Step 2: Run test to verify it fails**

Run: `cargo test -p eider-mcp -- test_extract_point`
Expected: FAIL (cannot find function `extract_point_timeseries`)

- [x] **Step 3: Write minimal implementation**

Append to the public functions in `mcp/src/tools.rs` (above `mod tests`):
```rust
/// Extract the timeseries for a specific point.
///
/// Executes a two-phase query:
/// 1. Discovers the grid cell coordinates nearest to the point.
/// 2. Reads the full timeseries for exactly those cells, aggregating if bilinear.
pub fn extract_point_timeseries(
    conn: &Connection,
    uri: &str,
    lat: f64,
    lon: f64,
    method: &str,
    value_col: Option<&str>,
) -> EyreResult<Value> {
    if method != "nearest" && method != "bilinear" {
        return Err(eyre!("method must be 'nearest' or 'bilinear'"));
    }
    let vc = value_col.unwrap_or("value");
    if vc.is_empty() || !vc.chars().all(|c| c.is_alphanumeric() || c == '_') {
        return Err(eyre!("invalid value column"));
    }
    let uri = esc(uri);

    // Phase 1: Coordinate Discovery CTE
    // Read the grid shape without time fetching to find the exact cells
    let discovery_cte = format!(
        "WITH grid AS (\
             SELECT lat, lon \
             FROM read_geo('{uri}', time_min := 0, time_max := 0) \
         ), \
         distances AS (\
             SELECT lat, lon, ST_Distance(ST_Point(lon, lat), ST_Point({lon}, {lat})) AS dist \
             FROM grid \
         ), \
         closest AS (\
             SELECT lat, lon, dist \
             FROM distances \
             ORDER BY dist ASC \
             LIMIT {limit}\
         )",
        limit = if method == "nearest" { 1 } else { 4 }
    );

    // Phase 2: Exact Pushdown
    let extract_query = if method == "nearest" {
        format!(
            "{discovery_cte} \
             SELECT d.time, c.lat, c.lon, d.{vc} \
             FROM closest c \
             JOIN read_geo('{uri}', \
                 lat_min := (SELECT min(lat) FROM closest), \
                 lat_max := (SELECT max(lat) FROM closest), \
                 lon_min := (SELECT min(lon) FROM closest), \
                 lon_max := (SELECT max(lon) FROM closest) \
             ) d ON c.lat = d.lat AND c.lon = d.lon"
        )
    } else {
        format!(
            "{discovery_cte} \
             SELECT d.time, {lat} AS lat, {lon} AS lon, \
                    sum(d.{vc} * (1.0 / nullif(c.dist, 0))) / sum(1.0 / nullif(c.dist, 0)) AS {vc} \
             FROM closest c \
             JOIN read_geo('{uri}', \
                 lat_min := (SELECT min(lat) FROM closest), \
                 lat_max := (SELECT max(lat) FROM closest), \
                 lon_min := (SELECT min(lon) FROM closest), \
                 lon_max := (SELECT max(lon) FROM closest) \
             ) d ON c.lat = d.lat AND c.lon = d.lon \
             GROUP BY d.time \
             ORDER BY d.time"
        )
    };

    let mut out = materialize(conn, &extract_query, DEFAULT_LIMIT)?;
    out["method"] = json!(method);
    Ok(out)
}
```

- [x] **Step 4: Run test to verify it passes**

Run: `cargo test -p eider-mcp -- extract_point_timeseries`
Expected: PASS

- [x] **Step 5: Commit**

```bash
git add mcp/src/tools.rs
git commit -m "feat(mcp): implement extract_point_timeseries pure logic"
```

### Task 2: Expose tool in `EiderServer`

**Files:**
- Modify: `mcp/src/server.rs`

- [x] **Step 1: Write parameter structs and handler**

Add the struct and update the router in `mcp/src/server.rs`:

```rust
/// Parameters for `extract_point_timeseries`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ExtractPointParams {
    /// Zarr dataset/array URI.
    pub uri: String,
    /// Target latitude.
    pub lat: f64,
    /// Target longitude.
    pub lon: f64,
    /// Interpolation method: "nearest" or "bilinear". Defaults to "nearest".
    pub method: Option<String>,
    /// Value column to extract. Defaults to "value".
    pub value_col: Option<String>,
}
```

Add the method to `impl EiderServer`:
```rust
    #[tool(
        name = "extract_point_timeseries",
        description = "Extract a timeseries for a specific geographic point. method: nearest|bilinear."
    )]
    pub async fn extract_point_timeseries(
        &self,
        Parameters(params): Parameters<ExtractPointParams>,
    ) -> Result<CallToolResult, ErrorData> {
        self.run(move |conn| {
            crate::tools::extract_point_timeseries(
                conn,
                &params.uri,
                params.lat,
                params.lon,
                params.method.as_deref().unwrap_or("nearest"),
                params.value_col.as_deref(),
            )
        })
        .await
    }
```

Register it in `fn router() -> ToolRouter<Self>`:
```rust
        let mut r = ToolRouter::new();
        r.register_tool(Self::describe_dataset);
        r.register_tool(Self::estimate_cost);
        r.register_tool(Self::read_region);
        r.register_tool(Self::list_tables);
        r.register_tool(Self::describe_table);
        r.register_tool(Self::run_sql);
        r.register_tool(Self::zonal_stats);
        r.register_tool(Self::extract_point_timeseries);
        r
```

- [x] **Step 2: Run build/clippy to verify**

Run: `cargo clippy -p eider-mcp`
Expected: PASS

- [x] **Step 3: Commit**

```bash
git add mcp/src/server.rs
git commit -m "feat(mcp): expose extract_point_timeseries tool via EiderServer"
```
