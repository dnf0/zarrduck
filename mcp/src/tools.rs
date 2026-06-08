//! Pure, rmcp-independent tool logic. Each tool is `&Connection (+args) ->
//! Result<serde_json::Value>` so it can be unit-tested directly and wrapped by
//! the rmcp adapter (added in a later task) without duplicating logic.

use crate::result::{materialize, rows_as_json};
use color_eyre::eyre::{eyre, Result as EyreResult};
use duckdb::Connection;
use serde_json::{json, Value};

/// Default cap on rows inlined in a tool result; full data stays in the temp
/// table referenced by the result handle.
const DEFAULT_LIMIT: usize = 1000;

/// Escape single quotes for embedding a value inside a single-quoted SQL
/// string literal.
fn esc(s: &str) -> String {
    s.replace('\'', "''")
}

/// Build the optional bbox/time/asset named args for `read_geo`/`plan_read_geo`
/// from JSON, e.g. `, lon_min := 1, lat_min := 2, time_min := 0, asset := 'a'`.
/// Returns an empty string when nothing is pushed down.
fn pushdown_args(bbox: Option<&Value>, time: Option<&Value>, asset: Option<&str>) -> String {
    let mut parts = Vec::new();
    if let Some(b) = bbox {
        for (k, p) in [
            ("lon_min", "lon_min"),
            ("lat_min", "lat_min"),
            ("lon_max", "lon_max"),
            ("lat_max", "lat_max"),
        ] {
            if let Some(v) = b.get(k).and_then(|x| x.as_f64()) {
                parts.push(format!("{p} := {v}"));
            }
        }
    }
    if let Some(t) = time {
        if let Some(v) = t.get("min").and_then(|x| x.as_f64()) {
            parts.push(format!("time_min := {v}"));
        }
        if let Some(v) = t.get("max").and_then(|x| x.as_f64()) {
            parts.push(format!("time_max := {v}"));
        }
    }
    if let Some(a) = asset {
        parts.push(format!("asset := '{}'", esc(a)));
    }
    if parts.is_empty() {
        String::new()
    } else {
        format!(", {}", parts.join(", "))
    }
}

/// Return the Zarr dataset metadata (array/chunk shape, dtype, CRS).
pub fn describe_dataset(conn: &Connection, uri: &str) -> EyreResult<Value> {
    let sql = format!("SELECT * FROM read_zarr_metadata('{}')", esc(uri));
    Ok(json!({ "metadata": rows_as_json(conn, &sql)? }))
}

/// Estimate the read cost (`total_chunks`, `total_bytes`) for a region, with
/// optional bbox/time pushdown.
pub fn estimate_cost(
    conn: &Connection,
    uri: &str,
    bbox: Option<&Value>,
    time: Option<&Value>,
) -> EyreResult<Value> {
    let sql = format!(
        "SELECT * FROM plan_read_geo('{}'{})",
        esc(uri),
        pushdown_args(bbox, time, None)
    );
    Ok(json!({ "estimate": rows_as_json(conn, &sql)? }))
}

/// Read a region with bbox/time/asset pushdown and materialize it into a temp
/// table, returning a capped head + handle.
pub fn read_region(
    conn: &Connection,
    uri: &str,
    bbox: Option<&Value>,
    time: Option<&Value>,
    asset: Option<&str>,
    limit: Option<usize>,
) -> EyreResult<Value> {
    let sql = format!(
        "SELECT * FROM read_geo('{}'{})",
        esc(uri),
        pushdown_args(bbox, time, asset)
    );
    materialize(conn, &sql, limit.unwrap_or(DEFAULT_LIMIT))
}

/// List the tables visible in the current session (including temp result
/// handles).
pub fn list_tables(conn: &Connection) -> EyreResult<Value> {
    Ok(json!({
        "tables": rows_as_json(
            conn,
            "SELECT table_name FROM information_schema.tables ORDER BY table_name",
        )?
    }))
}

/// Describe a table's schema and a small sample. The table name is validated
/// (alphanumeric/underscore only) since it is interpolated into SQL.
pub fn describe_table(conn: &Connection, name: &str) -> EyreResult<Value> {
    if name.is_empty() || !name.chars().all(|c| c.is_alphanumeric() || c == '_') {
        return Err(eyre!("invalid table name"));
    }
    Ok(json!({
        "schema": rows_as_json(conn, &format!("DESCRIBE {name}"))?,
        "sample": rows_as_json(conn, &format!("SELECT * FROM {name} LIMIT 5"))?,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session() -> Option<Connection> {
        std::env::set_var(
            "GEOZARR_ALLOW_PATH",
            env!("CARGO_MANIFEST_DIR").to_string() + "/..",
        );
        if std::env::var("EIDER_EXTENSION_PATH").is_err()
            && !std::path::Path::new("../target/debug/eider.duckdb_extension").exists()
        {
            eprintln!("skip: extension not built");
            return None;
        }
        Some(eider_session::open_session().unwrap())
    }

    fn zarr() -> String {
        format!(
            "{}/../climate_data.zarr/air_temperature",
            env!("CARGO_MANIFEST_DIR")
        )
    }

    #[test]
    fn describe_returns_crs_and_dtype() {
        let Some(c) = session() else { return };
        let v = describe_dataset(&c, &zarr()).unwrap();
        let s = v.to_string();
        assert!(s.contains("EPSG:4326") && s.contains("Float32"), "{s}");
    }

    #[test]
    fn read_region_caps_and_handles() {
        let Some(c) = session() else { return };
        let v = read_region(&c, &zarr(), None, None, None, Some(5)).unwrap();
        assert_eq!(v["rows"].as_array().unwrap().len(), 5);
        assert!(v["row_count"].as_i64().unwrap() > 5);
        assert!(v["table_handle"]
            .as_str()
            .unwrap()
            .starts_with("mcp_result_"));
    }

    #[test]
    fn estimate_cost_returns_chunks() {
        let Some(c) = session() else { return };
        let v = estimate_cost(&c, &zarr(), None, None).unwrap();
        assert!(v.to_string().contains("total_chunks"));
    }
}
