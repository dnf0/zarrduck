mod common;
use common::*;
use predicates::prelude::*;

// Same URI as in info_test: point at the array subdirectory, not the group root.
// Pointing at the group root in AgentJson mode triggers "Zarr Group containing
// multiple datasets" because climate_data.zarr has 4 arrays.
fn air_temp_uri() -> String {
    climate_zarr()
        .join("air_temperature")
        .to_string_lossy()
        .into_owned()
}

#[test]
fn extract_writes_rows_into_output_db() {
    geozarr_ext_path();
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("extracted.duckdb");

    eider(&dir)
        .env("GEOZARR_ALLOW_PATH", repo_root())
        .arg("extract")
        .arg(air_temp_uri())
        .arg(fixture_path("polygon.geojson"))
        .args(["--out", out.to_str().unwrap()])
        .arg("--yes")
        .arg("--output=json")
        .assert()
        .success()
        .stdout(predicate::str::contains(r#""status": "success""#));

    let conn = duckdb::Connection::open(&out).unwrap();
    let n: i64 = conn
        .query_row("SELECT count(*) FROM extracted_data", [], |r| r.get(0))
        .unwrap();
    assert!(n > 0, "extraction should produce rows for the polygon area");
}

#[test]
fn extract_refuses_overwrite_in_json_mode() {
    geozarr_ext_path();
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("exists.duckdb");
    std::fs::write(&out, b"x").unwrap();
    eider(&dir)
        .env("GEOZARR_ALLOW_PATH", repo_root())
        .arg("extract")
        .arg(air_temp_uri())
        .arg(fixture_path("polygon.geojson"))
        .args(["--out", out.to_str().unwrap()])
        .arg("--output=json")
        .assert()
        .failure()
        .stdout(predicate::str::contains("already exists"));
}
