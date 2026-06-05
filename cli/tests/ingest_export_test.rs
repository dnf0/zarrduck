mod common;
use common::*;
use predicates::prelude::*;

// The ingest command uses ST_Read which expects a spatial file with a geometry
// column. Plain CSV without a geom column causes "Column geom in EXCLUDE list
// not found". We use a GeoJSON fixture instead; ST_Read produces a geom column
// (excluded) plus the lon/lat/value property columns.

#[test]
fn ingest_geojson_to_zarr_then_zarr_store_exists() {
    geozarr_ext_path();
    let dir = tempfile::tempdir().unwrap();
    let out_zarr = dir.path().join("ingested.zarr");

    eider(&dir)
        .env("GEOZARR_ALLOW_PATH", dir.path())
        .arg("ingest")
        .arg(fixture_path("ingest_input.geojson"))
        .arg(out_zarr.to_str().unwrap())
        .args(["--value-column", "value"])
        .arg("--output=json")
        .assert()
        .success()
        .stdout(predicate::str::contains(r#""status": "success""#));

    // The ingest/export pipeline writes a single-array Zarr store: the array is
    // created at path "/" within the store root. zarrs 0.16 writes Zarr v3 format
    // (zarr.json) at the store root rather than Zarr v2 (.zarray).
    assert!(
        out_zarr.join("zarr.json").exists() || out_zarr.join(".zarray").exists(),
        "ingest should produce zarr.json or .zarray at the store root"
    );
}

#[test]
fn ingest_missing_input_errors() {
    let dir = tempfile::tempdir().unwrap();
    eider(&dir)
        .arg("ingest")
        .arg("missing_input.csv")
        .arg("s3://bucket/out.zarr")
        .arg("--output=json")
        .assert()
        .failure()
        .stdout(predicate::str::contains("does not exist"));
}

#[test]
fn ingest_rejects_invalid_chunks_json() {
    geozarr_ext_path();
    let dir = tempfile::tempdir().unwrap();
    // Use GeoJSON fixture so ST_Read succeeds; --chunks parse error is reached
    // only after spatial loading and ST_Read succeed.
    eider(&dir)
        .env("GEOZARR_ALLOW_PATH", dir.path())
        .arg("ingest")
        .arg(fixture_path("ingest_input.geojson"))
        .arg(dir.path().join("o.zarr").to_str().unwrap())
        .args(["--chunks", "not-json"])
        .arg("--output=json")
        .assert()
        .failure()
        .stdout(predicate::str::contains("Failed to parse"));
}
