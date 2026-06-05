mod common;
use common::*;
use predicates::prelude::*;

// `export` requires coordinate columns to be 0-based integer dimension indices
// (see RowProcessor::calculate_indices in src/export/stream.rs). The value column
// is everything else. Shape is inferred from COUNT(DISTINCT) per coordinate.

/// A source DuckDB db with a full 2x2x2 grid of 0-based index coordinates
/// (t, y, x in {0,1}) and a `value` column. Returns the db path.
fn make_export_source_db(dir: &tempfile::TempDir) -> std::path::PathBuf {
    let db = dir.path().join("src.duckdb");
    let conn = duckdb::Connection::open(&db).unwrap();
    conn.execute_batch(
        "CREATE TABLE src (t BIGINT, y BIGINT, x BIGINT, value DOUBLE);
         INSERT INTO src VALUES
             (0, 0, 0, 1.0), (0, 0, 1, 2.0),
             (0, 1, 0, 3.0), (0, 1, 1, 4.0),
             (1, 0, 0, 5.0), (1, 0, 1, 6.0),
             (1, 1, 0, 7.0), (1, 1, 1, 8.0);",
    )
    .unwrap();
    db
}

#[test]
fn export_query_to_zarr_then_info_reads_it_back() {
    if find_geozarr_ext().is_none() {
        eprintln!("skipping: eider.duckdb_extension not built (expected on Windows)");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let src = make_export_source_db(&dir);
    let out_zarr = dir.path().join("exported.zarr");

    // Regression guard: `--dest` must not collide with the global `--output`
    // table|json flag. Before the fix this panicked with a clap downcast error.
    eider(&dir)
        .env("GEOZARR_ALLOW_PATH", dir.path())
        .arg("export")
        .args(["--db", src.to_str().unwrap()])
        .args(["--query", "SELECT * FROM src"])
        .args(["--dest", out_zarr.to_str().unwrap()])
        .args(["--value-column", "value"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Export successful"));

    // Round-trip: the produced store must be readable back by the extension.
    eider(&dir)
        .arg("info")
        .arg(out_zarr.to_str().unwrap())
        .arg("--output=json")
        .assert()
        .success()
        .stdout(predicate::str::contains(r#""array_shape""#));
}

#[test]
fn export_rejects_non_index_coordinates() {
    if find_geozarr_ext().is_none() {
        eprintln!("skipping: eider.duckdb_extension not built (expected on Windows)");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("bad.duckdb");
    let conn = duckdb::Connection::open(&db).unwrap();
    // t has two distinct values {0, 5} -> inferred dim size 2, but the value 5
    // is out of bounds, so streaming must reject it.
    conn.execute_batch(
        "CREATE TABLE src (t BIGINT, y BIGINT, x BIGINT, value DOUBLE);
         INSERT INTO src VALUES (0, 0, 0, 1.0), (5, 0, 0, 2.0);",
    )
    .unwrap();
    drop(conn);

    let out_zarr = dir.path().join("bad.zarr");
    eider(&dir)
        .env("GEOZARR_ALLOW_PATH", dir.path())
        .arg("export")
        .args(["--db", db.to_str().unwrap()])
        .args(["--query", "SELECT * FROM src"])
        .args(["--dest", out_zarr.to_str().unwrap()])
        .args(["--value-column", "value"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("exceeds maximum bound"));
}

#[test]
fn export_errors_when_value_column_missing() {
    if find_geozarr_ext().is_none() {
        eprintln!("skipping: eider.duckdb_extension not built (expected on Windows)");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let src = make_export_source_db(&dir);
    eider(&dir)
        .env("GEOZARR_ALLOW_PATH", dir.path())
        .arg("export")
        .args(["--db", src.to_str().unwrap()])
        .args(["--query", "SELECT * FROM src"])
        .args(["--dest", dir.path().join("o.zarr").to_str().unwrap()])
        .args(["--value-column", "nonexistent"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("not found in query results"));
}
