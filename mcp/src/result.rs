//! Result shaping: run a query into a session TEMP table and return a capped,
//! JSON-friendly summary (handle + head rows + row count + truncated flag).
//!
//! Full data stays in the temp table, referenced by `table_handle`, so large
//! results don't flood the MCP client; the agent can follow up with `run_sql`
//! against the handle.

use color_eyre::eyre::{Result as EyreResult, WrapErr};
use duckdb::Connection;
use serde_json::{json, Map, Value};
use std::sync::atomic::{AtomicU64, Ordering};

static RESULT_SEQ: AtomicU64 = AtomicU64::new(0);

/// Materialize `select_sql` into a session TEMP table and return
/// `{table_handle, row_count, columns, rows (<=limit), truncated}`.
///
/// The full result stays in the temp table; only the first `limit` rows are
/// inlined in the JSON.
pub fn materialize(conn: &Connection, select_sql: &str, limit: usize) -> EyreResult<Value> {
    let n = RESULT_SEQ.fetch_add(1, Ordering::Relaxed);
    let handle = format!("mcp_result_{n}");
    conn.execute_batch(&format!("CREATE TEMP TABLE {handle} AS {select_sql}"))
        .wrap_err("materialize result")?;
    let row_count: i64 = conn
        .query_row(&format!("SELECT count(*) FROM {handle}"), [], |r| r.get(0))
        .wrap_err("count materialized rows")?;
    let (columns, rows) =
        rows_with_columns(conn, &format!("SELECT * FROM {handle} LIMIT {limit}"))?;
    Ok(json!({
        "table_handle": handle,
        "row_count": row_count,
        "columns": columns,
        "rows": rows,
        "truncated": (row_count as usize) > limit,
    }))
}

/// Execute an arbitrary `SELECT` and return its rows as JSON objects keyed by
/// column name.
pub fn rows_as_json(conn: &Connection, sql: &str) -> EyreResult<Value> {
    let (_, rows) = rows_with_columns(conn, sql)?;
    Ok(rows)
}

/// Run `sql`, returning `(column_names_as_json, rows_as_json_array)`.
///
/// Column names are read from the executed statement (`query` executes the
/// statement before returning `Rows`, so the schema is available even for an
/// empty result set). Values are converted via [`value_ref_to_json`].
fn rows_with_columns(conn: &Connection, sql: &str) -> EyreResult<(Value, Value)> {
    let mut stmt = conn.prepare(sql).wrap_err("prepare query")?;
    let mut rows = stmt.query([]).wrap_err("execute query")?;
    // `query` already executed the statement, so column metadata is available.
    let names: Vec<String> = rows.as_ref().map(|s| s.column_names()).unwrap_or_default();
    let mut out = Vec::new();
    while let Some(row) = rows.next()? {
        let mut obj = Map::new();
        for (i, name) in names.iter().enumerate() {
            obj.insert(name.clone(), value_ref_to_json(row, i)?);
        }
        out.push(Value::Object(obj));
    }
    Ok((json!(names), Value::Array(out)))
}

/// Convert a single DuckDB cell to JSON.
///
/// Integers map to JSON numbers (i64) where they fit, floats to f64, booleans
/// and text to their JSON equivalents, `NULL` to JSON null. Anything else
/// (blobs, temporal, nested) is rendered as its string form so the agent still
/// sees a value rather than losing the column.
fn value_ref_to_json(row: &duckdb::Row, i: usize) -> EyreResult<Value> {
    use duckdb::types::ValueRef;
    let v = row.get_ref(i).wrap_err("read cell value")?;
    Ok(match v {
        ValueRef::Null => Value::Null,
        ValueRef::Boolean(b) => json!(b),
        ValueRef::TinyInt(n) => json!(n as i64),
        ValueRef::SmallInt(n) => json!(n as i64),
        ValueRef::Int(n) => json!(n as i64),
        ValueRef::BigInt(n) => json!(n),
        ValueRef::UTinyInt(n) => json!(n as i64),
        ValueRef::USmallInt(n) => json!(n as i64),
        ValueRef::UInt(n) => json!(n as i64),
        ValueRef::UBigInt(n) => i64::try_from(n)
            .map(|x| json!(x))
            .unwrap_or_else(|_| json!(n.to_string())),
        ValueRef::HugeInt(n) => i64::try_from(n)
            .map(|x| json!(x))
            .unwrap_or_else(|_| json!(n.to_string())),
        ValueRef::Float(f) => json!(f as f64),
        ValueRef::Double(f) => json!(f),
        ValueRef::Text(bytes) => json!(String::from_utf8_lossy(bytes).into_owned()),
        // Fall back to the owned-value string form for everything else
        // (decimal, temporal, blob, nested types).
        _ => match row.get::<usize, String>(i) {
            Ok(s) => json!(s),
            Err(_) => Value::Null,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn conn() -> Connection {
        let c = Connection::open_in_memory().unwrap();
        c.execute_batch("CREATE TABLE t AS SELECT * FROM range(0,100) tbl(x)")
            .unwrap();
        c
    }

    #[test]
    fn materialize_caps_and_handles() {
        let c = conn();
        let r = materialize(&c, "SELECT x FROM t", 10).unwrap();
        assert_eq!(r["row_count"], json!(100));
        assert_eq!(r["truncated"], json!(true));
        assert_eq!(r["rows"].as_array().unwrap().len(), 10);
        assert_eq!(r["columns"], json!(["x"]));
        let h = r["table_handle"].as_str().unwrap();
        let n: i64 = c
            .query_row(&format!("SELECT count(*) FROM {h}"), [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 100);
    }

    #[test]
    fn rows_as_json_keys_and_nulls() {
        let c = conn();
        let v = rows_as_json(&c, "SELECT 1 AS a, NULL AS b, 'hi' AS c").unwrap();
        let row = &v.as_array().unwrap()[0];
        assert_eq!(row["a"], json!(1));
        assert_eq!(row["b"], Value::Null);
        assert_eq!(row["c"], json!("hi"));
    }
}
